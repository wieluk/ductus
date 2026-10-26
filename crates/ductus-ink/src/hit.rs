use crate::radius_at;
use ductus_core::{InkPoint, Stroke};
use kurbo::{Line, ParamCurveNearest, Point, Rect};

pub fn bounds(stroke: &Stroke) -> Rect {
    let half = stroke.style.width / 2.0;
    let mut rect: Option<Rect> = None;
    for p in &stroke.points {
        let pt = Point::new(p.x, p.y);
        rect = Some(match rect {
            Some(r) => r.union_pt(pt),
            None => Rect::from_points(pt, pt),
        });
    }
    rect.map_or(Rect::ZERO, |r| r.inflate(half, half))
}

pub fn hits_circle(stroke: &Stroke, center: Point, r: f64) -> bool {
    if !bounds(stroke).inflate(r, r).contains(center) {
        return false;
    }
    let pts = &stroke.points;
    let pos = |i: usize| Point::new(pts[i].x, pts[i].y);
    match pts.len() {
        0 => false,
        1 => (pos(0) - center).hypot() <= r + radius_at(&stroke.style, &pts[0]),
        n => (1..n).any(|i| {
            let reach =
                r + radius_at(&stroke.style, &pts[i - 1]).max(radius_at(&stroke.style, &pts[i]));
            Line::new(pos(i - 1), pos(i))
                .nearest(center, 1e-6)
                .distance_sq
                <= reach * reach
        }),
    }
}

fn lerp(a: &InkPoint, b: &InkPoint, f: f64) -> InkPoint {
    let mix = |u: f32, v: f32| u + (v - u) * f as f32;
    InkPoint {
        x: a.x + (b.x - a.x) * f,
        y: a.y + (b.y - a.y) * f,
        pressure: mix(a.pressure, b.pressure),
        tilt_x: mix(a.tilt_x, b.tilt_x),
        tilt_y: mix(a.tilt_y, b.tilt_y),
        t: a.t + ((f64::from(b.t.saturating_sub(a.t))) * f) as u32,
    }
}

/// Segments near the eraser are subdivided first, so the cut follows its circle.
pub fn erase_circle(stroke: &Stroke, center: Point, r: f64) -> Option<Vec<Vec<InkPoint>>> {
    if !hits_circle(stroke, center, r) {
        return None;
    }
    let style = &stroke.style;
    let step = (r / 3.0).max(0.25);
    let reach = r + style.width;
    let mut dense: Vec<InkPoint> = Vec::with_capacity(stroke.points.len());
    for (i, p) in stroke.points.iter().enumerate() {
        if let Some(prev) = i.checked_sub(1).map(|j| &stroke.points[j]) {
            let seg = Line::new(Point::new(prev.x, prev.y), Point::new(p.x, p.y));
            let near = seg.nearest(center, 1e-6).distance_sq <= reach * reach;
            let n = (seg.length() / step).ceil() as usize;
            if near && n > 1 {
                dense.extend((1..n).map(|k| lerp(prev, p, k as f64 / n as f64)));
            }
        }
        dense.push(*p);
    }
    let erased = |p: &InkPoint| {
        let limit = r + radius_at(style, p);
        (Point::new(p.x, p.y) - center).hypot2() <= limit * limit
    };
    if !dense.iter().any(erased) {
        return None;
    }
    let pieces = dense
        .split(erased)
        // Lone points would linger as dots next to the cut.
        .filter(|run| run.len() >= 2)
        .map(<[InkPoint]>::to_vec)
        .collect();
    Some(pieces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::{InkPoint, StrokeId, StrokeStyle};

    fn line_stroke() -> Stroke {
        Stroke {
            id: StrokeId(1),
            style: StrokeStyle {
                width: 4.0,
                ..StrokeStyle::default()
            },
            points: [(0.0, 0.0), (100.0, 0.0)]
                .into_iter()
                .map(|(x, y)| InkPoint {
                    x,
                    y,
                    pressure: 1.0,
                    tilt_x: 0.0,
                    tilt_y: 0.0,
                    t: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn erasing_the_middle_splits_in_two() {
        let pieces = erase_circle(&line_stroke(), Point::new(50.0, 0.0), 5.0).unwrap();
        assert_eq!(pieces.len(), 2);
        let end_of_first = pieces[0].last().unwrap().x;
        let start_of_second = pieces[1][0].x;
        assert!(
            end_of_first + 2.0 <= 45.0 && end_of_first > 40.0,
            "{end_of_first}"
        );
        assert!(
            start_of_second - 2.0 >= 55.0 && start_of_second < 60.0,
            "{start_of_second}"
        );
        assert!(erase_circle(&line_stroke(), Point::new(50.0, 20.0), 5.0).is_none());
    }

    #[test]
    fn erasing_an_end_leaves_one_piece() {
        let pieces = erase_circle(&line_stroke(), Point::new(100.0, 0.0), 10.0).unwrap();
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0][0].x, 0.0);
    }
}
