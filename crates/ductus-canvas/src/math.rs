use crate::{Canvas, TileLayer};
use ductus_core::{Command, MathBox, Rgba, StrokeId};
use ductus_render::{StrokePath, Viewport};
use kurbo::{Point, Rect};
use tiny_skia::Pixmap;

const NEW_FORMULA_SCALE: f64 = 1.5;

#[derive(Debug, Clone)]
pub(crate) struct MathDraft {
    target: Option<StrokeId>,
    math: MathBox,
}

pub(crate) fn math_rect(math: &MathBox) -> Rect {
    Rect::new(math.x, math.y, math.x + math.width, math.y + math.height)
}

/// A formula with mistakes keeps its previous size.
pub(crate) fn measured(mut math: MathBox) -> MathBox {
    if let Some((w, h)) = ductus_math::size(&math) {
        math.width = w;
        math.height = h;
    }
    math
}

pub(crate) fn math_paths(math: &MathBox) -> Vec<StrokePath> {
    ductus_math::outlines(math)
        .iter()
        .filter_map(|(path, color)| StrokePath::fill(path, *color))
        .collect()
}

impl Canvas {
    pub fn math_at(&self, at: Point) -> Option<StrokeId> {
        let slack = 6.0 / self.view.zoom;
        self.doc
            .maths()
            .iter()
            .filter(|m| math_rect(m).inflate(slack, slack).contains(at))
            .map(|m| m.id)
            .max_by_key(|id| self.doc.stack_key(*id))
    }

    pub fn begin_math(&mut self, at: Point, syntax: &str, color: Rgba) {
        self.cancel_math();
        self.clear_selection();
        self.math_draft = Some(MathDraft {
            target: None,
            math: MathBox {
                id: StrokeId(0),
                source: String::new(),
                syntax: syntax.to_owned(),
                x: at.x,
                y: at.y,
                width: 0.0,
                height: 0.0,
                // A bit larger than body text, to sit well with handwriting.
                scale: NEW_FORMULA_SCALE,
                color,
            },
        });
    }

    pub fn edit_math(&mut self, id: StrokeId) -> Option<(String, String)> {
        self.cancel_math();
        let math = self.doc.maths()[self.doc.math_index_of(id)?].clone();
        self.clear_selection();
        self.tiles.invalidate(TileLayer::Base, math_rect(&math));
        let found = (math.source.clone(), math.syntax.clone());
        self.math_draft = Some(MathDraft {
            target: Some(id),
            math,
        });
        self.redraw_overlay();
        Some(found)
    }

    pub fn set_math_draft(&mut self, source: &str, syntax: &str) -> String {
        let Some(draft) = &mut self.math_draft else {
            return String::new();
        };
        draft.math.source = source.to_owned();
        draft.math.syntax = syntax.to_owned();
        let problem = if source.trim().is_empty() {
            String::new()
        } else {
            match ductus_math::layout(&draft.math).as_ref() {
                Ok(_) => String::new(),
                Err(e) => e.clone(),
            }
        };
        draft.math = measured(draft.math.clone());
        self.redraw_overlay();
        problem
    }

    pub fn commit_math(&mut self) -> bool {
        let Some(draft) = self.math_draft.take() else {
            return false;
        };
        self.redraw_overlay();
        let math = measured(draft.math);
        let empty = math.source.trim().is_empty();
        let cmd = match draft.target {
            Some(id) => {
                let before =
                    self.doc.maths()[self.doc.math_index_of(id).expect("target exists")].clone();
                self.tiles.invalidate(TileLayer::Base, math_rect(&before));
                if empty {
                    Command::remove_maths(&self.doc, &[id])
                } else if before == math {
                    None
                } else {
                    Some(Command::ReplaceMaths {
                        before: vec![before],
                        after: vec![math.clone()],
                    })
                }
            }
            None if empty => None,
            None => Some(Command::AddMaths(vec![MathBox {
                id: self.doc.alloc_id(),
                ..math.clone()
            }])),
        };
        self.tiles.invalidate(TileLayer::Base, math_rect(&math));
        let Some(cmd) = cmd else {
            return false;
        };
        self.history.apply(&mut self.doc, cmd);
        self.sync_layout();
        true
    }

    pub fn cancel_math(&mut self) {
        if let Some(draft) = self.math_draft.take() {
            if let Some(id) = draft.target
                && let Some(i) = self.doc.math_index_of(id)
            {
                let rect = math_rect(&self.doc.maths()[i]);
                self.tiles.invalidate(TileLayer::Base, rect);
            }
            self.redraw_overlay();
        }
    }

    pub fn math_draft_screen_rect(&self) -> Option<Rect> {
        let math = &self.math_draft.as_ref()?.math;
        let r = math_rect(math);
        let r = Rect::new(r.x0, r.y0, r.x1.max(r.x0 + 24.0), r.y1.max(r.y0 + 24.0));
        Some(self.view.to_screen_rect(r))
    }

    pub(crate) fn math_draft_target(&self) -> Option<StrokeId> {
        self.math_draft.as_ref().and_then(|d| d.target)
    }

    pub(crate) fn math_draft_bounds(&self) -> Option<Rect> {
        let math = &self.math_draft.as_ref()?.math;
        Some(math_rect(math).inflate(8.0, 8.0))
    }

    pub(crate) fn render_math_draft(&self, pixmap: &mut Pixmap, vp: &Viewport) {
        if let Some(draft) = &self.math_draft {
            let paths = math_paths(&draft.math);
            ductus_render::render_strokes(pixmap, vp, paths.iter(), None);
            let r = math_rect(&draft.math).inflate(4.0, 4.0);
            let r = Rect::new(r.x0, r.y0, r.x1.max(r.x0 + 24.0), r.y1.max(r.y0 + 24.0));
            let corners = [
                Point::new(r.x0, r.y0),
                Point::new(r.x1, r.y0),
                Point::new(r.x1, r.y1),
                Point::new(r.x0, r.y1),
            ];
            ductus_render::render_guide(pixmap, vp, &corners, true, crate::SELECTION_COLOR);
        }
    }

    pub(crate) fn render_math(
        &self,
        id: StrokeId,
        pixmap: &mut Pixmap,
        vp: &Viewport,
        clip: Option<&tiny_skia::Mask>,
    ) {
        if let Some(i) = self.doc.math_index_of(id) {
            let paths = math_paths(&self.doc.maths()[i]);
            ductus_render::render_strokes(pixmap, vp, paths.iter(), clip);
        }
    }

    /// Another Typst or font version may lay formulas out a little differently.
    pub(crate) fn measure_maths(&mut self) {
        let maths: Vec<MathBox> = self.doc.maths().to_vec();
        for math in maths {
            let updated = measured(math.clone());
            if updated != math {
                self.doc.update_math_quietly(updated);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::canvas_for_tests as canvas;
    use ductus_core::Rgba;
    use kurbo::Point;

    #[test]
    fn editing_changes_and_emptying_removes() {
        let mut c = canvas();
        let at = c.view.to_doc(Point::new(100.0, 100.0));
        c.begin_math(at, "typst", Rgba::BLACK);
        c.set_math_draft("x^2", "typst");
        c.commit_math();
        let id = c.document().maths()[0].id;
        assert_eq!(c.math_at(at + kurbo::Vec2::new(4.0, 4.0)), Some(id));
        assert_eq!(c.edit_math(id), Some(("x^2".into(), "typst".into())));
        c.set_math_draft("x^3", "typst");
        assert!(c.commit_math());
        assert_eq!(c.document().maths()[0].source, "x^3");
        c.edit_math(id);
        c.set_math_draft("", "typst");
        c.commit_math();
        assert!(c.document().maths().is_empty());
        c.undo();
        assert_eq!(c.document().maths()[0].source, "x^3");
        c.edit_math(id);
        c.set_math_draft("y", "typst");
        c.cancel_math();
        assert_eq!(c.document().maths()[0].source, "x^3");
    }
}
