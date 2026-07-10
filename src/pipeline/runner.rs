use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::api::{ApiClient, Middleware};
use crate::config::Settings;
use crate::core::{accel_providers, laplacian_variance, Cropper, Detector, Prepare, Reader};
use crate::data::{Code, Frame};
use crate::error::Result;
use crate::preview::Preview;
use crate::service::camera::{CamPaces, FrameReceiver};
use crate::service::restore::RestoreClient;

pub struct Pipeline
{
    settings: Settings
}

struct Shared
{
    middleware: Mutex<Middleware>,
    api: ApiClient,
    preview: Preview,
    stats: Stats,
    paces: CamPaces
}

impl Pipeline
{
    pub fn new(settings: &Settings) -> Result<Self>
    {
        return Ok(Self { settings: settings.clone() });
    }

    // Поднять пул воркеров, репортер статистики и раздавать им кадры
    pub async fn run(self, frames: FrameReceiver, paces: CamPaces) -> Result<()>
    {
        let workers = self.settings.pipeline.worker_count();

        tracing::info!(
            workers,
            cold_fps = self.settings.pipeline.cold_fps,
            hot_fps = self.settings.pipeline.hot_fps,
            accel = ?accel_providers(),
            "pipeline starting"
        );

        let shared = Arc::new(Shared
        {
            middleware: Mutex::new(Middleware::new(&self.settings.api)),
            api: ApiClient::new(&self.settings.api),
            preview: Preview::new(self.settings.preview),
            stats: Stats::default(),
            paces
        });

        let rx = Arc::new(Mutex::new(frames));
        let mut handles = Vec::with_capacity(workers);
        for id in 0..workers
        {
            let worker = Worker::new(id, &self.settings, shared.clone())?;
            let rx = rx.clone();
            handles.push(tokio::spawn(worker.run(rx)));
        }

        let reporter = spawn_reporter(shared.clone());

        for handle in handles
        {
            let _ = handle.await;
        }
        reporter.abort();

        tracing::info!("frame channel closed, pipeline finished");
        return Ok(());
    }
}

struct Worker
{
    id: usize,
    prep: Prepare,
    detector: Detector,
    cropper: Cropper,
    reader: Reader,
    restore: RestoreClient,
    blur_threshold: f32,
    shared: Arc<Shared>
}

impl Worker
{
    fn new(id: usize, settings: &Settings, shared: Arc<Shared>) -> Result<Self>
    {
        return Ok(Self
        {
            id,
            prep: Prepare::new(&settings.normalization),
            detector: Detector::new(settings.detection.clone())?,
            cropper: Cropper::new(),
            reader: Reader::new(),
            restore: RestoreClient::new(&settings.restore_service),
            blur_threshold: settings.detection.blur_threshold,
            shared
        });
    }

    async fn run(mut self, rx: Arc<Mutex<FrameReceiver>>)
    {
        tracing::debug!(worker = self.id, "worker started");
        loop
        {
            let frame = {
                let mut guard = rx.lock().await;
                match guard.recv().await
                {
                    Some(frame) => frame,
                    None => break
                }
            };
            self.handle(frame).await;
        }
        tracing::debug!(worker = self.id, "worker stopped");
    }

    // Обработка одного кадра
    async fn handle(&mut self, frame: Frame)
    {
        let start = Instant::now();
        self.shared.stats.processed.fetch_add(1, Ordering::Relaxed);

        let out = tokio::task::block_in_place(|| self.process(&frame));

        if out.seen
        {
            if let Some(pace) = self.shared.paces.get(&frame.camera_id)
            {
                pace.mark_seen();
            }
        }

        for text in out.decoded
        {
            self.publish(&frame, text, false, start).await;
        }

        for crop in out.unread
        {
            let Some(fixed) = self.restore.fix(&crop).await else { continue; };

            if fixed.data == crop.data
            {
                continue;
            }

            self.shared.preview.show_crop(&crop);
            self.shared.preview.show_restored(&fixed);

            let framed = self.cropper.quiet_zone(&fixed);
            if let Some(text) = self.reader.read(&framed, true)
            {
                self.shared.stats.restored.fetch_add(1, Ordering::Relaxed);
                self.publish(&frame, text, true, start).await;
            }
        }
    }

    // Нормализация, быстрый RXing, YOLO, оценка резкости, декод
    fn process(&mut self, frame: &Frame) -> Processed
    {
        // 1. Нормализация: ч/б + ресайз + контраст
        let gray = self.prep.run(frame);

        // 2. Быстрая авто-детекция RXing прямо по всему кадру
        if let Some(text) = self.reader.read(&gray, false)
        {
            self.shared.preview.show(&gray, &[]);
            self.shared.stats.decoded.fetch_add(1, Ordering::Relaxed);
            return Processed { decoded: vec![text], unread: Vec::new(), seen: true };
        }

        // 3. Не прочиталось — зовём YOLO
        let regions = self.detector.find(&gray);
        self.shared.preview.show(&gray, &regions);

        let mut out = Processed { decoded: Vec::new(), unread: Vec::new(), seen: !regions.is_empty() };

        // 4-5. Плотная обрезка, оценка резкости и попытка распознавания
        for region in &regions
        {
            let crop = self.cropper.crop(&gray, region);
            self.shared.preview.show_crop(&crop);

            // Дисперсия Лапласиана
            let focus = laplacian_variance(&crop.data, crop.width as usize, crop.height as usize);
            let sharp = focus >= self.blur_threshold;
            if !sharp
            {
                self.shared.stats.blurry.fetch_add(1, Ordering::Relaxed);
            }

            let framed = self.cropper.quiet_zone(&crop);
            match self.reader.read(&framed, sharp)
            {
                Some(text) =>
                {
                    self.shared.stats.decoded.fetch_add(1, Ordering::Relaxed);
                    out.decoded.push(text);
                }
                None =>
                {
                    tracing::debug!(camera = %frame.camera_id, focus, sharp, "region unread, queued for restore");
                    out.unread.push(crop);
                }
            }
        }
        return out;
    }

    // Отправить распознанный код дальше, если middleware его пропускает
    async fn publish(&self, frame: &Frame, text: String, restored: bool, start: Instant)
    {
        {
            let mut middleware = self.shared.middleware.lock().await;
            if !middleware.allow(&text)
            {
                return;
            }
        }

        let code = Code
        {
            camera_id: frame.camera_id.clone(),
            text,
            captured_at: frame.captured_at,
            restored
        };
        self.shared.api.send(&code, start.elapsed()).await;
    }
}

struct Processed
{
    decoded: Vec<String>,
    unread: Vec<Frame>,
    seen: bool
}

#[derive(Default)]
struct Stats
{
    processed: AtomicU64,
    decoded: AtomicU64,
    restored: AtomicU64,
    blurry: AtomicU64
}

#[derive(Default, Clone, Copy)]
struct Snapshot
{
    processed: u64,
    decoded: u64,
    restored: u64,
    blurry: u64
}

impl Stats
{
    fn snapshot(&self) -> Snapshot
    {
        return Snapshot
        {
            processed: self.processed.load(Ordering::Relaxed),
            decoded: self.decoded.load(Ordering::Relaxed),
            restored: self.restored.load(Ordering::Relaxed),
            blurry: self.blurry.load(Ordering::Relaxed)
        };
    }
}

impl Snapshot
{
    fn delta(self, prev: Snapshot) -> Snapshot
    {
        return Snapshot
        {
            processed: self.processed.saturating_sub(prev.processed),
            decoded: self.decoded.saturating_sub(prev.decoded),
            restored: self.restored.saturating_sub(prev.restored),
            blurry: self.blurry.saturating_sub(prev.blurry)
        };
    }
}

// Раз в 5 секунд сводка по пайплайну
fn spawn_reporter(shared: Arc<Shared>) -> tokio::task::JoinHandle<()>
{
    return tokio::spawn(async move
    {
        let period = Duration::from_secs(5);
        let mut ticker = tokio::time::interval(period);
        let mut last = shared.stats.snapshot();

        loop
        {
            ticker.tick().await;

            let now = shared.stats.snapshot();
            let d = now.delta(last);
            last = now;

            let hot = shared.paces.values().filter(|p| p.is_hot()).count();

            tracing::info!(
                fps = format!("{:.1}", d.processed as f64 / period.as_secs_f64()),
                decoded = d.decoded,
                restored = d.restored,
                blurry = d.blurry,
                hot_cams = hot,
                cams = shared.paces.len(),
                "pipeline stats"
            );
        }
    });
}
