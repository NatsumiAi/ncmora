const CELL_W_PX: u32 = 8;
const CELL_H_PX: u32 = 16;

pub fn cover_viewport(
    image_w: u32,
    image_h: u32,
    target_w: u16,
    target_h: u16,
) -> (u32, u32, u32, u32) {
    if target_w == 0 || target_h == 0 || image_w == 0 || image_h == 0 {
        return (0, 0, image_w.max(1), image_h.max(1));
    }

    let image_ratio = image_w as f64 / image_h as f64;
    // Cell geometry in terminal space: 2 columns ~= 1 row in physical size.
    // Keep image aspect visually correct by using cell pixel ratio for viewport fitting.
    let target_ratio = (target_w as f64 * CELL_W_PX as f64) / (target_h as f64 * CELL_H_PX as f64);

    if (image_ratio - target_ratio).abs() < f64::EPSILON {
        return (0, 0, image_w, image_h);
    }

    if image_ratio > target_ratio {
        let crop_w = ((image_h as f64) * target_ratio)
            .round()
            .clamp(1.0, image_w as f64) as u32;
        let crop_x = (image_w.saturating_sub(crop_w)) / 2;
        (crop_x, 0, crop_w, image_h)
    } else {
        let crop_h = ((image_w as f64) / target_ratio)
            .round()
            .clamp(1.0, image_h as f64) as u32;
        let crop_y = (image_h.saturating_sub(crop_h)) / 2;
        (0, crop_y, image_w, crop_h)
    }
}
