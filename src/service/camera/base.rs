use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::watch;

use crate::data::Frame;
use crate::error::Result;

pub type FrameSender = watch::Sender<Option<Frame>>;
pub type FrameReceiver = watch::Receiver<Option<Frame>>;

pub struct CamPace
{
    start: Instant,
    hot_until_ms: AtomicU64,
    cold: Duration,
    hot: Duration,
    hold: Duration
}

impl CamPace
{
    pub fn new(cold_fps: f64, hot_fps: f64, hot_hold_ms: u64) -> Arc<Self>
    {
        return Arc::new(Self
        {
            start: Instant::now(),
            hot_until_ms: AtomicU64::new(0),
            cold: interval_of(cold_fps),
            hot: interval_of(hot_fps),
            hold: Duration::from_millis(hot_hold_ms)
        });
    }

    fn now_ms(&self) -> u64
    {
        return self.start.elapsed().as_millis() as u64;
    }

    // Код замечен — держим камеру горячей ещё hold миллисекунд
    pub fn mark_seen(&self)
    {
        let until = self.now_ms() + self.hold.as_millis() as u64;
        self.hot_until_ms.store(until, Ordering::Relaxed);
    }

    pub fn is_hot(&self) -> bool
    {
        return self.now_ms() < self.hot_until_ms.load(Ordering::Relaxed);
    }

    // Минимальный интервал между кадрами при текущем режиме
    pub fn interval(&self) -> Duration
    {
        if self.is_hot()
        {
            return self.hot;
        }
        return self.cold;
    }
}

fn interval_of(fps: f64) -> Duration
{
    if fps > 0.0
    {
        return Duration::from_secs_f64(1.0 / fps);
    }
    return Duration::ZERO;
}

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

pub trait Camera: Send
{
    fn id(&self) -> &str;
    fn run(&mut self, sender: FrameSender, stop: Stop, pace: Arc<CamPace>) -> Result<()>;
}
