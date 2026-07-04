use crate::config::NormConfig;
use crate::data::{Frame, PixelFormat};

pub struct Prepare
{
    contrast: f32
}

impl Prepare
{
    pub fn new(cfg: &NormConfig) -> Self
    {
        return Self { contrast: cfg.contrast };
    }

    // Вернуть новый ч/б кадр с применённым контрастом
    pub fn run(&self, frame: &Frame) -> Frame
    {
        let count = (frame.width * frame.height) as usize;
        let mut out = Vec::with_capacity(count);

        match frame.format
        {
            PixelFormat::Gray8 =>
            {
                if self.contrast != 1.0
                {
                    for &v in &frame.data
                    {
                        out.push(self.contrast_of(v as f32));
                    }
                }
                else
                {
                    out.extend_from_slice(&frame.data);
                }
            }
            PixelFormat::Bgr8 =>
            {
                for px in frame.data.chunks_exact(3)
                {
                    let gray = 0.114 * px[0] as f32 + 0.587 * px[1] as f32 + 0.299 * px[2] as f32;
                    if self.contrast != 1.0
                    {
                        out.push(self.contrast_of(gray));
                    }
                    else
                    {
                        out.push(gray as u8);
                    }
                }
            }
            PixelFormat::Rgb8 =>
            {
                for px in frame.data.chunks_exact(3)
                {
                    let gray = 0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32;
                    if self.contrast != 1.0
                    {
                        out.push(self.contrast_of(gray));
                    }
                    else
                    {
                        out.push(gray as u8);
                    }
                }
            }
        }

        return Frame
        {
            camera_id: frame.camera_id.clone(),
            captured_at: frame.captured_at,
            width: frame.width,
            height: frame.height,
            format: PixelFormat::Gray8,
            data: out
        };
    }

    // Установить контраст
    fn contrast_of(&self, value: f32) -> u8
    {
        let out = (value - 128.0) * self.contrast + 128.0;
        return out.clamp(0.0, 255.0) as u8;
    }
}
