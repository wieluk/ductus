use crate::Canvas;
use kurbo::{Rect, Vec2};
use tiny_skia::Pixmap;

const MARK_COLOR: ductus_core::Rgba = ductus_core::Rgba {
    r: 1.0,
    g: 0.82,
    b: 0.1,
    a: 0.35,
};

impl Canvas {
    pub fn focus_rect(&mut self, rect: Rect) {
        let z = self.view.zoom;
        let center = rect.center();
        self.view.offset = Vec2::new(
            (self.view.width / 2.0 - center.x * z).round(),
            (self.view.height / 2.0 - center.y * z).round(),
        );
        self.mark = Some(rect.inflate(6.0, 4.0));
        self.redraw_overlay();
    }

    pub fn focus_sheet(&mut self, index: usize) {
        let Some(sheet) = self.layout.sheet_rects().get(index).copied() else {
            return;
        };
        let z = self.view.zoom;
        self.view.offset = Vec2::new(
            (self.view.width / 2.0 - sheet.center().x * z).round(),
            (24.0 - sheet.y0 * z).round(),
        );
    }

    pub fn clear_mark(&mut self) {
        if self.mark.take().is_some() {
            self.redraw_overlay();
        }
    }

    pub(crate) fn mark_bounds(&self) -> Option<Rect> {
        self.mark
    }

    pub(crate) fn render_mark(&self, pixmap: &mut Pixmap, vp: &ductus_render::Viewport) {
        if let Some(rect) = self.mark {
            let mut path = kurbo::BezPath::new();
            path.extend(kurbo::Shape::path_elements(
                &kurbo::RoundedRect::from_rect(rect, 4.0),
                0.1,
            ));
            if let Some(fill) = ductus_render::StrokePath::fill(&path, MARK_COLOR) {
                ductus_render::render_strokes(pixmap, vp, std::iter::once(&fill), None);
            }
        }
    }
}
