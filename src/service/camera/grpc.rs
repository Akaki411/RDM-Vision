use crate::config::GrpcConfig;
use crate::error::Result;

use super::base::{Camera, FrameSender, Stop};

pub struct GrpcCamera
{
    cfg: GrpcConfig
}

impl GrpcCamera
{
    pub fn new(cfg: GrpcConfig) -> Self
    {
        return Self { cfg };
    }
}

impl Camera for GrpcCamera
{
    fn id(&self) -> &str
    {
        return &self.cfg.id;
    }

    fn run(&mut self, _sender: FrameSender, _stop: Stop) -> Result<()>
    {
        // МОДУЛЬ ЗАГЛУШКА, НАДО ДОПИСАТЬ
        tracing::warn!(camera = %self.cfg.id, "gRPC camera not implemented yet");
        return Ok(());
    }
}
