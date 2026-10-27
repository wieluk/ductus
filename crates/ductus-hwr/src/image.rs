//! What the recognizers do to line and formula images before reading them.

use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

/// The box around pixels whose mean channel is below `threshold` (0–255), as (x, y, w, h).
pub fn ink_bounds(image: &Pixmap, threshold: u32) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = (image.width(), image.height());
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (i, p) in image.pixels().iter().enumerate() {
        let p = p.demultiply();
        if p.alpha() > 0 && (p.red() as u32 + p.green() as u32 + p.blue() as u32) < 3 * threshold {
            let (x, y) = (i as u32 % w, i as u32 / w);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    (x1 >= x0 && y1 >= y0).then(|| (x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

/// The part `rect` of `image` scaled to `w`×`h`, as gray values 0–1 (white 1) on white.
pub fn gray_scaled(
    image: &Pixmap,
    rect: (u32, u32, u32, u32),
    w: u32,
    h: u32,
) -> Result<Vec<f32>, String> {
    let (x0, y0, rw, rh) = rect;
    let mut canvas = Pixmap::new(w, h).ok_or("empty image")?;
    canvas.fill(tiny_skia::Color::WHITE);
    let paint = PixmapPaint {
        quality: FilterQuality::Bicubic,
        ..Default::default()
    };
    let at = Transform::from_translate(-(x0 as f32), -(y0 as f32))
        .post_scale(w as f32 / rw as f32, h as f32 / rh as f32);
    canvas.draw_pixmap(0, 0, image.as_ref(), &paint, at, None);
    Ok(canvas
        .pixels()
        .iter()
        .map(|p| gray(p.demultiply()))
        .collect())
}

fn gray(p: tiny_skia::ColorU8) -> f32 {
    let g = (p.red() as f32 * 0.299 + p.green() as f32 * 0.587 + p.blue() as f32 * 0.114) / 255.0;
    let a = p.alpha() as f32 / 255.0;
    g * a + (1.0 - a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> Pixmap {
        let mut p = Pixmap::new(40, 20).unwrap();
        p.fill(tiny_skia::Color::WHITE);
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(20, 20, 20, 255);
        p.fill_rect(
            tiny_skia::Rect::from_xywh(10.0, 5.0, 12.0, 6.0).unwrap(),
            &paint,
            Transform::identity(),
            None,
        );
        p
    }

    #[test]
    fn ink_is_found() {
        assert_eq!(ink_bounds(&line(), 160), Some((10, 5, 12, 6)));
        let mut blank = Pixmap::new(5, 5).unwrap();
        blank.fill(tiny_skia::Color::WHITE);
        assert_eq!(ink_bounds(&blank, 240), None);
    }

    #[test]
    fn cut_and_scaled_to_gray() {
        let gray = gray_scaled(&line(), (10, 5, 12, 6), 6, 3).unwrap();
        assert_eq!(gray.len(), 18);
        assert!(gray.iter().all(|g| *g < 0.2), "{gray:?}");
    }
}
