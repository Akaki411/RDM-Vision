use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Tensor;

use crate::config::DetectConfig;
use crate::data::{Frame, PixelFormat, Point, Region};
use crate::error::{AppError, Result};

pub struct Detector
{
    session: Session,
    input: usize,
    conf: f32
}

impl Detector
{
    // Загрузка модели из cfg.model_path
    pub fn new(cfg: DetectConfig) -> Result<Self>
    {
        let session = Session::builder()
            .and_then(|b| Ok(b.with_optimization_level(GraphOptimizationLevel::Level3)?))
            .and_then(|mut b| b.commit_from_file(&cfg.model_path))
            .map_err(|e| AppError::Detect(e.to_string()))?;

        return Ok(Self
        {
            session,
            input: cfg.input_size as usize,
            conf: cfg.confidence_threshold
        });
    }

    // Найти все области data matrix (по 4 угла). При ошибке — пустой список
    pub fn find(&mut self, frame: &Frame) -> Vec<Region>
    {
        // Кадр зашёл в YOLO
        tracing::debug!(w = frame.width, h = frame.height, "yolo reading frame");

        match self.run(frame)
        {
            Ok(regions) =>
            {
                tracing::debug!(regions = regions.len(), "yolo frame done");
                regions
            }
            Err(err) =>
            {
                tracing::warn!(error = %err, "detection failed");
                Vec::new()
            }
        }
    }

    fn run(&mut self, frame: &Frame) -> Result<Vec<Region>>
    {
        let s = self.input;

        // Приводим кадр ко входу модели
        let (gray, scale, pad_x, pad_y) = letterbox(frame, s);

        // Серый канал дублируем в RGB, нормируем в 0..1
        let area = s * s;
        let mut data = vec![0f32; 3 * area];
        for i in 0..area
        {
            let v = gray[i] as f32 / 255.0;
            data[i] = v;
            data[area + i] = v;
            data[2 * area + i] = v;
        }

        let tensor = Tensor::from_array(([1usize, 3, s, s], data))
            .map_err(|e| AppError::Detect(e.to_string()))?;

        let outputs = self
            .session
            .run(ort::inputs![tensor])
            .map_err(|e| AppError::Detect(e.to_string()))?;

        let (shape, out) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::Detect(e.to_string()))?;

        return Ok(parse(out, shape, self.conf, scale, pad_x, pad_y));
    }
}

// Преобразование вывода модели в адекватный формат
fn parse(out: &[f32], shape: &[i64], conf: f32, scale: f32, pad_x: f32, pad_y: f32) -> Vec<Region>
{
    if shape.len() != 3
    {
        return Vec::new();
    }
    let num = shape[1] as usize;
    let attrs = shape[2] as usize;
    if attrs < 18
    {
        return Vec::new();
    }

    let mut regions = Vec::new();
    for i in 0..num
    {
        let base = i * attrs;
        let score = out[base + 4];
        if score < conf
        {
            continue;
        }

        let mut corners = [Point { x: 0.0, y: 0.0 }; 4];
        for j in 0..4
        {
            let kx = out[base + 6 + j * 3];
            let ky = out[base + 7 + j * 3];
            corners[j] = Point
            {
                x: (kx - pad_x) / scale,
                y: (ky - pad_y) / scale
            };
        }

        regions.push(Region { score, corners });
    }
    return regions;
}

// Ресайз в квадрат size×size с сохранением пропорций и серым фоном
// Возвращает буфер, масштаб и отступы для обратного перевода координат
// Опять нейроночная тема
fn letterbox(frame: &Frame, size: usize) -> (Vec<u8>, f32, f32, f32)
{
    let w = frame.width as usize;
    let h = frame.height as usize;
    let scale = (size as f32 / w as f32).min(size as f32 / h as f32);
    let nw = ((w as f32 * scale).round() as usize).clamp(1, size);
    let nh = ((h as f32 * scale).round() as usize).clamp(1, size);
    let pad_x = ((size - nw) / 2) as f32;
    let pad_y = ((size - nh) / 2) as f32;

    let mut out = vec![114u8; size * size];
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

            let v = bilerp(
                gray_at(frame, x0, y0),
                gray_at(frame, x1, y0),
                gray_at(frame, x0, y1),
                gray_at(frame, x1, y1),
                fx,
                fy
            );
            out[(pad_y as usize + dy) * size + pad_x as usize + dx] = v;
        }
    }
    return (out, scale, pad_x, pad_y);
}

fn bilerp(v00: f32, v10: f32, v01: f32, v11: f32, fx: f32, fy: f32) -> u8
{
    let top = v00 + (v10 - v00) * fx;
    let bot = v01 + (v11 - v01) * fx;
    return (top + (bot - top) * fy).round().clamp(0.0, 255.0) as u8;
}

fn gray_at(frame: &Frame, x: usize, y: usize) -> f32
{
    let w = frame.width as usize;
    match frame.format
    {
        PixelFormat::Gray8 => frame.data[y * w + x] as f32,
        PixelFormat::Bgr8 =>
        {
            let i = (y * w + x) * 3;
            0.114 * frame.data[i] as f32 + 0.587 * frame.data[i + 1] as f32 + 0.299 * frame.data[i + 2] as f32
        }
        PixelFormat::Rgb8 =>
        {
            let i = (y * w + x) * 3;
            0.299 * frame.data[i] as f32 + 0.587 * frame.data[i + 1] as f32 + 0.114 * frame.data[i + 2] as f32
        }
    }
}
