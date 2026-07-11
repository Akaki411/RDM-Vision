use std::sync::Arc;
use std::thread::JoinHandle;

use tokio::sync::watch;

use crate::config::{CameraConfig, PipelineConfig, Settings};
use crate::error::{AppError, Result};

use super::base::{CamPace, Camera, FrameReceiver, Stop};
use super::gige::GigeCamera;

pub struct CamStream
{
    pub id: String,
    pub frames: FrameReceiver,
    pub pace: Arc<CamPace>
}

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

    // Запустить по потоку на камеру, у каждой свой слот последнего кадра и темп
    pub fn spawn(self, pipeline: &PipelineConfig) -> (Vec<CamStream>, CamerasHandle)
    {
        let stop = Stop::new();
        let mut handles = Vec::with_capacity(self.list.len());
        let mut streams = Vec::with_capacity(self.list.len());

        for mut camera in self.list
        {
            let id = camera.id().to_string();
            let pace = CamPace::new(pipeline.cold_fps, pipeline.hot_fps, pipeline.hot_hold_ms);
            let (sender, receiver) = watch::channel(None::<crate::data::Frame>);
            streams.push(CamStream { id: id.clone(), frames: receiver, pace: pace.clone() });

            let stop = stop.clone();
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

        return (streams, CamerasHandle { stop, handles });
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
