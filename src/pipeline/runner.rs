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

        let gray = self.prep.run(&frame);
        let regions = self.detector.find(&gray);

        tracing::debug!(camera = %frame.camera_id, regions = regions.len(), "frame processed");

        // Предпросмотр - отправка кадра с нарисованными областями в окно
        self.preview.show(&frame, &regions);

        // Обработка каждого из найденных регионов
        for region in &regions
        {
            let crop = self.cropper.crop(&gray, region);

            // Показываем в окне область в том виде, в котором она уходит в RXing
            self.preview.show_crop(&crop);

            if let Some((text, restored)) = self.read_code(crop).await
            {
                if self.middleware.allow(&text)
                {
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
        }
    }

    // Попытка распознать код, при неудаче одно восстановление и повтор
    async fn read_code(&mut self, crop: Frame) -> Option<(String, bool)>
    {
        if let Some(text) = self.reader.read(&crop)
        {
            return Some((text, false));
        }

        // Если не прочиталось, то восстанавливаем и пробуем ещё раз
        let fixed = self.restore.fix(&crop).await?;
        if let Some(text) = self.reader.read(&fixed)
        {
            return Some((text, true));
        }

        // После восстановления снова ничего - пропускаем кадр
        return None;
    }
}
