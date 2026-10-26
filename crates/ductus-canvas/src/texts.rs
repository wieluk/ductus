//! The frontend lays out and draws text (it owns the fonts); the canvas keeps boxes, undo and selection.

use crate::Canvas;
use crate::selection::{text_rect, transform_text};
use ductus_core::{Command, StrokeId, TextBox};
use kurbo::{Point, Rect};

#[derive(Debug, Clone, PartialEq)]
pub struct TextView {
    pub id: StrokeId,
    pub markdown: String,
    pub screen: Rect,
    pub scale: f64,
    pub selected: bool,
}

impl Canvas {
    pub fn text_at(&self, at: Point) -> Option<StrokeId> {
        self.doc
            .texts()
            .iter()
            .rev()
            .find(|t| text_rect(t).contains(at))
            .map(|t| t.id)
    }

    pub fn add_text(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        markdown: &str,
    ) -> Option<StrokeId> {
        if markdown.trim().is_empty() {
            return None;
        }
        let text = TextBox {
            id: self.doc.alloc_id(),
            markdown: markdown.to_owned(),
            x,
            y,
            width,
            height,
            scale: 1.0,
        };
        let id = text.id;
        self.history
            .apply(&mut self.doc, Command::AddTexts(vec![text]));
        self.sync_layout();
        Some(id)
    }

    pub fn set_text(&mut self, id: StrokeId, markdown: &str, height: f64) -> bool {
        let Some(index) = self.doc.text_index_of(id) else {
            return false;
        };
        let before = self.doc.texts()[index].clone();
        let cmd = if markdown.trim().is_empty() {
            self.selection.retain(|s| *s != id);
            Command::remove_texts(&self.doc, &[id])
        } else if before.markdown == markdown && (before.height - height).abs() < 0.01 {
            None
        } else {
            Some(Command::ReplaceTexts {
                after: vec![TextBox {
                    markdown: markdown.to_owned(),
                    height,
                    ..before.clone()
                }],
                before: vec![before],
            })
        };
        let Some(cmd) = cmd else {
            return false;
        };
        self.history.apply(&mut self.doc, cmd);
        self.sync_layout();
        self.redraw_overlay();
        true
    }

    /// No undo step: fonts or window scaling changed the layout, not the text.
    pub fn set_text_height(&mut self, id: StrokeId, height: f64) {
        if let Some(index) = self.doc.text_index_of(id) {
            let text = &self.doc.texts()[index];
            if (text.height - height).abs() >= 0.5 {
                let updated = TextBox {
                    height,
                    ..text.clone()
                };
                self.doc.update_text_quietly(updated);
                self.redraw_overlay();
            }
        }
    }

    pub fn text_views(&self) -> Vec<TextView> {
        let dragging = self.transforming();
        self.doc
            .texts()
            .iter()
            .map(|t| {
                let selected = self.selection.contains(&t.id);
                let shown = if dragging && selected {
                    transform_text(t, self.selection_transform)
                } else {
                    t.clone()
                };
                TextView {
                    id: t.id,
                    markdown: t.markdown.clone(),
                    screen: self.view.to_screen_rect(text_rect(&shown)),
                    scale: self.view.zoom * shown.scale,
                    selected,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::Canvas;
    use kurbo::Point;

    fn canvas() -> Canvas {
        let mut c = Canvas::new();
        c.set_viewport(400.0, 300.0, 1.0);
        c
    }

    #[test]
    fn adding_editing_and_emptying_are_undoable() {
        let mut c = canvas();
        assert!(c.add_text(10.0, 20.0, 200.0, 21.0, "  ").is_none());
        let id = c.add_text(10.0, 20.0, 200.0, 21.0, "Limits").unwrap();
        assert_eq!(c.text_at(Point::new(50.0, 30.0)), Some(id));
        assert!(!c.set_text(id, "Limits", 21.0), "no change, no undo step");
        assert!(c.set_text(id, "**Limits**\n\nof sequences", 63.0));
        assert_eq!(c.document().texts()[0].height, 63.0);
        assert!(c.set_text(id, "", 0.0));
        assert!(c.document().texts().is_empty());
        c.undo();
        assert_eq!(
            c.document().texts()[0].markdown,
            "**Limits**\n\nof sequences"
        );
        c.undo();
        c.undo();
        assert!(c.document().texts().is_empty());
    }
}
