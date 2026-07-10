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

    pub const GIGE_ID: &str = "cam-gige-01";
    pub const GIGE_ADDRESS: &str = "127.0.0.1";
    pub const GIGE_INTERFACE: &str = "127.0.0.1";

    pub const NORM_TARGET_SIZE: u32 = 640;
    pub const NORM_CONTRAST: f32 = 1.4;

    pub const DETECT_MODEL_PATH: &str = "models/yolo26n-pose.onnx";
    pub const DETECT_INPUT_SIZE: u32 = 640;
    pub const DETECT_CONFIDENCE: f32 = 0.4;
    pub const DETECT_NMS: f32 = 0.45;
    pub const DETECT_BLUR_THRESHOLD: f32 = 15.0;

    pub const RESTORE_ENDPOINT: &str = "http://127.0.0.1:50051";
    pub const RESTORE_TIMEOUT_MS: u64 = 2000;

    pub const API_BASE_URL: &str = "http://127.0.0.1:3000";
    pub const API_CODE_ENDPOINT: &str = "/api/codes";
    pub const API_REPEAT_MS: u64 = 3000;
    pub const API_TIMEOUT_MS: u64 = 5000;

    pub const PIPELINE_CHANNEL_CAPACITY: usize = 8;
    pub const PIPELINE_WORKERS: usize = 0;
    pub const PIPELINE_COLD_FPS: f64 = 3.0;
    pub const PIPELINE_HOT_FPS: f64 = 15.0;
    pub const PIPELINE_HOT_HOLD_MS: u64 = 1500;

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
                CameraConfig::Gige(GigeConfig::default())
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
    Gige(GigeConfig)
}

impl CameraConfig
{
    pub fn id(&self) -> &str
    {
        match self
        {
            CameraConfig::Rtsp(c) => &c.id,
            CameraConfig::Gige(c) => &c.id
        }
    }

    pub fn enabled(&self) -> bool
    {
        match self
        {
            CameraConfig::Rtsp(c) => c.enabled,
            CameraConfig::Gige(c) => c.enabled
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

// GigE Vision / GenICam камера. address/interface пустые — включаем авто-режим
// (discovery в сети / выбор интерфейса). Темп кадров задаёт pipeline (cold/hot)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GigeConfig
{
    pub id: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub interface: String,
    pub enabled: bool
}

impl Default for GigeConfig
{
    fn default() -> Self
    {
        Self
        {
            id: defaults::GIGE_ID.into(),
            address: defaults::GIGE_ADDRESS.into(),
            interface: defaults::GIGE_INTERFACE.into(),
            enabled: false
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
    pub nms_threshold: f32,
    #[serde(default = "default_blur_threshold")]
    pub blur_threshold: f32
}

fn default_input_size() -> u32
{
    return defaults::DETECT_INPUT_SIZE;
}

fn default_blur_threshold() -> f32
{
    return defaults::DETECT_BLUR_THRESHOLD;
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
            nms_threshold: defaults::DETECT_NMS,
            blur_threshold: default_blur_threshold()
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
    pub channel_capacity: usize,
    // Число параллельных воркеров обработки. 0 — по числу ядер
    #[serde(default = "default_workers")]
    pub workers: usize,
    // M — предел кадров, пока код не виден
    #[serde(default = "default_cold_fps")]
    pub cold_fps: f64,
    // N — предел кадров, когда код в кадре (разгон)
    #[serde(default = "default_hot_fps")]
    pub hot_fps: f64,
    // Удержание горячего режима после последнего обнаружения
    #[serde(default = "default_hot_hold_ms")]
    pub hot_hold_ms: u64
}

fn default_workers() -> usize
{
    return defaults::PIPELINE_WORKERS;
}

fn default_cold_fps() -> f64
{
    return defaults::PIPELINE_COLD_FPS;
}

fn default_hot_fps() -> f64
{
    return defaults::PIPELINE_HOT_FPS;
}

fn default_hot_hold_ms() -> u64
{
    return defaults::PIPELINE_HOT_HOLD_MS;
}

impl PipelineConfig
{
    // Развёрнутое число воркеров: 0 в конфиге → по числу доступных ядер
    pub fn worker_count(&self) -> usize
    {
        if self.workers > 0
        {
            return self.workers;
        }
        return std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    }
}

impl Default for PipelineConfig
{
    fn default() -> Self
    {
        Self
        {
            channel_capacity: defaults::PIPELINE_CHANNEL_CAPACITY,
            workers: default_workers(),
            cold_fps: default_cold_fps(),
            hot_fps: default_hot_fps(),
            hot_hold_ms: default_hot_hold_ms()
        }
    }
}
