use crate::tmplayer::render::cover_cache::CoverKey;
use image::imageops::FilterType;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

#[derive(Debug)]
struct Request {
    key: CoverKey,
    bytes: Vec<u8>,
}
#[derive(Debug)]
struct ResultFrame {
    key: CoverKey,
    frame: Option<Buffer>,
}

/// Bounded background preparation of color half-block covers. No terminal protocol is used.
#[derive(Debug)]
pub struct HalfblockCovers {
    tx: SyncSender<Request>,
    rx: Receiver<ResultFrame>,
    pending: HashSet<CoverKey>,
    frames: HashMap<CoverKey, Option<Buffer>>,
    order: VecDeque<CoverKey>,
}

impl HalfblockCovers {
    pub fn new() -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(2);
        let (results, rx) = mpsc::sync_channel::<ResultFrame>(2);
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let frame = prepare(&request.bytes, request.key.width, request.key.height);
                if results
                    .send(ResultFrame {
                        key: request.key,
                        frame,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            tx,
            rx,
            pending: HashSet::new(),
            frames: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(result) = self.rx.try_recv() {
            self.pending.remove(&result.key);
            self.frames.insert(result.key, result.frame);
            self.order.push_back(result.key);
            while self.order.len() > 8 {
                if let Some(old) = self.order.pop_front() {
                    self.frames.remove(&old);
                }
            }
            changed = true;
        }
        changed
    }

    pub fn paint(&mut self, target: &mut Buffer, area: Rect, hash: u64, bytes: &[u8]) {
        self.paint_segment(target, area, area, 0, hash, bytes);
    }

    /// Translate a full cached content frame, clipping only its destination.
    /// Preparing a cropped segment would change both the image viewport and cache key.
    pub fn paint_segment(
        &mut self,
        target: &mut Buffer,
        area: Rect,
        clip: Rect,
        dx: i16,
        hash: u64,
        bytes: &[u8],
    ) {
        if area.is_empty() {
            return;
        }
        let key = CoverKey {
            hash,
            width: area.width,
            height: area.height,
        };
        if let Some(frame) = self.frames.get(&key) {
            if let Some(frame) = frame {
                let clip = clip.intersection(target.area);
                for y in 0..area.height {
                    let dest_y = u32::from(area.y) + u32::from(y);
                    if dest_y < u32::from(clip.y) || dest_y >= u32::from(clip.bottom()) {
                        continue;
                    }
                    for x in 0..area.width {
                        let dest_x = i32::from(area.x) + i32::from(x) + i32::from(dx);
                        if dest_x < i32::from(clip.x) || dest_x >= i32::from(clip.right()) {
                            continue;
                        }
                        if let Some(cell) = target.cell_mut((dest_x as u16, dest_y as u16)) {
                            *cell = frame[(x, y)].clone();
                        }
                    }
                }
            }
            return;
        }
        if self.pending.len() < 2 && !self.pending.contains(&key) {
            let request = Request {
                key,
                bytes: bytes.to_vec(),
            };
            match self.tx.try_send(request) {
                Ok(()) => {
                    self.pending.insert(key);
                }
                Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {}
            }
        }
    }
}

fn prepare(bytes: &[u8], width: u16, height: u16) -> Option<Buffer> {
    if width == 0 || height == 0 {
        return None;
    }
    let image = image::load_from_memory(bytes).ok()?;
    let (x, y, w, h) = crate::render::graphics_overlay::cover_viewport(
        image.width(),
        image.height(),
        width,
        height,
    );
    let pixels = image
        .crop_imm(x, y, w, h)
        .resize_exact(width.into(), u32::from(height) * 2, FilterType::Triangle)
        .to_rgb8();
    let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
    for y in 0..height {
        for x in 0..width {
            let top = pixels.get_pixel(x.into(), u32::from(y) * 2).0;
            let bottom = pixels.get_pixel(x.into(), u32::from(y) * 2 + 1).0;
            buffer[(x, y)].set_symbol("▀").set_style(
                Style::default()
                    .fg(Color::Rgb(top[0], top[1], top[2]))
                    .bg(Color::Rgb(bottom[0], bottom[1], bottom[2])),
            );
        }
    }
    Some(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn prepared_halfblocks_preserve_distinct_top_and_bottom_colors() {
        let mut image = image::RgbImage::new(1, 2);
        image.put_pixel(0, 0, image::Rgb([255, 0, 0]));
        image.put_pixel(0, 1, image::Rgb([0, 0, 255]));
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let frame = prepare(encoded.get_ref(), 1, 1).unwrap();
        let cell = &frame[(0, 0)];
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Rgb(0, 0, 255));
    }
}
