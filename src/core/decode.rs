use zxingcpp::{BarcodeFormat, Binarizer, ImageFormat, ImageView};

use crate::data::{Frame, PixelFormat};

pub struct Reader;

impl Reader
{
    pub fn new() -> Self
    {
        return Self;
    }

    // Быстрый проход по всему кадру, читает сразу все коды
    pub fn read_frame(&self, frame: &Frame) -> Vec<String>
    {
        let reader = zxingcpp::read()
            .formats(BarcodeFormat::DataMatrix)
            .binarizer(Binarizer::LocalAverage)
            .try_harder(false)
            .try_rotate(true)
            .try_invert(false)
            .try_downscale(false);
        return decode_all(&reader, frame);
    }

    // Тщательный проход по обрезанной области, ждём ровно один код
    pub fn read_roi(&self, frame: &Frame) -> Option<String>
    {
        let reader = zxingcpp::read()
            .formats(BarcodeFormat::DataMatrix)
            .binarizer(Binarizer::LocalAverage)
            .try_harder(true)
            .try_rotate(true)
            .try_invert(true)
            .try_downscale(true)
            .max_number_of_symbols(1);
        return decode_all(&reader, frame).into_iter().next();
    }
}

// Прогнать ридер по кадру и собрать тексты валидных штрихкодов
fn decode_all(reader: &zxingcpp::BarcodeReader, frame: &Frame) -> Vec<String>
{
    let luma = to_luma(frame);
    let view = match ImageView::from_slice(&luma, frame.width, frame.height, ImageFormat::Lum)
    {
        Ok(view) => view,
        Err(_) => return Vec::new()
    };

    match reader.from(&view)
    {
        Ok(barcodes) => barcodes
            .into_iter()
            .filter(|b| b.is_valid())
            .map(|b| b.text())
            .filter(|t| !t.is_empty())
            .collect(),
        Err(_) => Vec::new()
    }
}

// Перевод кадра в буфер яркости для ZXing
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
