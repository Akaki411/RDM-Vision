use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat
{
    Bgr8,
    Rgb8,
    Gray8
}

impl PixelFormat
{
    pub fn channels(self) -> usize
    {
        match self
        {
            PixelFormat::Bgr8 | PixelFormat::Rgb8 => 3,
            PixelFormat::Gray8 => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame
{
    pub camera_id: String,
    pub captured_at: SystemTime,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>
}

impl Frame
{
    pub fn expected_len(&self) -> usize
    {
        return self.width as usize * self.height as usize * self.format.channels();
    }
}
