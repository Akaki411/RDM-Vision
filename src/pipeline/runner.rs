use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant};

use tokio::sync::Mutex;

use crate::config::Settings;
use crate::core::{accel_providers, laplacian_variance, Cropper, Detector, Prepare, Reader};
use crate::data::Frame;
use crate::error::Result;
use crate::preview::Preview;
use crate::service::camera::{CamPace, CamStream, FrameReceiver};
use crate::service::restore::RestoreClient;
use crate::ws::{CodeMessage, CodeServer, Middleware};

pub struct Pipeline
{
    settings: Settings
}

struct Shared
{
    middleware: Mutex<Middleware>,
    server: CodeServer,
    preview: Preview,
    stats: Stats
}

impl Pipeline
{
    pub fn new(settings: &Settings) -> Result<Self>
    {
        return Ok(Self { settings: settings.clone() });
    }

    pub async fn run(self, streams: Vec<CamStream>) -> Result<()>
    {
        tracing::info!(
            cameras = streams.len(),
            cold_fps = self.settings.pipeline.cold_fps,
            hot_fps = self.settings.pipeline.hot_fps,
            accel = ?accel_providers(),
            "pipeline starting"
        );

        let shared = Arc::new(Shared
        {
            middleware: Mutex::new(Middleware::new(&self.settings.websocket)),
            server: CodeServer::start(&self.settings.websocket).await?,
            preview: Preview::new(self.settings.preview),
            stats: Stats::default()
        });

        let mut handles = Vec::with_capacity(streams.len());
        for stream in streams
        {
            let worker = Worker::new(&self.settings, shared.clone(), stream.pace.clone())?;
            handles.push(tokio::spawn(worker.run(stream.frames)));
        }

        for handle in handles
        {
            let _ = handle.await;
        }

        tracing::info!("all camera streams closed, pipeline finished");
        return Ok(());
    }
}

struct Worker
{
    prep: Prepare,
    detector: Detector,
    cropper: Cropper,
    reader: Reader,
    restore: Option<RestoreClient>,
    blur_threshold: f32,
    pace: Arc<CamPace>,
    shared: Arc<Shared>
}

impl Worker
{
    fn new(settings: &Settings, shared: Arc<Shared>, pace: Arc<CamPace>) -> Result<Self>
    {
        let restore = if settings.restore_service.enabled
        {
            Some(RestoreClient::new(&settings.restore_service))
        }
        else
        {
            None
        };

        return Ok(Self
        {
            prep: Prepare::new(&settings.normalization),
            detector: Detector::new(settings.detection.clone())?,
            cropper: Cropper::new(),
            reader: Reader::new(&settings.recognition),
            restore,
            blur_threshold: settings.detection.blur_threshold,
            pace,
            shared
        });
    }

    // Берём самый свежий кадр слота, промежуточные watch отбрасывает сам
    async fn run(mut self, mut rx: FrameReceiver)
    {
        while rx.changed().await.is_ok()
        {
            let frame = rx.borrow_and_update().clone();
            if let Some(frame) = frame
            {
                self.handle(frame).await;
            }
        }
    }

    // Обработка одного кадра
    async fn handle(&mut self, frame: Frame)
    {
        let start = Instant::now();
        self.shared.stats.processed.fetch_add(1, Ordering::Relaxed);

        let out = tokio::task::block_in_place(|| self.process(&frame));

        if out.seen
        {
            self.pace.mark_seen();
        }

        for text in out.decoded
        {
            self.publish(&frame, text, false, start).await;
        }

        if let Some(restore) = &self.restore
        {
            for crop in out.unread
            {
                let Some(fixed) = restore.fix(&crop).await else { continue; };

                if fixed.data == crop.data
                {
                    continue;
                }

                self.shared.preview.show_crop(&crop);
                self.shared.preview.show_restored(&fixed);

                let framed = self.cropper.quiet_zone(&fixed);
                if let Some(text) = self.reader.read_roi(&framed)
                {
                    self.shared.stats.restored.fetch_add(1, Ordering::Relaxed);
                    self.publish(&frame, text, true, start).await;
                }
            }
        }
    }

    // Нормализация → быстрый полнокадровый ZXing → YOLO → обрезка → тщательный ZXing
    fn process(&mut self, frame: &Frame) -> Processed
    {
        let gray = self.prep.run(frame);

        let texts = self.reader.read_frame(&gray);
        if !texts.is_empty()
        {
            self.shared.preview.show(&gray, &[]);
            self.shared.stats.decoded.fetch_add(texts.len() as u64, Ordering::Relaxed);
            return Processed { decoded: texts, unread: Vec::new(), seen: true };
        }

        if self.restore.is_none()
        {
            self.shared.preview.show(&gray, &[]);
            return Processed { decoded: Vec::new(), unread: Vec::new(), seen: false };
        }

        let regions = self.detector.find(&gray);
        self.shared.preview.show(&gray, &regions);

        let mut out = Processed { decoded: Vec::new(), unread: Vec::new(), seen: !regions.is_empty() };

        for region in &regions
        {
            let crop = self.cropper.crop(&gray, region);
            self.shared.preview.show_crop(&crop);

            let focus = laplacian_variance(&crop.data, crop.width as usize, crop.height as usize);
            if focus < self.blur_threshold
            {
                self.shared.stats.blurry.fetch_add(1, Ordering::Relaxed);
                out.unread.push(crop);
                continue;
            }

            let framed = self.cropper.quiet_zone(&crop);
            match self.reader.read_roi(&framed)
            {
                Some(text) =>
                {
                    self.shared.stats.decoded.fetch_add(1, Ordering::Relaxed);
                    out.decoded.push(text);
                }
                None => out.unread.push(crop)
            }
        }
        return out;
    }

    // Разослать распознанный код клиентам, если middleware его пропускает
    async fn publish(&self, frame: &Frame, text: String, restored: bool, start: Instant)
    {
        {
            let mut middleware = self.shared.middleware.lock().await;
            if !middleware.allow(&text)
            {
                return;
            }
        }

        let message = CodeMessage
        {
            camera_id: frame.camera_id.clone(),
            code: text,
            restored,
            time_ms: start.elapsed().as_millis() as u64
        };
        self.shared.server.broadcast(&message);
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

