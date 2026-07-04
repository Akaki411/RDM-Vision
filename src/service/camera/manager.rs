use std::thread::JoinHandle;

use tokio::sync::mpsc;

use crate::config::{CameraConfig, Settings};
use crate::error::{AppError, Result};

use super::base::{Camera, FrameReceiver, Stop};
use super::grpc::GrpcCamera;

// Установление соединений к камерам по конфигу
fn build(cfg: &CameraConfig) -> Result<Box<dyn Camera>>
{
    match cfg
    {
        CameraConfig::Rtsp(c) => Ok(Box::new(super::rtsp::RtspCamera::new(c.clone()))),
        CameraConfig::Grpc(c) => Ok(Box::new(GrpcCamera::new(c.clone())))
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

    // Запустить по потоку на камеру
    pub fn spawn(self, capacity: usize) -> (FrameReceiver, CamerasHandle)
    {
        let (sender, receiver) = mpsc::channel(capacity);
        let stop = Stop::new();
        let mut handles = Vec::with_capacity(self.list.len());

        for mut camera in self.list
        {
            let sender = sender.clone();
            let stop = stop.clone();
            let id = camera.id().to_string();
            let handle = std::thread::Builder::new()
                .name(format!("camera-{id}"))
                .spawn(move ||
                {
                    if let Err(err) = camera.run(sender, stop)
                    {
                        tracing::error!(camera = %id, error = %err, "camera thread exited with error");
                    }
                })
                .expect("failed to spawn camera thread");
            handles.push(handle);
        }
        drop(sender);

        return (receiver, CamerasHandle { stop, handles });
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
