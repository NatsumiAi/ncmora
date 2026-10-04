use crate::tmplayer::render::cover_cache::CoverKey;
use image::imageops::FilterType;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidget;
use ratatui_image::{Resize, StatefulImage, picker::Picker};
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

/// Bounded background preparation using ratatui-image's chafa-backed Halfblocks protocol.
/// No terminal capability query or escape-sequence image protocol is used.
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
    // Match the previous Halfblocks overlay: crop to fill, resize in font pixels,
    // then render StatefulImage with Crop. Feeding chafa a 1x2 sample grid instead
    // loses the richer glyph palette selected by the linked ratatui-image backend.
    let picker = Picker::halfblocks();
    let font = picker.font_size();
    let pixels = image.crop_imm(x, y, w, h).resize_exact(
        u32::from(width) * u32::from(font.width),
        u32::from(height) * u32::from(font.height),
        FilterType::Triangle,
    );
    let area = Rect::new(0, 0, width, height);
    let mut buffer = Buffer::empty(area);
    let mut protocol = picker.new_resize_protocol(pixels);
    StatefulImage::default().resize(Resize::Crop(None)).render(area, &mut buffer, &mut protocol);
    Some(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn prepared_cells_match_baseline_stateful_halfblocks_charset_and_colors() {
        use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
        use ratatui::{Terminal, backend::TestBackend};
        use ratatui_image::picker::ProtocolType;

        for (image_width, image_height) in [(31, 17), (17, 31), (32, 32)] {
            let image = DynamicImage::ImageRgba8(RgbaImage::from_fn(image_width, image_height, |x, y| {
                Rgba([(x * 31 % 256) as u8, (y * 47 % 256) as u8,
                    ((x + y) * 23 % 256) as u8, if (x + y) % 5 == 0 { 0 } else { 255 }])
            }));
            let mut encoded = Cursor::new(Vec::new());
            image.write_to(&mut encoded, image::ImageFormat::Png).unwrap();
            for (width, height) in [(1, 1), (12, 6), (9, 7)] {
                // Baseline 7961505 GraphicsOverlay Halfblocks path, with its
                // query-free fallback Picker. Keep this independent of prepare().
                let mut picker = Picker::halfblocks();
                picker.set_protocol_type(ProtocolType::Halfblocks);
                let font = picker.font_size();
                let (iw, ih) = image.dimensions();
                let ratio = f64::from(width) * f64::from(font.width)
                    / (f64::from(height) * f64::from(font.height));
                let (x, y, w, h) = if f64::from(iw) / f64::from(ih) > ratio {
                    let w = (f64::from(ih) * ratio).round().clamp(1.0, f64::from(iw)) as u32;
                    ((iw - w) / 2, 0, w, ih)
                } else {
                    let h = (f64::from(iw) / ratio).round().clamp(1.0, f64::from(ih)) as u32;
                    (0, (ih - h) / 2, iw, h)
                };
                let cropped = image.crop_imm(x, y, w, h).resize_exact(
                    u32::from(width) * u32::from(font.width),
                    u32::from(height) * u32::from(font.height), FilterType::Triangle,
                );
                let mut protocol = picker.new_resize_protocol(cropped);
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal.draw(|frame| {
                    frame.render_stateful_widget(
                        StatefulImage::default().resize(Resize::Crop(None)), frame.area(), &mut protocol,
                    );
                }).unwrap();
                let actual = prepare(encoded.get_ref(), width, height).unwrap();
                let expected = terminal.backend().buffer();
                for y in 0..height {
                    for x in 0..width {
                        assert_eq!(actual[(x, y)].symbol(), expected[(x, y)].symbol());
                        assert_eq!(actual[(x, y)].fg, expected[(x, y)].fg);
                        assert_eq!(actual[(x, y)].bg, expected[(x, y)].bg);
                    }
                }
            }
        }
    }
}
