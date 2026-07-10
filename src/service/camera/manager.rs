use std::collections::HashMap;
use std::sync::Arc;
use std::thread::JoinHandle;

use tokio::sync::mpsc;

use crate::config::{CameraConfig, PipelineConfig, Settings};
use crate::error::{AppError, Result};

use super::base::{CamPace, Camera, FrameReceiver, Stop};
use super::gige::GigeCamera;

// Темпы камер по id — пайплайн через них переключает горячий/холодный ход
pub type CamPaces = HashMap<String, Arc<CamPace>>;

// Установление соединений к камерам по конфигу
fn build(cfg: &CameraConfig) -> Result<Box<dyn Camera>>
{
    match cfg
    {
        CameraConfig::Rtsp(c) => Ok(Box::new(super::rtsp::RtspCamera::new(c.clone()))),
        CameraConfig::Gige(c) => Ok(Box::new(GigeCamera::new(c.clone())))
    }
}

pub struct Cameras
{
    list: Vec<Box<dyn Camera>>
}

impl Cameras
{
    // Список камер из настроек, пропуская выключенные
    pub fn from_settings(settings: &Settings) -> Result<Self>
    {
        let mut list = Vec::new();
        for cfg in &settings.cameras
        {
            if !cfg.enabled()
            {
                tracing::info!(camera = cfg.id(), "camera disabled, skipping");
                continue;
            }
            list.push(build(cfg)?);
        }

        if list.is_empty()
        {
            return Err(AppError::Camera("no enabled cameras".into()));
        }
        return Ok(Self { list });
    }

    // Запустить по потоку на камеру. Возвращает канал кадров, ручку остановки и
    // темпы камер (по ним пайплайн гоняет холодный/горячий режим)
    pub fn spawn(self, capacity: usize, pipeline: &PipelineConfig) -> (FrameReceiver, CamerasHandle, CamPaces)
    {
        let (sender, receiver) = mpsc::channel(capacity);
        let stop = Stop::new();
        let mut handles = Vec::with_capacity(self.list.len());
        let mut paces = CamPaces::new();

        for mut camera in self.list
        {
            let sender = sender.clone();
            let stop = stop.clone();
            let id = camera.id().to_string();
            let pace = CamPace::new(pipeline.cold_fps, pipeline.hot_fps, pipeline.hot_hold_ms);
            paces.insert(id.clone(), pace.clone());
            let handle = std::thread::Builder::new()
                .name(format!("camera-{id}"))
                .spawn(move ||
                {
                    if let Err(err) = camera.run(sender, stop, pace)
                    {
                        tracing::error!(camera = %id, error = %err, "camera thread exited with error");
                    }
                })
                .expect("failed to spawn camera thread");
            handles.push(handle);
        }
        drop(sender);

        return (receiver, CamerasHandle { stop, handles }, paces);
    }
}

pub struct CamerasHandle
{
    stop: Stop,
    handles: Vec<JoinHandle<()>>
}

impl CamerasHandle
{
    // Остановить камеры и дождаться завершения их потоков
    pub fn shutdown(self)
    {
        self.stop.stop();
        for handle in self.handles
        {
            let _ = handle.join();
        }
    }
}
