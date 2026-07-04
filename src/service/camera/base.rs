use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc;

use crate::data::Frame;
use crate::error::Result;

// Камеры пишут сюда, получатель читает пайплайн
pub type FrameSender = mpsc::Sender<Frame>;
pub type FrameReceiver = mpsc::Receiver<Frame>;

// Флаг остановки всех потоков камер после завершения работы программы
#[derive(Clone, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop
{
    pub fn new() -> Self
    {
        return Self::default();
    }

    pub fn stop(&self)
    {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_stopped(&self) -> bool
    {
        return self.0.load(Ordering::Relaxed);
    }

    pub fn inner(&self) -> Arc<AtomicBool>
    {
        return self.0.clone();
    }
}

// Источник кадров
pub trait Camera: Send
{
    fn id(&self) -> &str;
    fn run(&mut self, sender: FrameSender, stop: Stop) -> Result<()>;
}
