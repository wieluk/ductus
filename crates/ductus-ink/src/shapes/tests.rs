use super::*;

fn wobble(i: usize, amp: f64) -> f64 {
    (((i * 7919 + 13) % 17) as f64 / 8.0 - 1.0) * amp
}

fn ink(points: impl IntoIterator<Item = (f64, f64)>) -> Vec<InkPoint> {
    points
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| InkPoint {
            x,
            y,
            pressure: 0.6,
            tilt_x: 0.0,
            tilt_y: 0.0,
            t: i as u32 * 8,
        })
        .collect()
}

fn drawn(corners: &[(f64, f64)], amp: f64) -> Vec<InkPoint> {
    let mut pts = Vec::new();
    for w in corners.windows(2) {
        let (a, b) = (Point::new(w[0].0, w[0].1), Point::new(w[1].0, w[1].1));
        let n = ((b - a).hypot() / 3.0).ceil() as usize;
        for k in 0..n {
            let p = a.lerp(b, k as f64 / n as f64);
            let i = pts.len();
            pts.push((p.x + wobble(i, amp), p.y + wobble(i + 5, amp)));
        }
    }
    let last = corners[corners.len() - 1];
    pts.push(last);
    ink(pts)
}

fn round(cx: f64, cy: f64, rx: f64, ry: f64, turns: f64, amp: f64) -> Vec<InkPoint> {
    let n = 120;
    ink((0..=n).map(|i| {
        let t = TAU * turns * i as f64 / n as f64;
        (
            cx + rx * t.cos() + wobble(i, amp),
            cy + ry * t.sin() + wobble(i + 3, amp),
        )
    }))
}

#[test]
fn wobbly_lines_become_straight() {
    let shape = recognize(&drawn(&[(10.0, 10.0), (310.0, 120.0)], 1.5)).unwrap();
    let Shape::Line { from, to } = shape else {
        panic!("{shape:?}")
    };
    assert!((from - Point::new(10.0, 10.0)).hypot() < 3.0);
    assert!((to - Point::new(310.0, 120.0)).hypot() < 3.0);
}

#[test]
fn arcs_and_handwriting_stay_ink() {
    assert_eq!(recognize(&round(100.0, 100.0, 80.0, 80.0, 0.25, 0.5)), None);
    let squiggle = ink((0..80).map(|i| {
        let t = i as f64 / 10.0;
        (t * 12.0 + 8.0 * t.cos(), 50.0 + 10.0 * t.sin())
    }));
    assert_eq!(recognize(&squiggle), None);
}

#[test]
fn round_loops_become_circles() {
    let shape = recognize(&round(200.0, 150.0, 70.0, 66.0, 1.0, 2.0)).unwrap();
    let Shape::Ellipse { center, rx, ry, .. } = shape else {
        panic!("{shape:?}")
    };
    assert_eq!(rx, ry, "nearly round snaps to a circle");
    assert!((center - Point::new(200.0, 150.0)).hypot() < 3.0);
    assert!((rx - 68.0).abs() < 5.0, "radius {rx}");
}

#[test]
fn boxes_become_axis_aligned_rectangles() {
    let rough = drawn(
        &[
            (50.0, 50.0),
            (252.0, 54.0),
            (249.0, 151.0),
            (48.0, 148.0),
            (52.0, 52.0),
        ],
        1.5,
    );
    let shape = recognize(&rough).unwrap();
    assert_eq!(shape.kind(), "rectangle");
    let Shape::Polygon { corners } = shape else {
        unreachable!()
    };
    assert!((corners[0].y - corners[1].y).abs() < 1e-6);
    assert!((corners[1].x - corners[2].x).abs() < 1e-6);
}
