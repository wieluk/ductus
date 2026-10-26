use kurbo::{Point, Rect, Vec2};

pub const MIN_ZOOM: f64 = 0.1;
pub const MAX_ZOOM: f64 = 10.0;

/// screen = doc * zoom + offset
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub zoom: f64,
    pub offset: Vec2,
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            offset: Vec2::new(40.0, 40.0),
            width: 0.0,
            height: 0.0,
            dpr: 1.0,
        }
    }
}

impl View {
    pub fn to_doc(&self, screen: Point) -> Point {
        ((screen.to_vec2() - self.offset) / self.zoom).to_point()
    }

    pub fn to_screen(&self, doc: Point) -> Point {
        (doc.to_vec2() * self.zoom + self.offset).to_point()
    }

    pub fn to_screen_rect(&self, doc: Rect) -> Rect {
        Rect::from_points(
            self.to_screen(doc.origin()),
            self.to_screen(Point::new(doc.x1, doc.y1)),
        )
    }

    pub fn visible_doc_rect(&self) -> Rect {
        Rect::from_points(
            self.to_doc(Point::ZERO),
            self.to_doc(Point::new(self.width, self.height)),
        )
    }

    pub fn zoom_around(&mut self, anchor: Point, zoom: f64) {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let doc = self.to_doc(anchor);
        self.zoom = zoom;
        self.offset = anchor.to_vec2() - doc.to_vec2() * zoom;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_anchor_fixed() {
        let mut v = View::default();
        let anchor = Point::new(300.0, 200.0);
        let before = v.to_doc(anchor);
        v.zoom_around(anchor, 2.5);
        assert!((v.to_doc(anchor) - before).hypot() < 1e-9);
        assert_eq!(v.zoom, 2.5);
    }

    #[test]
    fn round_trip() {
        let v = View {
            zoom: 1.7,
            offset: Vec2::new(-30.0, 12.5),
            ..View::default()
        };
        let p = Point::new(123.0, -45.0);
        assert!((v.to_doc(v.to_screen(p)) - p).hypot() < 1e-9);
    }
}
