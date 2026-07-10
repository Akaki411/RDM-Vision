use crate::config::NormConfig;
use crate::data::{Frame, PixelFormat};

pub struct Prepare
{
    contrast: f32,
    target: u32
}

impl Prepare
{
    pub fn new(cfg: &NormConfig) -> Self
    {
        return Self { contrast: cfg.contrast, target: cfg.target_size };
    }

    // Вернуть новый ч/б кадр
    pub fn run(&self, frame: &Frame) -> Frame
    {
        let (mut gray, mut w, mut h) = to_gray(frame);

        // Большие кадры ужимаем до target_size — RXing и YOLO быстрее, деталей хватает
        if self.target > 0 && w.max(h) > self.target as usize
        {
            let scale = self.target as f32 / w.max(h) as f32;
            let nw = ((w as f32 * scale).round() as usize).max(1);
            let nh = ((h as f32 * scale).round() as usize).max(1);
            gray = resize(&gray, w, h, nw, nh);
            w = nw;
            h = nh;
        }

        if self.contrast != 1.0
        {
            stretch(&mut gray, self.contrast);
        }

        return Frame
        {
            camera_id: frame.camera_id.clone(),
            captured_at: frame.captured_at,
            width: w as u32,
            height: h as u32,
            format: PixelFormat::Gray8,
            data: gray
        };
    }
}

// Перевод кадра в буфер яркости
fn to_gray(frame: &Frame) -> (Vec<u8>, usize, usize)
{
    let w = frame.width as usize;
    let h = frame.height as usize;
    let gray = match frame.format
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
    };
    return (gray, w, h);
}

// Растяжение контраста
fn stretch(gray: &mut [u8], max_gain: f32)
{
    let mut hist = [0u32; 256];
    for &v in gray.iter()
    {
        hist[v as usize] += 1;
    }

    let total = gray.len() as u32;
    let cut = total / 100;

    // Нижний и верхний перцентили
    let mut low = 0usize;
    let mut acc = 0u32;
    while low < 255 && acc + hist[low] < cut
    {
        acc += hist[low];
        low += 1;
    }

    let mut high = 255usize;
    acc = 0;
    while high > low && acc + hist[high] < cut
    {
        acc += hist[high];
        high -= 1;
    }

    if high <= low
    {
        return;
    }

    let gain = (255.0 / (high - low) as f32).min(max_gain.max(1.0));
    let mid = (low + high) as f32 / 2.0;

    // Таблица переходов вместо расчёта на каждый пиксель
    let mut lut = [0u8; 256];
    for (v, out) in lut.iter_mut().enumerate()
    {
        *out = ((v as f32 - mid) * gain + 128.0).clamp(0.0, 255.0) as u8;
    }

    for v in gray.iter_mut()
    {
        *v = lut[*v as usize];
    }
}

// Билинейный ресайз серого буфера
fn resize(src: &[u8], w: usize, h: usize, nw: usize, nh: usize) -> Vec<u8>
{
    let mut out = vec![0u8; nw * nh];
    for dy in 0..nh
    {
        let sy = ((dy as f32 + 0.5) * h as f32 / nh as f32 - 0.5).clamp(0.0, (h - 1) as f32);
        let y0 = sy.floor() as usize;
        let y1 = (y0 + 1).min(h - 1);
        let fy = sy - y0 as f32;
        for dx in 0..nw
        {
            let sx = ((dx as f32 + 0.5) * w as f32 / nw as f32 - 0.5).clamp(0.0, (w - 1) as f32);
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(w - 1);
            let fx = sx - x0 as f32;

            let top = src[y0 * w + x0] as f32 + (src[y0 * w + x1] as f32 - src[y0 * w + x0] as f32) * fx;
            let bot = src[y1 * w + x0] as f32 + (src[y1 * w + x1] as f32 - src[y1 * w + x0] as f32) * fx;
            out[dy * nw + dx] = (top + (bot - top) * fy).round().clamp(0.0, 255.0) as u8;
        }
    }
    return out;
}
