use std::io::Cursor;
use std::time::{Duration, SystemTime};

use image::{DynamicImage, GrayImage, ImageFormat};
use tonic::transport::Channel;

use crate::config::RestoreConfig;
use crate::data::{Frame, PixelFormat};

use super::proto::irec_client::IrecClient;
use super::proto::RecoveryRequest;
const SEND_MIME: &str = "image/png";

pub struct RestoreClient
{
    client: IrecClient<Channel>
}

impl RestoreClient
{
    // Канал не падает, если сервис ещё не поднят
    pub fn new(cfg: &RestoreConfig) -> Self
    {
        let channel = Channel::from_shared(cfg.endpoint.clone())
            .expect("restore service endpoint is not a valid URI")
            .timeout(Duration::from_millis(cfg.timeout_ms))
            .connect_lazy();

        return Self { client: IrecClient::new(channel) };
    }

    // Отправить область кода и получить восстановленную
    pub async fn fix(&self, image: &Frame) -> Option<Frame>
    {
        let encoded = encode(image)?;

        let request = RecoveryRequest
        {
            image: encoded,
            mime_type: SEND_MIME.to_string()
        };

        let mut client = self.client.clone();
        let response = match client.recovery_image(request).await
        {
            Ok(resp) => resp.into_inner(),
            Err(status) =>
            {
                tracing::warn!(error = %status, "restore call failed");
                return None;
            }
        };

        if response.image.is_empty()
        {
            tracing::debug!("restore returned empty image, skipping");
            return None;
        }

        return decode(&response.image, &image.camera_id, image.captured_at);
    }
}

// Кадр (ч/б) в PNG
fn encode(frame: &Frame) -> Option<Vec<u8>>
{
    let gray = to_gray(frame);
    let img = GrayImage::from_raw(frame.width, frame.height, gray)?;

    let mut buf = Vec::new();
    DynamicImage::ImageLuma8(img)
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .ok()?;
    return Some(buf);
}

// Ответ сервиса обратно в ч/б кадр
fn decode(bytes: &[u8], camera_id: &str, captured_at: SystemTime) -> Option<Frame>
{
    let img = image::load_from_memory(bytes).ok()?;
    let luma = img.to_luma8();
    let (w, h) = (luma.width(), luma.height());

    return Some(Frame
    {
        camera_id: camera_id.to_string(),
        captured_at,
        width: w,
        height: h,
        format: PixelFormat::Gray8,
        data: luma.into_raw()
    });
}

// Данные кадра как непрерывный ч/б буфер
fn to_gray(frame: &Frame) -> Vec<u8>
{
    match frame.format
    {
        PixelFormat::Gray8 => frame.data.clone(),
        PixelFormat::Bgr8 => frame
            .data
            .chunks_exact(3)
            .map(|p| (0.114 * p[0] as f32 + 0.587 * p[1] as f32 + 0.299 * p[2] as f32) as u8)
            .collect(),
        PixelFormat::Rgb8 => frame
            .data
            .chunks_exact(3)
            .map(|p| (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) as u8)
            .collect()
    }
}
