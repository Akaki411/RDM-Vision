use std::time::Duration;

use crate::config::ApiConfig;
use crate::data::Code;

pub struct ApiClient
{
    #[allow(dead_code)]
    url: String
}

impl ApiClient
{
    pub fn new(cfg: &ApiConfig) -> Self
    {
        return Self { url: format!("{}{}", cfg.base_url, cfg.code_endpoint) };
    }

    // Связь с API-сервером (ЗАГЛУШКА)
    pub async fn send(&self, code: &Code, took: Duration)
    {
        tracing::info!(
            camera = %code.camera_id,
            code = %code.text,
            restored = code.restored,
            took_ms = took.as_secs_f64() * 1000.0,
            "code decoded"
        );
    }
}
