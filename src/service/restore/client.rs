use std::time::Duration;

use crate::config::RestoreConfig;
use crate::data::Frame;

pub struct RestoreClient
{
    endpoint: String,
    #[allow(dead_code)]
    timeout: Duration
}

impl RestoreClient
{
    pub fn new(cfg: &RestoreConfig) -> Self
    {
        return Self
        {
            endpoint: cfg.endpoint.clone(),
            timeout: Duration::from_millis(cfg.timeout_ms)
        };
    }

    // Отправить область кода и получить восстановленную, None - восстановить не удалось
    pub async fn fix(&mut self, image: &Frame) -> Option<Frame>
    {
        // ЗАГЛУШКА
        //Надо подключиться к self.endpoint (tonic), собрать RestoreRequest из
        // байтов изображения, вызвать RestoreDM и при success вернуть новый Frame
        let _ = (image, &self.endpoint);
        return None;
    }
}
