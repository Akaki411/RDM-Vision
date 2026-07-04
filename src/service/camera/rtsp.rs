use std::time::{Duration, Instant, SystemTime};

use crate::config::RtspConfig;
use crate::data::{Frame, PixelFormat};
use crate::error::Result;

use super::base::{Camera, FrameSender, Stop};
use super::docker::DockerFrameStream;

pub struct RtspCamera
{
    cfg: RtspConfig
}

impl RtspCamera
{
    pub fn new(cfg: RtspConfig) -> Self
    {
        Self { cfg }
    }

    fn interval(&self) -> Duration
    {
        if self.cfg.fps > 0.0
        {
            Duration::from_secs_f64(1.0 / self.cfg.fps)
        }
        else
        {
            Duration::ZERO
        }
    }
}

impl Camera for RtspCamera
{
    fn id(&self) -> &str
    {
        &self.cfg.id
    }

    fn run(&mut self, sender: FrameSender, stop: Stop) -> Result<()>
    {
        let interval = self.interval();
        let reconnect = Duration::from_millis(self.cfg.reconnect_delay_ms);
        let width = 640u32;
        let height = 480u32;
        let frame_size = (width * height) as usize;

        while !stop.is_stopped()
        {
            tracing::info!(
                camera = %self.cfg.id,
                url = %self.cfg.url,
                "spawning docker ffmpeg container"
            );

            let mut stream = match DockerFrameStream::spawn(
                &self.cfg.url,
                self.cfg.fps,
                &self.cfg.transport,
                width,
                height,
                stop.inner(),
            )
            {
                Ok(s) => s,
                Err(err) =>
                {
                    tracing::warn!(camera = %self.cfg.id, error = %err, "spawn failed, retrying");
                    sleep_stoppable(reconnect, &stop);
                    continue;
                }
            };

            tracing::info!(camera = %self.cfg.id, "connected, reading frames");

            let mut last_sent = Instant::now().checked_sub(interval).unwrap_or_else(Instant::now);
            let mut got_first = false;

            while !stop.is_stopped()
            {
                match stream.read_frame(frame_size)
                {
                    Some(data) =>
                    {
                        if !got_first
                        {
                            got_first = true;
                            tracing::info!(
                                camera = %self.cfg.id,
                                w = width,
                                h = height,
                                "first frame received"
                            );
                        }

                        if last_sent.elapsed() < interval
                        {
                            continue;
                        }
                        last_sent = Instant::now();

                        let frame = Frame
                        {
                            camera_id: self.cfg.id.clone(),
                            captured_at: SystemTime::now(),
                            width,
                            height,
                            format: PixelFormat::Gray8,
                            data,
                        };

                        if sender.blocking_send(frame).is_err()
                        {
                            break;
                        }
                    }
                    None =>
                    {
                        tracing::warn!(camera = %self.cfg.id, "stream ended, reconnecting");
                        break;
                    }
                }
            }

            if !stop.is_stopped()
            {
                sleep_stoppable(reconnect, &stop);
            }
        }

        Ok(())
    }
}

fn sleep_stoppable(dur: Duration, stop: &Stop)
{
    let step = Duration::from_millis(100);
    let mut left = dur;
    while left > Duration::ZERO && !stop.is_stopped()
    {
        let chunk = left.min(step);
        std::thread::sleep(chunk);
        left = left.saturating_sub(chunk);
    }
}
