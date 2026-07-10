// Дисперсия Лапласиана
pub fn laplacian_variance(gray: &[u8], w: usize, h: usize) -> f32
{
    if w < 3 || h < 3 || gray.len() < w * h
    {
        return 0.0;
    }

    let mut sum = 0f64;
    let mut sum_sq = 0f64;
    let mut n = 0f64;

    for y in 1..h - 1
    {
        for x in 1..w - 1
        {
            let c = gray[y * w + x] as i32;
            let lap = gray[y * w + x - 1] as i32
                + gray[y * w + x + 1] as i32
                + gray[(y - 1) * w + x] as i32
                + gray[(y + 1) * w + x] as i32
                - 4 * c;
            let lap = lap as f64;
            sum += lap;
            sum_sq += lap * lap;
            n += 1.0;
        }
    }

    if n == 0.0
    {
        return 0.0;
    }

    let mean = sum / n;
    return (sum_sq / n - mean * mean) as f32;
}
