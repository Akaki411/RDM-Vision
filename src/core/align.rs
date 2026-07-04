use crate::data::{Frame, PixelFormat, Point, Region};

pub struct Cropper;

impl Cropper
{
    pub fn new() -> Self
    {
        return Self;
    }

    pub fn crop(&self, frame: &Frame, region: &Region) -> Frame
    {
        // Углы модели могут идти в произвольном порядке — упорядочиваем их
        // геометрически (ВЛ, ВП, НП, НЛ), иначе код может отзеркалиться
        let c = order_corners(region.corners);

        // Сторона квадрата — по самой длинной стороне четырёхугольника
        let side = [
            dist(c[0], c[1]),
            dist(c[1], c[2]),
            dist(c[2], c[3]),
            dist(c[3], c[0])
        ]
        .into_iter()
        .fold(0.0f32, f32::max)
        .round() as i32;
        let side = side.clamp(32, 512) as usize;

        let margin = (side / 8).max(4);
        let out_size = side + 2 * margin;

        // Белый фон вокруг кода
        let mut data = vec![255u8; out_size * out_size];

        // Дальше уже нейронка что-то нашаманила, тут какой-то матан начинается, а я его не знаю
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

                data[(oy + margin) * out_size + ox + margin] = sample(frame, sx, sy);
            }
        }

        return Frame
        {
            camera_id: frame.camera_id.clone(),
            captured_at: frame.captured_at,
            width: out_size as u32,
            height: out_size as u32,
            format: PixelFormat::Gray8,
            data
        };
    }
}

// Упорядочить 4 угла как [ВЛ, ВП, НП, НЛ] по их положению
fn order_corners(c: [Point; 4]) -> [Point; 4]
{
    let tl = pick(&c, |p| p.x + p.y, false);
    let br = pick(&c, |p| p.x + p.y, true);
    let tr = pick(&c, |p| p.y - p.x, false);
    let bl = pick(&c, |p| p.y - p.x, true);
    return [tl, tr, br, bl];
}

fn pick<F: Fn(Point) -> f32>(c: &[Point; 4], key: F, want_max: bool) -> Point
{
    let mut best = c[0];
    let mut best_val = key(c[0]);
    for &p in &c[1..]
    {
        let v = key(p);
        if (want_max && v > best_val) || (!want_max && v < best_val)
        {
            best_val = v;
            best = p;
        }
    }
    return best;
}

fn dist(a: Point, b: Point) -> f32
{
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    return (dx * dx + dy * dy).sqrt();
}

fn sample(frame: &Frame, x: f32, y: f32) -> u8
{
    let w = frame.width as i32;
    let h = frame.height as i32;
    let xi = x.round() as i32;
    let yi = y.round() as i32;
    if xi < 0 || yi < 0 || xi >= w || yi >= h
    {
        return 255;
    }
    return gray_at(frame, xi as usize, yi as usize);
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
