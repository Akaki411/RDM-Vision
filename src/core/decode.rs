use std::collections::HashSet;

use rxing::{BarcodeFormat, DecodeHints};

use crate::config::RecognitionConfig;
use crate::data::{Frame, PixelFormat};

pub struct Reader
{
    cfg: RecognitionConfig
}

impl Reader
{
    pub fn new(cfg: &RecognitionConfig) -> Self
    {
        return Self { cfg: cfg.clone() };
    }

    // Быстрый проход по всему кадру, читает сразу все коды
    pub fn read_frame(&self, frame: &Frame) -> Vec<String>
    {
        let mut hints = self.hints();
        let luma = to_luma(frame);

        for variant in self.variants()
        {
            let Some((data, width, height)) = materialize(&variant, &luma, frame.width, frame.height)
            else
            {
                continue;
            };

            let texts = decode_multi(data, width, height, &mut hints);
            if !texts.is_empty()
            {
                return texts;
            }
        }
        return Vec::new();
    }

    // Тщательный проход по обрезанной области, ждём ровно один код
    pub fn read_roi(&self, frame: &Frame) -> Option<String>
    {
        let mut hints = self.hints();
        let luma = to_luma(frame);

        for variant in self.variants()
        {
            let Some((data, width, height)) = materialize(&variant, &luma, frame.width, frame.height)
            else
            {
                continue;
            };

            if let Some(text) = decode_single(data, width, height, &mut hints)
            {
                return Some(text);
            }
        }
        return None;
    }

    // Формат и опции try_harder / try_invert для RXing
    fn hints(&self) -> DecodeHints
    {
        let mut hints = DecodeHints::default();
        hints.PossibleFormats = Some(HashSet::from([BarcodeFormat::DATA_MATRIX]));
        if self.cfg.try_harder
        {
            hints.TryHarder = Some(true);
        }
        if self.cfg.try_invert
        {
            hints.AlsoInverted = Some(true);
        }
        return hints;
    }

    // Порядок попыток, оригинал, затем опциональные уменьшение и повороты
    fn variants(&self) -> Vec<Variant>
    {
        let mut variants = vec![Variant::Original];
        if self.cfg.try_downscale
        {
            variants.push(Variant::Downscale);
        }
        if self.cfg.try_rotate
        {
            variants.extend([Variant::Rotate90, Variant::Rotate180, Variant::Rotate270]);
        }
        return variants;
    }
}

// Варианты кадра, которые прогоняем через rxing при неудаче предыдущего
enum Variant
{
    Original,
    Downscale,
    Rotate90,
    Rotate180,
    Rotate270
}

// Построить буфер яркости для конкретного варианта
fn materialize(variant: &Variant, luma: &[u8], width: u32, height: u32) -> Option<(Vec<u8>, u32, u32)>
{
    match variant
    {
        Variant::Original => Some((luma.to_vec(), width, height)),
        Variant::Downscale => downscale(luma, width, height),
        Variant::Rotate90 => Some((rotate90(luma, width, height), height, width)),
        Variant::Rotate180 => Some((rotate180(luma), width, height)),
        Variant::Rotate270 => Some((rotate270(luma, width, height), height, width))
    }
}

// Прочитать все валидные DataMatrix в кадре
fn decode_multi(luma: Vec<u8>, width: u32, height: u32, hints: &mut DecodeHints) -> Vec<String>
{
    match rxing::helpers::detect_multiple_in_luma_with_hints(luma, width, height, hints)
    {
        Ok(results) => results
            .into_iter()
            .map(|r| r.getText().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        Err(_) => Vec::new()
    }
}

// Прочитать ровно один DataMatrix из обрезанной области
fn decode_single(luma: Vec<u8>, width: u32, height: u32, hints: &mut DecodeHints) -> Option<String>
{
    match rxing::helpers::detect_in_luma_with_hints(luma, width, height, Some(BarcodeFormat::DATA_MATRIX), hints)
    {
        Ok(result) =>
        {
            let text = result.getText().to_string();
            if text.is_empty() { None } else { Some(text) }
        }
        Err(_) => None
    }
}

// Уменьшение вдвое усреднением 2x2, None если сторона схлопывается в ноль
fn downscale(src: &[u8], width: u32, height: u32) -> Option<(Vec<u8>, u32, u32)>
{
    let (nw, nh) = (width / 2, height / 2);
    if nw == 0 || nh == 0
    {
        return None;
    }

    let (width, nw, nh) = (width as usize, nw as usize, nh as usize);
    let mut dst = vec![0u8; nw * nh];
    for y in 0..nh
    {
        for x in 0..nw
        {
            let (sx, sy) = (x * 2, y * 2);
            let sum = src[sy * width + sx] as u16
                + src[sy * width + sx + 1] as u16
                + src[(sy + 1) * width + sx] as u16
                + src[(sy + 1) * width + sx + 1] as u16;
            dst[y * nw + x] = (sum / 4) as u8;
        }
    }
    return Some((dst, nw as u32, nh as u32));
}

// Поворот на 90° по часовой стрелке, стороны меняются местами
fn rotate90(src: &[u8], width: u32, height: u32) -> Vec<u8>
{
    let (width, height) = (width as usize, height as usize);
    let mut dst = vec![0u8; width * height];
    for y in 0..height
    {
        for x in 0..width
        {
            dst[x * height + (height - 1 - y)] = src[y * width + x];
        }
    }
    return dst;
}

// Разворот буфера
fn rotate180(src: &[u8]) -> Vec<u8>
{
    let mut dst = src.to_vec();
    dst.reverse();
    return dst;
}

// Поворот на 270° по часовой стрелке, стороны меняются местами
fn rotate270(src: &[u8], width: u32, height: u32) -> Vec<u8>
{
    let (width, height) = (width as usize, height as usize);
    let mut dst = vec![0u8; width * height];
    for y in 0..height
    {
        for x in 0..width
        {
            dst[(width - 1 - x) * height + y] = src[y * width + x];
        }
    }
    return dst;
}

// Перевод кадра в буфер яркости для rxing
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
