use rxing::{DecodeHintValue, DecodeHints};
use crate::data::{Frame, PixelFormat};

pub struct Reader;

impl Reader
{
    pub fn new() -> Self
    {
        return Self;
    }

    // Прочитать код из выпрямленной области. None — прочитать не удалось
    pub fn read(&self, frame: &Frame) -> Option<String>
    {
        let luma = to_luma(frame);
        let mut hints = DecodeHints::default().with(DecodeHintValue::TryHarder(true));
        let result = rxing::helpers::detect_in_luma_with_hints(
            luma,
            frame.width,
            frame.height,
            Some(rxing::BarcodeFormat::DATA_MATRIX),
            &mut hints
        );
        match result
        {
            Ok(res) => Some(res.getText().to_string()),
            Err(_) => None
        }
    }
}

// Перевод кадра в буфер яркости для RXing
fn to_luma(frame: &Frame) -> Vec<u8>
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

impl Default for Reader
{
    fn default() -> Self
    {
        return Self::new();
    }
}
