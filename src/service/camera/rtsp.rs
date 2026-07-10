use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use crate::config::RtspConfig;
use crate::data::{Frame, PixelFormat};
use crate::error::Result;

use super::base::{CamPace, Camera, FrameSender, Stop};
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
}

impl Camera for RtspCamera
{
    fn id(&self) -> &str
    {
        &self.cfg.id
    }

    fn run(&mut self, sender: FrameSender, stop: Stop, pace: Arc<CamPace>) -> Result<()>
    {
        // ffmpeg тянет по верхней (горячей) частоте, а вниз до холодной темп режет
        // сам поток камеры по pace.interval() — так холостой ход дешёвый
        let capture_fps = pace.capture_fps();
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
                capture_fps,
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

            let mut last_sent = Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or_else(Instant::now);
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

                        // Темп режем по текущему режиму: холодный M fps / горячий N fps
                        if last_sent.elapsed() < pace.interval()
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

                        // Пайплайн занят — кадр отбрасываем, свежий важнее очереди
                        match sender.try_send(frame)
                        {
                            Ok(()) | Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {}
                            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break
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
