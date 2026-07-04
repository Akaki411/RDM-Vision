#[derive(Debug, thiserror::Error)]
pub enum AppError
{
    #[error("config error: {0}")]
    Config(String),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("camera error: {0}")]
    Camera(String),

    #[error("detection error: {0}")]
    Detect(String)
}

pub type Result<T> = std::result::Result<T, AppError>;
