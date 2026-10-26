use ductus_core::InkPoint;
use kurbo::{Point, Vec2};
use std::f64::consts::{PI, TAU};

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Line {
        from: Point,
        to: Point,
    },
    Arrow {
        from: Point,
        to: Point,
    },
    Ellipse {
        center: Point,
        rx: f64,
        ry: f64,
        angle: f64,
    },
    Polygon {
        corners: Vec<Point>,
    },
}

/// Angles this close to a multiple of 45° snap to it.
const SNAP_ANGLE: f64 = 6.0 * PI / 180.0;

fn snap_direction(from: Point, to: Point) -> Point {
    let d = to - from;
    let angle = d.atan2();
    let step = PI / 4.0;
    let snapped = (angle / step).round() * step;
    if (angle - snapped).abs() < SNAP_ANGLE {
        from + Vec2::from_angle(snapped) * d.hypot()
    } else {
        to
    }
}

fn positions(points: &[InkPoint]) -> Vec<Point> {
    points.iter().map(|p| Point::new(p.x, p.y)).collect()
}

fn path_length(pts: &[Point]) -> f64 {
    pts.windows(2).map(|w| (w[1] - w[0]).hypot()).sum()
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let len2 = ab.hypot2();
    if len2 == 0.0 {
        return (p - a).hypot();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).hypot()
}

/// Ramer–Douglas–Peucker.
fn simplify(pts: &[Point], epsilon: f64) -> Vec<Point> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let (index, dist) = pts[1..pts.len() - 1]
        .iter()
        .enumerate()
        .map(|(i, p)| (i + 1, distance_to_segment(*p, a, b)))
        .fold(
            (0, 0.0),
            |best, cur| if cur.1 > best.1 { cur } else { best },
        );
    if dist <= epsilon {
        return vec![a, b];
    }
    let mut left = simplify(&pts[..=index], epsilon);
    let right = simplify(&pts[index..], epsilon);
    left.pop();
    left.extend(right);
    left
}

fn corner_angle(a: Point, b: Point, c: Point) -> f64 {
    let (u, v) = (a - b, c - b);
    (u.dot(v) / (u.hypot() * v.hypot()).max(1e-9))
        .clamp(-1.0, 1.0)
        .acos()
}

pub fn recognize(points: &[InkPoint]) -> Option<Shape> {
    let pts = positions(points);
    let length = path_length(&pts);
    if pts.len() < 3 || length < 12.0 {
        return None;
    }
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    let closed = (last - first).hypot() < 0.18 * length;
    if closed {
        recognize_closed(&pts, length)
    } else {
        recognize_open(&pts, length)
    }
}

fn recognize_open(pts: &[Point], length: f64) -> Option<Shape> {
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    let chord = (last - first).hypot();
    // Straight: nearly no detour and no point far from the chord.
    let deviation = pts
        .iter()
        .map(|p| distance_to_segment(*p, first, last))
        .fold(0.0, f64::max);
    if length < chord * 1.08 && deviation < 0.06 * chord.max(1.0) {
        return Some(Shape::Line {
            from: first,
            to: snap_direction(first, last),
        });
    }
    recognize_arrow(pts, length)
}

/// Drawn in one stroke: barb, back, barb (A B C B D) or a V across the tip (A B C D).
fn recognize_arrow(pts: &[Point], length: f64) -> Option<Shape> {
    let v = simplify(pts, 0.04 * length);
    let shaft_of = |a: Point, b: Point| (b - a).hypot();
    let side = |origin: Point, dir: Vec2, p: Point| dir.cross(p - origin).signum();
    match v.as_slice() {
        [a, b, c, b2, d] => {
            let shaft = shaft_of(*a, *b);
            let short = |p: Point, q: Point| (q - p).hypot() < 0.6 * shaft;
            let back_at_tip = (*b2 - *b).hypot() < 0.3 * shaft;
            let dir = *b - *a;
            let opposite = side(*b, dir, *c) != side(*b, dir, *d);
            (short(*b, *c) && short(*b2, *d) && back_at_tip && opposite).then(|| Shape::Arrow {
                from: *a,
                to: snap_direction(*a, *b),
            })
        }
        [a, b, c, d] => {
            let shaft = shaft_of(*a, *b);
            let dir = *b - *a;
            let barbs_near = (*c - *b).hypot() < 0.6 * shaft && (*d - *b).hypot() < 0.6 * shaft;
            let opposite = side(*b, dir, *c) != side(*b, dir, *d);
            let pointing_back = (*c - *b).dot(dir) < 0.0 && (*d - *b).dot(dir) < 0.0;
            (barbs_near && opposite && pointing_back).then(|| Shape::Arrow {
                from: *a,
                to: snap_direction(*a, *b),
            })
        }
        _ => None,
    }
}

fn recognize_closed(pts: &[Point], length: f64) -> Option<Shape> {
    let polygon = fit_polygon(pts, length);
    let ellipse = fit_ellipse(pts);
    match (polygon, ellipse) {
        (Some((poly, poly_err)), Some((ell, ell_err))) => {
            Some(if poly_err <= ell_err { poly } else { ell })
        }
        (Some((poly, _)), None) => Some(poly),
        (None, Some((ell, _))) => Some(ell),
        (None, None) => None,
    }
}

fn polygon_error(pts: &[Point], corners: &[Point], perimeter: f64) -> f64 {
    let n = corners.len();
    let mean = pts
        .iter()
        .map(|p| {
            (0..n)
                .map(|i| distance_to_segment(*p, corners[i], corners[(i + 1) % n]))
                .fold(f64::INFINITY, f64::min)
        })
        .sum::<f64>()
        / pts.len() as f64;
    mean / perimeter
}

fn fit_polygon(pts: &[Point], length: f64) -> Option<(Shape, f64)> {
    let mut corners = simplify(pts, 0.045 * length);
    // The closing point duplicates the start.
    if corners.len() > 2 && (corners[corners.len() - 1] - corners[0]).hypot() < 0.15 * length {
        corners.pop();
    }
    // Drop nearly straight vertices left by the simplification.
    let mut changed = true;
    while changed && corners.len() > 3 {
        changed = false;
        let n = corners.len();
        for i in 0..n {
            let angle = corner_angle(corners[(i + n - 1) % n], corners[i], corners[(i + 1) % n]);
            if angle > 160f64.to_radians() {
                corners.remove(i);
                changed = true;
                break;
            }
        }
    }
    if !(3..=6).contains(&corners.len()) {
        return None;
    }
    if corners.len() == 4 {
        corners = square_up(&corners);
    }
    let error = polygon_error(pts, &corners, length);
    (error < 0.025).then_some((Shape::Polygon { corners }, error))
}

fn square_up(c: &[Point]) -> Vec<Point> {
    let right = (0..4).all(|i| {
        let a = corner_angle(c[(i + 3) % 4], c[i], c[(i + 1) % 4]);
        (a - PI / 2.0).abs() < 15f64.to_radians()
    });
    if !right {
        return c.to_vec();
    }
    let e1 = c[1] - c[0];
    let e2 = c[2] - c[3];
    let mut angle = (e1 + e2).atan2();
    let step = PI / 2.0;
    let snapped = (angle / step).round() * step;
    if (angle - snapped).abs() < 8f64.to_radians() {
        angle = snapped;
    }
    let u = Vec2::from_angle(angle);
    let v = Vec2::new(-u.y, u.x);
    let center = c.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2()) / 4.0;
    let half_w = c
        .iter()
        .map(|p| (p.to_vec2() - center).dot(u).abs())
        .sum::<f64>()
        / 4.0;
    let half_h = c
        .iter()
        .map(|p| (p.to_vec2() - center).dot(v).abs())
        .sum::<f64>()
        / 4.0;
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .map(|(su, sv)| (center + u * (su * half_w) + v * (sv * half_h)).to_point())
        .collect()
}

fn fit_ellipse(pts: &[Point]) -> Option<(Shape, f64)> {
    let n = pts.len() as f64;
    let center = pts.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2()) / n;
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for p in pts {
        let d = p.to_vec2() - center;
        sxx += d.x * d.x;
        syy += d.y * d.y;
        sxy += d.x * d.y;
    }
    let mut angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let u = Vec2::from_angle(angle);
    let v = Vec2::new(-u.y, u.x);
    // For points spread evenly around an ellipse, std = radius / √2.
    let ru = (pts
        .iter()
        .map(|p| (p.to_vec2() - center).dot(u).powi(2))
        .sum::<f64>()
        / n
        * 2.0)
        .sqrt();
    let rv = (pts
        .iter()
        .map(|p| (p.to_vec2() - center).dot(v).powi(2))
        .sum::<f64>()
        / n
        * 2.0)
        .sqrt();
    if ru < 4.0 || rv < 4.0 {
        return None;
    }
    let error = pts
        .iter()
        .map(|p| {
            let d = p.to_vec2() - center;
            ((d.dot(u) / ru).powi(2) + (d.dot(v) / rv).powi(2)).sqrt() - 1.0
        })
        .map(f64::abs)
        .sum::<f64>()
        / n;
    if error > 0.12 {
        return None;
    }
    let (mut rx, mut ry) = (ru, rv);
    if (rx - ry).abs() < 0.12 * rx.max(ry) {
        let r = (rx + ry) / 2.0;
        (rx, ry, angle) = (r, r, 0.0);
    } else {
        let step = PI / 2.0;
        let snapped = (angle / step).round() * step;
        if (angle - snapped).abs() < 10f64.to_radians() {
            angle = snapped;
        }
    }
    // In the polygon error's units, so the two fits compare.
    let perimeter = PI * (3.0 * (rx + ry) - ((3.0 * rx + ry) * (rx + 3.0 * ry)).sqrt());
    let comparable = error * (rx + ry) / 2.0 / perimeter;
    Some((
        Shape::Ellipse {
            center: center.to_point(),
            rx,
            ry,
            angle,
        },
        comparable,
    ))
}

impl Shape {
    pub fn outline_points(&self, step: f64) -> Vec<Point> {
        let segment = |a: Point, b: Point, out: &mut Vec<Point>| {
            let n = ((b - a).hypot() / step).ceil().max(1.0) as usize;
            out.extend((0..n).map(|i| a.lerp(b, i as f64 / n as f64)));
        };
        let mut out = Vec::new();
        match self {
            Shape::Line { from, to } => {
                segment(*from, *to, &mut out);
                out.push(*to);
            }
            Shape::Arrow { from, to } => {
                let (left, right) = self.arrow_barbs().expect("arrows have barbs");
                segment(*from, *to, &mut out);
                segment(*to, left, &mut out);
                segment(left, *to, &mut out);
                segment(*to, right, &mut out);
                out.push(right);
            }
            Shape::Ellipse {
                center,
                rx,
                ry,
                angle,
            } => {
                let circumference =
                    PI * (3.0 * (rx + ry) - ((3.0 * rx + ry) * (rx + 3.0 * ry)).sqrt());
                let n = (circumference / step).ceil().max(12.0) as usize;
                let (u, v) = (
                    Vec2::from_angle(*angle),
                    Vec2::from_angle(*angle + PI / 2.0),
                );
                for i in 0..=n {
                    let t = TAU * i as f64 / n as f64;
                    out.push(*center + u * (rx * t.cos()) + v * (ry * t.sin()));
                }
            }
            Shape::Polygon { corners } => {
                for i in 0..corners.len() {
                    segment(corners[i], corners[(i + 1) % corners.len()], &mut out);
                }
                out.push(corners[0]);
            }
        }
        out
    }

    fn arrow_barbs(&self) -> Option<(Point, Point)> {
        let Shape::Arrow { from, to } = self else {
            return None;
        };
        let dir = *to - *from;
        let len = dir.hypot().max(1.0);
        let head = (len * 0.25).clamp(8.0, 40.0);
        let back = -dir / len * head;
        let rotate = |v: Vec2, a: f64| {
            Vec2::new(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos())
        };
        let spread = 28f64.to_radians();
        Some((*to + rotate(back, spread), *to + rotate(back, -spread)))
    }

    /// Lines follow the pen with their end; closed shapes scale with its distance from the center.
    pub fn adjusted(&self, pen_at_snap: Point, pen: Point) -> Shape {
        match self {
            Shape::Line { from, .. } => Shape::Line {
                from: *from,
                to: snap_direction(*from, pen),
            },
            Shape::Arrow { from, .. } => Shape::Arrow {
                from: *from,
                to: snap_direction(*from, pen),
            },
            Shape::Ellipse {
                center,
                rx,
                ry,
                angle,
            } => {
                let k = scale_factor(*center, pen_at_snap, pen);
                Shape::Ellipse {
                    center: *center,
                    rx: rx * k,
                    ry: ry * k,
                    angle: *angle,
                }
            }
            Shape::Polygon { corners } => {
                let center =
                    corners.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / corners.len() as f64;
                let k = scale_factor(center.to_point(), pen_at_snap, pen);
                Shape::Polygon {
                    corners: corners
                        .iter()
                        .map(|p| (center + (p.to_vec2() - center) * k).to_point())
                        .collect(),
                }
            }
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Shape::Line { .. } => "line",
            Shape::Arrow { .. } => "arrow",
            Shape::Ellipse { rx, ry, .. } if (rx - ry).abs() < 1e-9 => "circle",
            Shape::Ellipse { .. } => "ellipse",
            Shape::Polygon { corners } if corners.len() == 3 => "triangle",
            Shape::Polygon { corners } if corners.len() == 4 => "rectangle",
            Shape::Polygon { .. } => "polygon",
        }
    }
}

fn scale_factor(center: Point, at_snap: Point, now: Point) -> f64 {
    let before = (at_snap - center).hypot().max(1.0);
    ((now - center).hypot() / before).clamp(0.2, 5.0)
}

#[cfg(test)]
mod tests;
