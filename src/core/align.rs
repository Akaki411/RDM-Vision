use crate::data::{Frame, PixelFormat, Point, Region};

pub struct Cropper;

impl Cropper
{
    pub fn new() -> Self
    {
        return Self;
    }

    // Развертка области без полей
    pub fn crop(&self, frame: &Frame, region: &Region) -> Frame
    {
        let c = order_corners(region.corners);

        let side = [
            dist(c[0], c[1]),
            dist(c[1], c[2]),
            dist(c[2], c[3]),
            dist(c[3], c[0])
        ]
        .into_iter()
        .fold(0.0f32, f32::max)
        .round() as i32;
        let side = side.clamp(32, 1024) as usize;

        let mut data = vec![255u8; side * side];

        for oy in 0..side
        {
            let v = (oy as f32 + 0.5) / side as f32;
            for ox in 0..side
            {
                let u = (ox as f32 + 0.5) / side as f32;

                let sx = (1.0 - u) * (1.0 - v) * c[0].x
                    + u * (1.0 - v) * c[1].x
                    + u * v * c[2].x
                    + (1.0 - u) * v * c[3].x;
                let sy = (1.0 - u) * (1.0 - v) * c[0].y
                    + u * (1.0 - v) * c[1].y
                    + u * v * c[2].y
                    + (1.0 - u) * v * c[3].y;

                data[oy * side + ox] = sample(frame, sx, sy);
            }
        }

        return Frame
        {
            camera_id: frame.camera_id.clone(),
            captured_at: frame.captured_at,
            width: side as u32,
            height: side as u32,
            format: PixelFormat::Gray8,
            data
        };
    }

    // Та же область, но с белой тихой зоной вокруг, её ждёт ZXing
    pub fn quiet_zone(&self, crop: &Frame) -> Frame
    {
        let w = crop.width as usize;
        let h = crop.height as usize;
        let margin = (w.max(h) / 8).max(4);
        let ow = w + 2 * margin;
        let oh = h + 2 * margin;

        let mut data = vec![255u8; ow * oh];
        for y in 0..h
        {
            let src = y * w;
            let dst = (y + margin) * ow + margin;
            data[dst..dst + w].copy_from_slice(&crop.data[src..src + w]);
        }

        return Frame
        {
            camera_id: crop.camera_id.clone(),
            captured_at: crop.captured_at,
            width: ow as u32,
            height: oh as u32,
            format: PixelFormat::Gray8,
            data
        };
    }
}

// Упорядочить 4 угла как [ВЛ, ВП, НП, НЛ] по их положению
fn order_corners(c: [Point; 4]) -> [Point; 4]
{
    let cx = (c[0].x + c[1].x + c[2].x + c[3].x) / 4.0;
    let cy = (c[0].y + c[1].y + c[2].y + c[3].y) / 4.0;

    let mut ordered = c;
    ordered.sort_by(|a, b|
    {
        let aa = (a.y - cy).atan2(a.x - cx);
        let ab = (b.y - cy).atan2(b.x - cx);
        aa.total_cmp(&ab)
    });

    let mut start = 0;
    let mut best = ordered[0].x + ordered[0].y;
    for (i, p) in ordered.iter().enumerate().skip(1)
    {
        if p.x + p.y < best
        {
            best = p.x + p.y;
            start = i;
        }
    }
    ordered.rotate_left(start);
    return ordered;
}

fn dist(a: Point, b: Point) -> f32
{
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    return (dx * dx + dy * dy).sqrt();
}

// Билинейная выборка
fn sample(frame: &Frame, x: f32, y: f32) -> u8
{
    let w = frame.width as usize;
    let h = frame.height as usize;
    if x < 0.0 || y < 0.0 || x > (w - 1) as f32 || y > (h - 1) as f32
    {
        return 255;
    }

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let top = gray_at(frame, x0, y0) as f32 + (gray_at(frame, x1, y0) as f32 - gray_at(frame, x0, y0) as f32) * fx;
    let bot = gray_at(frame, x0, y1) as f32 + (gray_at(frame, x1, y1) as f32 - gray_at(frame, x0, y1) as f32) * fx;
    return (top + (bot - top) * fy).round().clamp(0.0, 255.0) as u8;
}

fn gray_at(frame: &Frame, x: usize, y: usize) -> u8
{
    let w = frame.width as usize;
    match frame.format
    {
        PixelFormat::Gray8 => frame.data[y * w + x],
        PixelFormat::Bgr8 =>
        {
            let i = (y * w + x) * 3;
            (0.114 * frame.data[i] as f32 + 0.587 * frame.data[i + 1] as f32 + 0.299 * frame.data[i + 2] as f32) as u8
        }
        PixelFormat::Rgb8 =>
        {
            let i = (y * w + x) * 3;
            (0.299 * frame.data[i] as f32 + 0.587 * frame.data[i + 1] as f32 + 0.114 * frame.data[i + 2] as f32) as u8
        }
    }
}

impl Default for Cropper
{
    fn default() -> Self
    {
        return Self::new();
    }
}
