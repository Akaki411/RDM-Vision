use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::error::Result;

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
    #[serde(default)]
    pub preview: bool
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
            preview: true
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
    return "auto".into();
}

impl Default for RtspConfig
{
    fn default() -> Self
    {
        Self
        {
            id: "cam-rtsp-01".into(),
            url: "rtsp://localhost:8554/stream".into(),
            fps: 10.0,
            transport: default_transport(),
            reconnect_delay_ms: 2000,
            read_timeout_ms: 5000,
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
            id: "cam-grpc-01".into(),
            endpoint: "http://192.168.1.20:50051".into(),
            fps: 5.0,
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
        Self { target_size: 640, grayscale: true, contrast: 1.2 }
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
    return 640;
}

impl Default for DetectConfig
{
    fn default() -> Self
    {
        Self
        {
            model_path: "models/yolo26n-pose.onnx".into(),
            input_size: default_input_size(),
            confidence_threshold: 0.5,
            nms_threshold: 0.45
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
        Self { endpoint: "http://127.0.0.1:5000".into(), timeout_ms: 2000 }
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
            base_url: "http://127.0.0.1:3000".into(),
            code_endpoint: "/api/codes".into(),
            repeat_time_ms: 3000,
            timeout_ms: 5000
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
        Self { channel_capacity: 8 }
    }
}
