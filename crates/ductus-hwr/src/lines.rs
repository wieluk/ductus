//! Lines come from the ink's geometry, not image analysis.

use ductus_core::{Rgba, Stroke, Tool};
use ductus_render::{StrokePath, Viewport};
use kurbo::{Point, Rect};
use tiny_skia::Pixmap;

#[derive(Debug, Clone, PartialEq)]
pub struct InkLine {
    pub strokes: Vec<usize>,
    pub rect: Rect,
    pub key: String,
}

fn vertical_overlap(a: Rect, b: Rect) -> f64 {
    a.y1.min(b.y1) - a.y0.max(b.y0)
}

fn horizontal_gap(a: Rect, b: Rect) -> f64 {
    (b.x0 - a.x1).max(a.x0 - b.x1).max(0.0)
}

/// Small marks (i-dots, accents, punctuation) join the line they sit on.
pub fn find_lines(strokes: &[Stroke]) -> Vec<InkLine> {
    let pens: Vec<(usize, Rect)> = strokes
        .iter()
        .enumerate()
        .filter(|(_, s)| s.style.tool == Tool::Pen && !s.points.is_empty())
        .map(|(i, s)| (i, ductus_ink::bounds(s)))
        .collect();
    if pens.is_empty() {
        return Vec::new();
    }
    let mut heights: Vec<f64> = pens.iter().map(|(_, b)| b.height()).collect();
    heights.sort_by(f64::total_cmp);
    let letter = heights[heights.len() / 2].max(8.0);

    let is_small = |b: &Rect| b.height() < letter * 0.5 && b.width() < letter;
    // Letters first, top to bottom; then the small marks, which join the
    // lines the letters made.
    let mut order = pens.clone();
    order.sort_by(|a, b| {
        is_small(&a.1)
            .cmp(&is_small(&b.1))
            .then(a.1.center().y.total_cmp(&b.1.center().y))
    });
    let mut lines: Vec<(Vec<usize>, Rect)> = Vec::new();
    for (index, b) in order {
        let small = is_small(&b);
        let best = lines
            .iter()
            .enumerate()
            .filter(|(_, (_, r))| horizontal_gap(*r, b) < letter * 4.0)
            .filter_map(|(i, (_, r))| {
                let overlap = vertical_overlap(*r, b);
                let enough = if small {
                    b.center().y > r.y0 - letter * 0.8 && b.center().y < r.y1 + letter * 0.3
                } else {
                    overlap >= 0.5 * b.height().min(r.height().max(letter * 0.6))
                };
                enough.then_some((i, overlap))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        match best {
            Some(i) => {
                lines[i].0.push(index);
                lines[i].1 = lines[i].1.union(b);
            }
            None => lines.push((vec![index], b)),
        }
    }
    // Lines that ended up on top of each other are one line.
    let mut merged = true;
    while merged {
        merged = false;
        'outer: for i in 0..lines.len() {
            for j in i + 1..lines.len() {
                let (a, b) = (lines[i].1, lines[j].1);
                if vertical_overlap(a, b) > 0.6 * a.height().min(b.height())
                    && horizontal_gap(a, b) < letter * 4.0
                {
                    let (strokes, rect) = lines.remove(j);
                    lines[i].0.extend(strokes);
                    lines[i].1 = lines[i].1.union(rect);
                    merged = true;
                    break 'outer;
                }
            }
        }
    }
    lines.sort_by(|a, b| {
        (a.1.y0 / letter)
            .floor()
            .total_cmp(&(b.1.y0 / letter).floor())
            .then(a.1.x0.total_cmp(&b.1.x0))
    });
    lines
        .into_iter()
        .map(|(mut indices, rect)| {
            indices.sort_by(|a, b| {
                ductus_ink::bounds(&strokes[*a])
                    .x0
                    .total_cmp(&ductus_ink::bounds(&strokes[*b]).x0)
            });
            InkLine {
                key: line_key(strokes, &indices),
                strokes: indices,
                rect,
            }
        })
        .collect()
}

/// Over rounded points, so the key survives saving but changes with the ink.
fn line_key(strokes: &[Stroke], indices: &[usize]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: i64| {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for &i in indices {
        for p in &strokes[i].points {
            eat((p.x * 2.0).round() as i64);
            eat((p.y * 2.0).round() as i64);
        }
        eat(-1);
    }
    format!("{h:016x}")
}

pub fn line_png(strokes: &[Stroke], line: &InkLine, height: u32) -> Option<Vec<u8>> {
    let scale = (f64::from(height) / line.rect.height().max(1.0)).clamp(0.25, 4.0);
    render(
        strokes,
        &line.strokes,
        line.rect,
        scale,
        f64::from(height) * 0.25,
    )
}

/// The strokes as one image whose longer side is about `side` pixels (formulas).
pub fn ink_png(strokes: &[Stroke], indices: &[usize], side: u32) -> Option<Vec<u8>> {
    let rect = indices
        .iter()
        .map(|&i| ductus_ink::bounds(&strokes[i]))
        .reduce(|a, b| a.union(b))?;
    let scale = (f64::from(side) / rect.width().max(rect.height()).max(1.0)).min(4.0);
    render(strokes, indices, rect, scale, f64::from(side) * 0.05)
}

fn render(
    strokes: &[Stroke],
    indices: &[usize],
    rect: Rect,
    scale: f64,
    pad: f64,
) -> Option<Vec<u8>> {
    let width = (rect.width() * scale + 2.0 * pad).ceil() as u32;
    let height = (rect.height() * scale + 2.0 * pad).ceil() as u32;
    let mut pixmap = Pixmap::new(width.clamp(1, 8192), height.clamp(1, 4096))?;
    pixmap.fill(tiny_skia::Color::WHITE);
    let vp = Viewport {
        origin: Point::new(rect.x0 - pad / scale, rect.y0 - pad / scale),
        scale,
    };
    let black = Rgba::rgb(0.0, 0.0, 0.0);
    let paths: Vec<StrokePath> = indices
        .iter()
        .filter_map(|&i| {
            StrokePath::fill(
                &ductus_ink::outline(&strokes[i].points, &strokes[i].style),
                black,
            )
        })
        .collect();
    ductus_render::render_strokes(&mut pixmap, &vp, paths.iter(), None);
    pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::{InkPoint, StrokeId, StrokeStyle};

    fn letter(x: f64, y: f64, h: f64) -> Stroke {
        Stroke {
            id: StrokeId(0),
            style: StrokeStyle::default(),
            points: (0..6)
                .map(|i| InkPoint {
                    x: x + f64::from(i) * h * 0.15,
                    y: y + if i % 2 == 0 { 0.0 } else { h },
                    pressure: 0.6,
                    tilt_x: 0.0,
                    tilt_y: 0.0,
                    t: 0,
                })
                .collect(),
        }
    }

    fn dot(x: f64, y: f64) -> Stroke {
        Stroke {
            points: vec![letter(x, y, 1.0).points[0]],
            ..letter(x, y, 1.0)
        }
    }

    #[test]
    fn letters_group_into_lines_with_their_dots() {
        let mut strokes = Vec::new();
        for i in 0..5 {
            strokes.push(letter(50.0 + f64::from(i) * 22.0, 100.0, 20.0));
        }
        for i in 0..4 {
            strokes.push(letter(50.0 + f64::from(i) * 22.0, 160.0, 20.0));
        }
        strokes.push(dot(75.0, 92.0));
        strokes.push(letter(200.0, 162.0, 18.0));
        let lines = find_lines(&strokes);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(lines[0].strokes.len(), 6);
        assert!(lines[0].strokes.contains(&9));
        assert_eq!(lines[1].strokes.len(), 5);
        let again = find_lines(&strokes);
        assert_eq!(lines[0].key, again[0].key);
        strokes[0].points[0].x += 3.0;
        assert_ne!(find_lines(&strokes)[0].key, lines[0].key);
    }

    #[test]
    fn far_apart_words_are_separate_lines() {
        let strokes = vec![letter(50.0, 100.0, 20.0), letter(600.0, 100.0, 20.0)];
        assert_eq!(find_lines(&strokes).len(), 2);
    }
}
