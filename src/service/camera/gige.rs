use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use viva_genicam::gige::nic::Iface;
use viva_genicam::gige::{self, DeviceInfo, GigeDevice, GVCP_PORT};
use viva_genicam::pfnc::PixelFormat as GigePixelFormat;
use viva_genicam::{connect_gige, FrameStream, StreamBuilder};

use crate::config::GigeConfig;
use crate::data::{Frame, PixelFormat};
use crate::error::{AppError, Result};

use super::base::{CamPace, Camera, FrameSender, Stop};

pub struct GigeCamera
{
    cfg: GigeConfig
}

impl GigeCamera
{
    pub fn new(cfg: GigeConfig) -> Self
    {
        return Self { cfg };
    }
}

impl Camera for GigeCamera
{
    fn id(&self) -> &str
    {
        return &self.cfg.id;
    }

    fn run(&mut self, sender: FrameSender, stop: Stop, pace: Arc<CamPace>) -> Result<()>
    {
        // viva-genicam асинхронна и требует многопоточный рантайм (GVCP-транзакции
        // ходят через block_in_place). Поднимаем свой в потоке камеры
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| AppError::Camera(format!("gige runtime: {e}")))?;

        let reconnect = Duration::from_secs(2);
        while !stop.is_stopped()
        {
            if let Err(err) = rt.block_on(self.session(&sender, &stop, &pace))
            {
                tracing::warn!(camera = %self.cfg.id, error = %err, "gige session ended, reconnecting");
            }
            if !stop.is_stopped()
            {
                sleep_stoppable(reconnect, &stop);
            }
        }
        return Ok(());
    }
}

impl GigeCamera
{
    // Полный цикл одного подключения: найти камеру, поднять поток, гнать кадры
    async fn session(&self, sender: &FrameSender, stop: &Stop, pace: &Arc<CamPace>) -> Result<()>
    {
        let device = self.locate().await?;
        tracing::info!(
            camera = %self.cfg.id,
            ip = %device.ip,
            model = device.model.as_deref().unwrap_or("gige"),
            "gige camera found"
        );

        // Управляющее подключение: тянет XML, строит nodemap
        let mut camera = connect_gige(&device)
            .await
            .map_err(|e| AppError::Camera(format!("gige connect: {e}")))?;

        // Отдельное подключение под настройку канала потока (как в примере крейта)
        let mut stream_device = GigeDevice::open(SocketAddr::new(IpAddr::V4(device.ip), GVCP_PORT))
            .await
            .map_err(|e| AppError::Camera(format!("gige stream open: {e}")))?;

        let iface = self.iface(device.ip)?;
        let stream = StreamBuilder::new(&mut stream_device)
            .iface(iface)
            .build()
            .await
            .map_err(|e| AppError::Camera(format!("gige stream build: {e}")))?;

        let time_sync = camera.time_sync().clone();
        let mut frames = FrameStream::new(stream, Some(time_sync));

        camera
            .acquisition_start()
            .map_err(|e| AppError::Camera(format!("gige acquisition start: {e}")))?;
        tracing::info!(camera = %self.cfg.id, "gige acquisition started");

        let mut last_sent = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);

        let result = loop
        {
            if stop.is_stopped()
            {
                break Ok(());
            }

            match frames.next_frame().await
            {
                Ok(Some(raw)) =>
                {
                    // Темп режем по режиму (cold/hot), как у RTSP
                    if last_sent.elapsed() < pace.interval()
                    {
                        continue;
                    }
                    last_sent = Instant::now();

                    let Some(frame) = convert(raw, &self.cfg.id) else { continue; };

                    // Пайплайн занят — кадр отбрасываем, свежий важнее очереди
                    match sender.try_send(frame)
                    {
                        Ok(()) | Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {}
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break Ok(())
                    }
                }
                Ok(None) => break Ok(()),
                Err(err) => break Err(AppError::Camera(format!("gige frame: {err}")))
            }
        };

        let _ = camera.acquisition_stop();
        return result;
    }

    // Камеру берём по адресу из конфига, иначе ищем в сети (discovery включает loopback)
    async fn locate(&self) -> Result<DeviceInfo>
    {
        let address = self.cfg.address.trim();
        if !address.is_empty()
        {
            let ip = Ipv4Addr::from_str(address)
                .map_err(|_| AppError::Camera(format!("bad gige address: {address}")))?;
            return Ok(DeviceInfo { ip, mac: [0u8; 6], model: None, manufacturer: None });
        }

        let mut found = gige::discover_all(Duration::from_millis(800))
            .await
            .map_err(|e| AppError::Camera(format!("gige discover: {e}")))?;
        if found.is_empty()
        {
            return Err(AppError::Camera("no gige cameras discovered".into()));
        }
        return Ok(found.remove(0));
    }

    // Интерфейс приёма GVSP: из конфига (IP или имя), иначе автоматически.
    // Для loopback-эмулятора по умолчанию берём локальную петлю
    fn iface(&self, device_ip: Ipv4Addr) -> Result<Iface>
    {
        let name = self.cfg.interface.trim();
        if !name.is_empty()
        {
            return match Ipv4Addr::from_str(name)
            {
                Ok(ip) => Iface::from_ipv4(ip).map_err(|e| AppError::Camera(format!("gige iface {ip}: {e}"))),
                Err(_) => Iface::from_system(name).map_err(|e| AppError::Camera(format!("gige iface {name}: {e}")))
            };
        }

        let ip = if device_ip.is_loopback() { Ipv4Addr::LOCALHOST } else { device_ip };
        return Iface::from_ipv4(ip).map_err(|e| AppError::Camera(format!("gige auto iface {ip}: {e}")));
    }
}

// Кадр GigE → внутренний Frame. Mono8 идёт как есть, остальное через RGB
fn convert(raw: viva_genicam::Frame, camera_id: &str) -> Option<Frame>
{
    let width = raw.width;
    let height = raw.height;
    let captured_at = raw.host_time().unwrap_or_else(SystemTime::now);

    let (format, data) = match raw.pixel_format
    {
        GigePixelFormat::Mono8 => (PixelFormat::Gray8, raw.payload.to_vec()),
        _ => match raw.to_rgb8()
        {
            Ok(rgb) => (PixelFormat::Rgb8, rgb),
            Err(err) =>
            {
                tracing::debug!(error = %err, "unsupported gige pixel format, skipping frame");
                return None;
            }
        }
    };

    let frame = Frame { camera_id: camera_id.to_string(), captured_at, width, height, format, data };

    // Битый/неполный кадр не пускаем дальше
    if frame.data.len() < frame.expected_len()
    {
        return None;
    }
    return Some(frame);
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
