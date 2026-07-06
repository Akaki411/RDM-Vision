use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::error::Result;

// Конфиг по умолчанию
mod defaults
{
    pub const RTSP_ID: &str = "cam-rtsp-01";
    pub const RTSP_URL: &str = "rtsp://localhost:8554/";
    pub const RTSP_FPS: f64 = 10.0;
    pub const RTSP_TRANSPORT: &str = "auto";
    pub const RTSP_RECONNECT_MS: u64 = 2000;
    pub const RTSP_READ_TIMEOUT_MS: u64 = 5000;

    pub const GRPC_ID: &str = "cam-grpc-01";
    pub const GRPC_ENDPOINT: &str = "http://192.168.1.20:50051";
    pub const GRPC_FPS: f64 = 5.0;

    pub const NORM_TARGET_SIZE: u32 = 640;
    pub const NORM_CONTRAST: f32 = 1.4;

    pub const DETECT_MODEL_PATH: &str = "models/yolo26n-pose.onnx";
    pub const DETECT_INPUT_SIZE: u32 = 640;
    pub const DETECT_CONFIDENCE: f32 = 0.5;
    pub const DETECT_NMS: f32 = 0.45;

    pub const RESTORE_ENDPOINT: &str = "http://127.0.0.1:5000";
    pub const RESTORE_TIMEOUT_MS: u64 = 2000;

    pub const API_BASE_URL: &str = "http://127.0.0.1:3000";
    pub const API_CODE_ENDPOINT: &str = "/api/codes";
    pub const API_REPEAT_MS: u64 = 3000;
    pub const API_TIMEOUT_MS: u64 = 5000;

    pub const PIPELINE_CHANNEL_CAPACITY: usize = 8;

    pub const PREVIEW: bool = false;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings
{
    pub cameras: Vec<CameraConfig>,
    pub normalization: NormConfig,
    pub detection: DetectConfig,
    pub restore_service: RestoreConfig,
    pub api: ApiConfig,
    #[serde(default)]
    pub pipeline: PipelineConfig,
    #[serde(default = "default_preview")]
    pub preview: bool
}

fn default_preview() -> bool
{
    return defaults::PREVIEW;
}

impl Settings
{
    // Загрузить настройки
    pub fn load(path: impl AsRef<Path>) -> Result<(Self, bool)>
    {
        let path = path.as_ref();

        if !path.exists()
        {
            let cfg = Settings::default();
            cfg.save(path)?;
            return Ok((cfg, true));
        }

        let text = std::fs::read_to_string(path)?;
        match serde_json::from_str::<Settings>(&text)
        {
            Ok(cfg) => Ok((cfg, false)),
            Err(_) =>
            {
                let cfg = Settings::default();
                cfg.save(path)?;
                Ok((cfg, true))
            }
        }
    }

    // Записать настройки в файл в читаемом виде
    fn save(&self, path: &Path) -> Result<()>
    {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        return Ok(());
    }
}

// Дефолтные настройки для создания конфига:
impl Default for Settings
{
    fn default() -> Self
    {
        Self
        {
            cameras: vec!
            [
                CameraConfig::Rtsp(RtspConfig::default()),
                CameraConfig::Grpc(GrpcConfig::default())
            ],
            normalization: NormConfig::default(),
            detection: DetectConfig::default(),
            restore_service: RestoreConfig::default(),
            api: ApiConfig::default(),
            pipeline: PipelineConfig::default(),
            preview: defaults::PREVIEW
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CameraConfig
{
    Rtsp(RtspConfig),
    Grpc(GrpcConfig)
}

impl CameraConfig
{
    pub fn id(&self) -> &str
    {
        match self
        {
            CameraConfig::Rtsp(c) => &c.id,
            CameraConfig::Grpc(c) => &c.id
        }
    }

    pub fn enabled(&self) -> bool
    {
        match self
        {
            CameraConfig::Rtsp(c) => c.enabled,
            CameraConfig::Grpc(c) => c.enabled
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtspConfig
{
    pub id: String,
    pub url: String,
    pub fps: f64,
    #[serde(default = "default_transport")]
    pub transport: String,
    pub reconnect_delay_ms: u64,
    pub read_timeout_ms: u64,
    pub enabled: bool
}

fn default_transport() -> String
{
    return defaults::RTSP_TRANSPORT.into();
}

impl Default for RtspConfig
{
    fn default() -> Self
    {
        Self
        {
            id: defaults::RTSP_ID.into(),
            url: defaults::RTSP_URL.into(),
            fps: defaults::RTSP_FPS,
            transport: default_transport(),
            reconnect_delay_ms: defaults::RTSP_RECONNECT_MS,
            read_timeout_ms: defaults::RTSP_READ_TIMEOUT_MS,
            enabled: true
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig
{
    pub id: String,
    pub endpoint: String,
    pub fps: f64,
    pub enabled: bool
}

impl Default for GrpcConfig
{
    fn default() -> Self
    {
        Self
        {
            id: defaults::GRPC_ID.into(),
            endpoint: defaults::GRPC_ENDPOINT.into(),
            fps: defaults::GRPC_FPS,
            enabled: true
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormConfig
{
    pub target_size: u32,
    pub grayscale: bool,
    pub contrast: f32
}

impl Default for NormConfig
{
    fn default() -> Self
    {
        Self { target_size: defaults::NORM_TARGET_SIZE, grayscale: true, contrast: defaults::NORM_CONTRAST }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectConfig
{
    pub model_path: String,
    #[serde(default = "default_input_size")]
    pub input_size: u32,
    pub confidence_threshold: f32,
    pub nms_threshold: f32
}

fn default_input_size() -> u32
{
    return defaults::DETECT_INPUT_SIZE;
}

impl Default for DetectConfig
{
    fn default() -> Self
    {
        Self
        {
            model_path: defaults::DETECT_MODEL_PATH.into(),
            input_size: default_input_size(),
            confidence_threshold: defaults::DETECT_CONFIDENCE,
            nms_threshold: defaults::DETECT_NMS
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreConfig
{
    pub endpoint: String,
    pub timeout_ms: u64
}

impl Default for RestoreConfig
{
    fn default() -> Self
    {
        Self { endpoint: defaults::RESTORE_ENDPOINT.into(), timeout_ms: defaults::RESTORE_TIMEOUT_MS }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig
{
    pub base_url: String,
    pub code_endpoint: String,
    pub repeat_time_ms: u64,
    pub timeout_ms: u64
}

impl Default for ApiConfig
{
    fn default() -> Self
    {
        Self
        {
            base_url: defaults::API_BASE_URL.into(),
            code_endpoint: defaults::API_CODE_ENDPOINT.into(),
            repeat_time_ms: defaults::API_REPEAT_MS,
            timeout_ms: defaults::API_TIMEOUT_MS
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig
{
    pub channel_capacity: usize
}

impl Default for PipelineConfig
{
    fn default() -> Self
    {
        Self { channel_capacity: defaults::PIPELINE_CHANNEL_CAPACITY }
    }
}
