use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use futures::StreamExt;
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;
use openh264::nal_units;
use retina::client::{self, PlayOptions, SessionOptions, SetupOptions};
use retina::codec::{CodecItem, FrameFormat};
use url::Url;

use crate::config::RtspConfig;
use crate::data::{Frame, PixelFormat};
use crate::error::{AppError, Result};

use super::base::{CamPace, Camera, FrameSender, Stop};

#[derive(Clone, Copy, PartialEq)]
enum Codec
{
    H264,
    Jpeg
}

pub struct RtspCamera
{
    cfg: RtspConfig
}

impl RtspCamera
{
    pub fn new(cfg: RtspConfig) -> Self
    {
        return Self { cfg };
    }
}

impl Camera for RtspCamera
{
    fn id(&self) -> &str
    {
        return &self.cfg.id;
    }

    fn run(&mut self, sender: FrameSender, stop: Stop, pace: Arc<CamPace>) -> Result<()>
    {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| AppError::Camera(format!("rtsp runtime: {e}")))?;

        let reconnect = Duration::from_millis(self.cfg.reconnect_delay_ms);
        while !stop.is_stopped()
        {
            if let Err(err) = rt.block_on(self.session(&sender, &stop, &pace))
            {
                tracing::warn!(camera = %self.cfg.id, error = %err, "rtsp session ended, reconnecting");
            }
            if !stop.is_stopped()
            {
                sleep_stoppable(reconnect, &stop);
            }
        }
        return Ok(());
    }
}

impl RtspCamera
{
    // Одно подключение: DESCRIBE → SETUP → PLAY → декод кадров в Gray8
    async fn session(&self, sender: &FrameSender, stop: &Stop, pace: &Arc<CamPace>) -> Result<()>
    {
        let url = Url::parse(&self.cfg.url).map_err(|e| AppError::Camera(format!("rtsp url: {e}")))?;

        let mut session = client::Session::describe(url, SessionOptions::default())
            .await
            .map_err(|e| AppError::Camera(format!("rtsp describe: {e}")))?;

        let mut pick = None;
        for (i, s) in session.streams().iter().enumerate()
        {
            tracing::debug!(camera = %self.cfg.id, stream = i, media = s.media(), encoding = s.encoding_name(), "rtsp stream offered");
            if s.media() != "video"
            {
                continue;
            }
            match s.encoding_name().to_ascii_lowercase().as_str()
            {
                "h264" => pick = Some((i, Codec::H264)),
                "jpeg" if pick.is_none() => pick = Some((i, Codec::Jpeg)),
                _ => {}
            }
        }
        let (video, codec) = pick.ok_or_else(|| AppError::Camera("no supported video stream (h264/jpeg)".into()))?;

        session
            .setup(video, SetupOptions::default().frame_format(FrameFormat::SIMPLE))
            .await
            .map_err(|e| AppError::Camera(format!("rtsp setup: {e}")))?;

        let playing = session
            .play(PlayOptions::default())
            .await
            .map_err(|e| AppError::Camera(format!("rtsp play: {e}")))?;
        let mut demuxed = playing
            .demuxed()
            .map_err(|e| AppError::Camera(format!("rtsp demux: {e}")))?;

        let mut decoder = match codec
        {
            Codec::H264 => Some(Decoder::new().map_err(|e| AppError::Camera(format!("h264 decoder: {e}")))?),
            Codec::Jpeg => None
        };
        tracing::info!(camera = %self.cfg.id, url = %self.cfg.url, codec = codec_name(codec), "rtsp connected, decoding video");

        let mut last_sent = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);

        loop
        {
            if stop.is_stopped()
            {
                return Ok(());
            }

            let item = match demuxed.next().await
            {
                Some(Ok(item)) => item,
                Some(Err(err)) => return Err(AppError::Camera(format!("rtsp stream: {err}"))),
                None => return Ok(())
            };

            let CodecItem::VideoFrame(frame) = item else { continue; };

            let decoded = match codec
            {
                Codec::H264 => decoder.as_mut().and_then(|d| decode_h264(d, frame.data())),
                Codec::Jpeg => decode_jpeg(frame.data())
            };
            let Some((data, w, h)) = decoded else { continue; };

            if last_sent.elapsed() < pace.interval()
            {
                continue;
            }
            last_sent = Instant::now();

            let out = Frame
            {
                camera_id: self.cfg.id.clone(),
                captured_at: SystemTime::now(),
                width: w,
                height: h,
                format: PixelFormat::Gray8,
                data
            };

            if sender.send(Some(out)).is_err()
            {
                return Ok(());
            }
        }
    }
}

fn codec_name(codec: Codec) -> &'static str
{
    match codec
    {
        Codec::H264 => "h264",
        Codec::Jpeg => "mjpeg"
    }
}

// Декодировать H.264 access unit (Annex B) и вернуть Y-плоскость как Gray8
fn decode_h264(decoder: &mut Decoder, au: &[u8]) -> Option<(Vec<u8>, u32, u32)>
{
    let mut latest: Option<(Vec<u8>, u32, u32)> = None;

    for nal in nal_units(au)
    {
        let Ok(Some(yuv)) = decoder.decode(nal) else { continue; };

        let (w, h) = yuv.dimensions();
        let (y_stride, _, _) = yuv.strides();
        let y = yuv.y();
        if y.len() < h.saturating_sub(1) * y_stride + w
        {
            continue;
        }

        let mut gray = vec![0u8; w * h];
        for row in 0..h
        {
            let src = row * y_stride;
            gray[row * w..row * w + w].copy_from_slice(&y[src..src + w]);
        }
        latest = Some((gray, w as u32, h as u32));
    }

    return latest;
}

// Декодировать один кадр MJPEG в Gray8 (берём последний SOI: retina иногда
// добавляет свой RFC2435-заголовок перед уже полным JFIF-кадром)
fn decode_jpeg(data: &[u8]) -> Option<(Vec<u8>, u32, u32)>
{
    let jpeg = &data[last_soi(data)..];
    let luma = image::load_from_memory(jpeg).ok()?.to_luma8();
    let (w, h) = (luma.width(), luma.height());
    return Some((luma.into_raw(), w, h));
}

// Смещение последнего маркера SOI (FF D8)
fn last_soi(data: &[u8]) -> usize
{
    let mut found = 0;
    let mut i = 0;
    while i + 1 < data.len()
    {
        if data[i] == 0xFF && data[i + 1] == 0xD8
        {
            found = i;
        }
        i += 1;
    }
    return found;
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
