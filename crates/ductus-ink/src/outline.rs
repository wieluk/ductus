use ductus_core::{InkPoint, LinePattern, StrokeStyle, Tool};
use kurbo::{Arc, BezPath, Circle, Point, Shape, Vec2};
use std::f64::consts::PI;

const MIN_WIDTH_RATIO: f64 = 0.25;
const CAP_TOLERANCE: f64 = 0.05;

pub fn radius_at(style: &StrokeStyle, p: &InkPoint) -> f64 {
    let half = style.width / 2.0;
    match style.tool {
        Tool::Pen => half * (MIN_WIDTH_RATIO + (1.0 - MIN_WIDTH_RATIO) * f64::from(p.pressure)),
        Tool::Highlighter => half,
    }
}

/// Fill non-zero: sharp turns may self-overlap.
pub fn outline(points: &[InkPoint], style: &StrokeStyle) -> BezPath {
    match style.pattern {
        LinePattern::Solid => solid_outline(points, style),
        LinePattern::Dashed => {
            // Dash and gap grow with the pen, with a floor for fine pens.
            let dash = (style.width * 4.0).max(6.0);
            let gap = (style.width * 2.5).max(4.0);
            let mut path = BezPath::new();
            for run in runs(points, dash, gap) {
                path.extend(solid_outline(&run, style));
            }
            path
        }
        LinePattern::Dotted => {
            let spacing = (style.width * 2.5).max(4.0);
            let mut path = BezPath::new();
            for p in resample(points, spacing) {
                // Light pressure still gives a visible dot.
                let r = radius_at(style, &p).max(style.width * 0.5) * 1.1;
                path.extend(Circle::new(Point::new(p.x, p.y), r).to_path(CAP_TOLERANCE));
            }
            path
        }
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
        t: a.t,
    }
}

fn resample(points: &[InkPoint], spacing: f64) -> Vec<InkPoint> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let mut out = vec![*first];
    let mut next = spacing;
    let mut walked = 0.0;
    for w in points.windows(2) {
        let len = ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt();
        while len > 0.0 && walked + len >= next {
            out.push(lerp(&w[0], &w[1], (next - walked) / len));
            next += spacing;
        }
        walked += len;
    }
    out
}

fn runs(points: &[InkPoint], dash: f64, gap: f64) -> Vec<Vec<InkPoint>> {
    let mut out = Vec::new();
    let Some(first) = points.first() else {
        return out;
    };
    let mut current = vec![*first];
    let mut drawing = true;
    let mut left = dash;
    for w in points.windows(2) {
        let len = ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt();
        let mut at = 0.0;
        while len - at >= left {
            at += left;
            let p = lerp(&w[0], &w[1], at / len);
            if drawing {
                current.push(p);
                out.push(std::mem::take(&mut current));
            } else {
                current = vec![p];
            }
            drawing = !drawing;
            left = if drawing { dash } else { gap };
        }
        left -= len - at;
        if drawing {
            current.push(w[1]);
        }
    }
    if drawing && current.len() > 1 {
        out.push(current);
    }
    out
}

fn solid_outline(points: &[InkPoint], style: &StrokeStyle) -> BezPath {
    let Some(first) = points.first() else {
        return BezPath::new();
    };
    let pos = |p: &InkPoint| Point::new(p.x, p.y);

    if points.len() == 1 || points.iter().all(|p| (pos(p) - pos(first)).hypot() < 0.01) {
        let r = points
            .iter()
            .map(|p| radius_at(style, p))
            .fold(0.0, f64::max);
        return Circle::new(pos(first), r).to_path(CAP_TOLERANCE);
    }

    let n = points.len();
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    let mut normals = Vec::with_capacity(n);
    let mut last_normal = Vec2::new(0.0, 1.0);
    for i in 0..n {
        let prev = pos(&points[i.saturating_sub(1)]);
        let next = pos(&points[(i + 1).min(n - 1)]);
        let tangent = next - prev;
        let normal = if tangent.hypot() > 1e-9 {
            let t = tangent.normalize();
            Vec2::new(-t.y, t.x)
        } else {
            last_normal
        };
        last_normal = normal;
        let r = radius_at(style, &points[i]);
        let p = pos(&points[i]);
        left.push(p + normal * r);
        right.push(p - normal * r);
        normals.push(normal);
    }

    let mut path = BezPath::new();
    path.move_to(left[0]);
    smooth_through(&mut path, &left);
    cap(
        &mut path,
        pos(&points[n - 1]),
        normals[n - 1],
        radius_at(style, &points[n - 1]),
    );
    right.reverse();
    path.line_to(right[0]);
    smooth_through(&mut path, &right);
    cap(&mut path, pos(first), -normals[0], radius_at(style, first));
    path.close_path();
    path
}

fn smooth_through(path: &mut BezPath, pts: &[Point]) {
    if pts.len() < 3 {
        if let Some(last) = pts.last() {
            path.line_to(*last);
        }
        return;
    }
    path.line_to(pts[0].midpoint(pts[1]));
    for i in 1..pts.len() - 1 {
        path.quad_to(pts[i], pts[i].midpoint(pts[i + 1]));
    }
    path.line_to(pts[pts.len() - 1]);
}

fn cap(path: &mut BezPath, center: Point, normal: Vec2, r: f64) {
    let arc = Arc {
        center,
        radii: Vec2::new(r, r),
        start_angle: normal.atan2(),
        sweep_angle: -PI,
        x_rotation: 0.0,
    };
    path.extend(arc.append_iter(CAP_TOLERANCE));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64, pressure: f32) -> InkPoint {
        InkPoint {
            x,
            y,
            pressure,
            tilt_x: 0.0,
            tilt_y: 0.0,
            t: 0,
        }
    }

    fn style(width: f64) -> StrokeStyle {
        StrokeStyle {
            width,
            ..StrokeStyle::default()
        }
    }

    #[test]
    fn single_point_is_a_dot() {
        let area = outline(&[pt(5.0, 5.0, 1.0)], &style(10.0)).area().abs();
        assert!((area - PI * 25.0).abs() < 0.5, "area {area}");
    }

    #[test]
    fn outline_stays_within_radius_of_curve() {
        let pts: Vec<_> = (0..=90)
            .map(|deg| {
                let a = f64::from(deg).to_radians();
                pt(50.0 * a.cos(), 50.0 * a.sin(), 1.0)
            })
            .collect();
        let bb = outline(&pts, &style(4.0)).bounding_box();
        assert!(
            bb.x0 >= -2.1 && bb.y0 >= -2.1 && bb.x1 <= 52.1 && bb.y1 <= 52.1,
            "{bb:?}"
        );
    }

    #[test]
    fn dotted_and_dashed_strokes_break_into_pieces() {
        let points: Vec<InkPoint> = (0..=100)
            .map(|i| InkPoint {
                x: f64::from(i),
                y: 0.0,
                pressure: 1.0,
                tilt_x: 0.0,
                tilt_y: 0.0,
                t: 0,
            })
            .collect();
        let subpaths = |pattern| {
            let style = StrokeStyle {
                pattern,
                ..style(2.0)
            };
            outline(&points, &style)
                .elements()
                .iter()
                .filter(|e| matches!(e, kurbo::PathEl::MoveTo(_)))
                .count()
        };
        assert_eq!(subpaths(LinePattern::Solid), 1);
        assert_eq!(subpaths(LinePattern::Dotted), 21);
        let dashes = subpaths(LinePattern::Dashed);
        assert!((7..=9).contains(&dashes), "{dashes} dashes");
    }
}
