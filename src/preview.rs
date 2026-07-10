use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use minifb::{Key, Window, WindowOptions};

use crate::data::{Frame, PixelFormat, Point, Region};

struct Shot
{
    w: usize,
    h: usize,
    gray: Vec<u8>,
    regions: Vec<Region>
}

struct Crop
{
    w: usize,
    h: usize,
    gray: Vec<u8>
}

// Это крч модуль для того, чтобы понимать что у нас происходит в пайплайне с картинками
pub struct Preview
{
    sender: Option<SyncSender<Shot>>,
    crop_sender: Option<SyncSender<Crop>>,
    restored_sender: Option<SyncSender<Crop>>,
    _window: Option<JoinHandle<()>>
}

impl Preview
{
    pub fn new(enabled: bool) -> Self
    {
        if !enabled
        {
            return Self { sender: None, crop_sender: None, restored_sender: None, _window: None };
        }

        // Свежий кадр важнее, канал держим коротким
        let (sender, receiver) = sync_channel::<Shot>(2);
        let (crop_sender, crop_receiver) = sync_channel::<Crop>(2);
        let (restored_sender, restored_receiver) = sync_channel::<Crop>(2);
        let window = std::thread::Builder::new()
            .name("preview-window".into())
            .spawn(move || run_window(receiver, crop_receiver, restored_receiver))
            .expect("failed to spawn preview window");

        return Self
        {
            sender: Some(sender),
            crop_sender: Some(crop_sender),
            restored_sender: Some(restored_sender),
            _window: Some(window)
        };
    }

    // Отправить кадр и области в окно, рисует уже поток окна
    pub fn show(&self, frame: &Frame, regions: &[Region])
    {
        let Some(sender) = &self.sender else { return; };

        let w = frame.width as usize;
        let h = frame.height as usize;
        if w == 0 || h == 0
        {
            return;
        }

        // При заполненном канале старый кадр просто отбрасываем
        match sender.try_send(Shot { w, h, gray: to_gray(frame), regions: regions.to_vec() })
        {
            Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
    }

    // Отправить в окно область «до восстановления» (левая врезка)
    pub fn show_crop(&self, crop: &Frame)
    {
        send_crop(self.crop_sender.as_ref(), crop);
    }

    // Отправить в окно область «после восстановления» (правая врезка)
    pub fn show_restored(&self, crop: &Frame)
    {
        send_crop(self.restored_sender.as_ref(), crop);
    }
}

// Общая отправка области во врезку, старую при заполнении канала отбрасываем
fn send_crop(sender: Option<&SyncSender<Crop>>, crop: &Frame)
{
    let Some(sender) = sender else { return; };

    let w = crop.width as usize;
    let h = crop.height as usize;
    if w == 0 || h == 0
    {
        return;
    }

    match sender.try_send(Crop { w, h, gray: to_gray(crop) })
    {
        Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
    }
}

impl Default for Preview
{
    fn default() -> Self
    {
        return Self::new(true);
    }
}

// Цикл окна: ждём первый кадр, открываем окно, крутим поток
fn run_window(receiver: Receiver<Shot>, crop_receiver: Receiver<Crop>, restored_receiver: Receiver<Crop>)
{
    // Размер окна берём с первого кадра
    let first = match receiver.recv()
    {
        Ok(shot) => shot,
        Err(_) => return
    };

    let mut window = match Window::new("RDM-Vision preview", first.w, first.h, WindowOptions::default())
    {
        Ok(win) => win,
        Err(err) =>
        {
            tracing::warn!(error = %err, "failed to open preview window");
            return;
        }
    };
    window.set_target_fps(60);

    let (w, h) = (first.w, first.h);
    let mut pixels = render(&first);

    // Счётчик кадров окна
    let mut fps = 0u32;
    let mut shown = 0u32;
    let mut mark = Instant::now();

    // Последние области для врезок: до и после восстановления
    let mut crop_before: Option<Crop> = None;
    let mut crop_after: Option<Crop> = None;

    while window.is_open() && !window.is_key_down(Key::Escape)
    {
        // Догоняем до самого свежего кадра
        loop
        {
            match receiver.try_recv()
            {
                Ok(shot) =>
                {
                    pixels = render(&shot);
                    shown += 1;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return
            }
        }

        // Догоняем до самой свежей области «до восстановления»
        loop
        {
            match crop_receiver.try_recv()
            {
                Ok(shot) => crop_before = Some(shot),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break
            }
        }

        // ...и «после восстановления»
        loop
        {
            match restored_receiver.try_recv()
            {
                Ok(shot) => crop_after = Some(shot),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break
            }
        }

        // Раз в секунду обновляем fps
        if mark.elapsed() >= Duration::from_secs(1)
        {
            fps = shown;
            shown = 0;
            mark = Instant::now();
        }

        draw_fps(&mut pixels, w, h, fps);
        draw_insets(&mut pixels, w, h, crop_before.as_ref(), crop_after.as_ref());

        if window.update_with_buffer(&pixels, w, h).is_err()
        {
            break;
        }
    }
}

// Собрать буфер окна из кадра
fn render(shot: &Shot) -> Vec<u32>
{
    let mut buf = Vec::with_capacity(shot.gray.len());
    for &v in &shot.gray
    {
        let v = v as u32;
        buf.push((v << 16) | (v << 8) | v);
    }
    for region in &shot.regions
    {
        draw_quad(&mut buf, shot.w, shot.h, &region.corners);
    }
    return buf;
}

// Четырёхугольник по 4 углам зелёными линиями
fn draw_quad(buf: &mut [u32], w: usize, h: usize, corners: &[Point; 4])
{
    for i in 0..4
    {
        draw_line(buf, w, h, corners[i], corners[(i + 1) % 4]);
    }
}

// Линия по алгоритму Брезенхэма (я хз что это, нейронка предложила)
fn draw_line(buf: &mut [u32], w: usize, h: usize, a: Point, b: Point)
{
    let mut x0 = a.x as i64;
    let mut y0 = a.y as i64;
    let x1 = b.x as i64;
    let y1 = b.y as i64;

    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop
    {
        put_pixel(buf, w, h, x0, y0);
        if x0 == x1 && y0 == y1
        {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy
        {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx
        {
            err += dx;
            y0 += sy;
        }
    }
}

fn put_pixel(buf: &mut [u32], w: usize, h: usize, x: i64, y: i64)
{
    if x < 0 || y < 0 || x as usize >= w || y as usize >= h
    {
        return;
    }
    buf[y as usize * w + x as usize] = 0x0000_FF00;
}

// Кадр в буфер яркости
fn to_gray(frame: &Frame) -> Vec<u8>
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

// Две врезки в правом нижнем углу: слева область до восстановления (серая рамка),
// справа — после восстановления (зелёная рамка)
fn draw_insets(buf: &mut [u32], w: usize, h: usize, before: Option<&Crop>, after: Option<&Crop>)
{
    let margin = 8usize;
    let gap = 6usize;
    let side = (w.min(h) / 3).clamp(56, 140);
    if side + margin + 2 >= w || side + margin + 2 >= h
    {
        return;
    }

    let y0 = h - side - margin;

    // Правая врезка — в самом углу, «после»
    let x_after = w - side - margin;
    if let Some(c) = after
    {
        draw_inset_at(buf, w, h, c, x_after, y0, side, 0x0000_FF00);
    }

    // Левая врезка — вплотную слева, «до». Рисуем только если хватает ширины
    if x_after >= side + gap + 1
    {
        let x_before = x_after - side - gap;
        if let Some(c) = before
        {
            draw_inset_at(buf, w, h, c, x_before, y0, side, 0x00A0_A0A0);
        }
    }
}

// Одна врезка: область масштабируем ближайшим соседом и обводим рамкой
fn draw_inset_at(buf: &mut [u32], w: usize, h: usize, crop: &Crop, x0: usize, y0: usize, side: usize, border: u32)
{
    if crop.w == 0 || crop.h == 0 || x0 + side > w || y0 + side > h
    {
        return;
    }

    for dy in 0..side
    {
        let sy = dy * crop.h / side;
        for dx in 0..side
        {
            let sx = dx * crop.w / side;
            let v = crop.gray[sy * crop.w + sx] as u32;
            buf[(y0 + dy) * w + x0 + dx] = (v << 16) | (v << 8) | v;
        }
    }

    if x0 >= 1 && y0 >= 1
    {
        draw_border(buf, w, h, x0 - 1, y0 - 1, side + 2, side + 2, border);
    }
}

// Прямоугольная рамка в 1 пиксель
fn draw_border(buf: &mut [u32], w: usize, h: usize, x: usize, y: usize, rw: usize, rh: usize, color: u32)
{
    for xx in x..(x + rw).min(w)
    {
        if y < h
        {
            buf[y * w + xx] = color;
        }
        let yb = y + rh - 1;
        if yb < h
        {
            buf[yb * w + xx] = color;
        }
    }
    for yy in y..(y + rh).min(h)
    {
        if x < w
        {
            buf[yy * w + x] = color;
        }
        let xr = x + rw - 1;
        if xr < w
        {
            buf[yy * w + xr] = color;
        }
    }
}

// Надпись fps на тёмной подложке в левом верхнем углу
fn draw_fps(buf: &mut [u32], w: usize, h: usize, fps: u32)
{
    let text = format!("fps: {fps}");
    let scale = 2usize;
    let pad = 3usize;
    let box_w = text.len() * 6 * scale + pad * 2;
    let box_h = 7 * scale + pad * 2;

    fill_rect(buf, w, h, 0, 0, box_w, box_h, 0x0000_0000);
    draw_text(buf, w, h, pad, pad, &text, scale, 0x00FF_FFFF);
}

// Залить прямоугольник
fn fill_rect(buf: &mut [u32], w: usize, h: usize, x: usize, y: usize, rw: usize, rh: usize, color: u32)
{
    for yy in y..(y + rh).min(h)
    {
        for xx in x..(x + rw).min(w)
        {
            buf[yy * w + xx] = color;
        }
    }
}

// Отрисовать строку пиксельным шрифтом
fn draw_text(buf: &mut [u32], w: usize, h: usize, x: usize, y: usize, text: &str, scale: usize, color: u32)
{
    let mut cx = x;
    for ch in text.chars()
    {
        if let Some(rows) = glyph(ch)
        {
            for (ry, row) in rows.iter().enumerate()
            {
                for (rx, cell) in row.bytes().enumerate()
                {
                    if cell != b'#'
                    {
                        continue;
                    }
                    for sy in 0..scale
                    {
                        for sx in 0..scale
                        {
                            let px = cx + rx * scale + sx;
                            let py = y + ry * scale + sy;
                            if px < w && py < h
                            {
                                buf[py * w + px] = color;
                            }
                        }
                    }
                }
            }
        }
        cx += 6 * scale;
    }
}

// Пиксельный шрифт 5x7, только нужные символы
fn glyph(c: char) -> Option<[&'static str; 7]>
{
    let g = match c
    {
        '0' => [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."],
        '1' => ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
        '2' => [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
        '3' => ["####.", "....#", "....#", ".###.", "....#", "....#", "####."],
        '4' => ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
        '5' => ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
        '6' => [".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."],
        '7' => ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."],
        '8' => [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
        '9' => [".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."],
        'f' => ["..###", ".#...", ".#...", "###..", ".#...", ".#...", ".#..."],
        'p' => ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."],
        's' => [".####", "#....", "#....", ".###.", "....#", "....#", "####."],
        ':' => [".....", "..#..", "..#..", ".....", "..#..", "..#..", "....."],
        _ => return None
    };
    return Some(g);
}
