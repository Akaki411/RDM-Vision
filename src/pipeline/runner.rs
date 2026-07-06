use std::time::{Duration, Instant};

use crate::api::{ApiClient, Middleware};
use crate::config::Settings;
use crate::core::{Cropper, Detector, Prepare, Reader};
use crate::data::{Code, Frame};
use crate::error::Result;
use crate::preview::Preview;
use crate::service::camera::FrameReceiver;
use crate::service::restore::RestoreClient;

pub struct Pipeline
{
    prep: Prepare,
    detector: Detector,
    cropper: Cropper,
    reader: Reader,
    restore: RestoreClient,
    middleware: Middleware,
    api: ApiClient,

    // Предпросмотр для дебага, чтобы убрать надо удалить это поле, его использование ниже и модуль preview
    preview: Preview
}

impl Pipeline
{
    // Применяем настройки ко всем модулям
    pub fn new(settings: &Settings) -> Result<Self>
    {
        return Ok(Self
        {
            prep: Prepare::new(&settings.normalization),
            detector: Detector::new(settings.detection.clone())?,
            cropper: Cropper::new(),
            reader: Reader::new(),
            restore: RestoreClient::new(&settings.restore_service),
            middleware: Middleware::new(&settings.api),
            api: ApiClient::new(&settings.api),
            preview: Preview::new(settings.preview)
        });
    }

    // Читать кадры из канала камер, пока он не закроется
    pub async fn run(mut self, mut frames: FrameReceiver) -> Result<()>
    {
        // Счётчик кадров
        let mut tick = Instant::now();

        while let Some(frame) = frames.recv().await
        {
            self.handle(frame).await;

            if tick.elapsed() >= Duration::from_secs(1)
            {
                tick = Instant::now();
            }
        }
        tracing::info!("frame channel closed, pipeline finished");
        return Ok(());
    }

    // Обработка одного кадра
    async fn handle(&mut self, frame: Frame)
    {
        let start = Instant::now();

        // 1. Нормализация: ч/б + контраст
        let gray = self.prep.run(&frame);

        // Предпросмотр - нормализованный поток уходит в окно каждым кадром
        self.preview.show(&gray, &[]);

        // 2. Быстрая авто-детекция RXing прямо по всему кадру
        if let Some(text) = self.reader.read(&gray)
        {
            self.publish(&frame, text, false, start).await;
            return;
        }

        // 3. Не прочиталось — зовём YOLO. Регионов нет — берём следующий кадр
        let regions = self.detector.find(&gray);
        tracing::debug!(camera = %frame.camera_id, regions = regions.len(), "frame processed");
        let Some(region) = regions.first() else { return; };

        // 4. ROI обрезка. Показываем область в том виде, в котором она уходит в RXing
        let crop = self.cropper.crop(&gray, region);
        self.preview.show_crop(&crop);

        // 5. Пробуем распознать обрезанную область
        if let Some(text) = self.reader.read(&crop)
        {
            self.publish(&frame, text, false, start).await;
            return;
        }

        // 6. Восстанавливаем область через gRPC-сервис и пробуем ещё раз
        let Some(fixed) = self.restore.fix(&crop).await else { return; };
        if let Some(text) = self.reader.read(&fixed)
        {
            self.publish(&frame, text, true, start).await;
        }
    }

    // Отправить распознанный код дальше, если middleware его пропускает
    async fn publish(&mut self, frame: &Frame, text: String, restored: bool, start: Instant)
    {
        if !self.middleware.allow(&text)
        {
            return;
        }

        let code = Code
        {
            camera_id: frame.camera_id.clone(),
            text,
            captured_at: frame.captured_at,
            restored
        };
        self.api.send(&code, start.elapsed()).await;
    }
}
