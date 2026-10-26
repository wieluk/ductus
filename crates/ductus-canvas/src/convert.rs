use crate::{Canvas, TileLayer};
use ductus_core::{Command, MathBox, Stroke, StrokeId, TextBox, Tool};

impl Canvas {
    /// The selected pen strokes; highlights stay as they are.
    pub fn selected_ink(&self) -> Vec<Stroke> {
        self.doc
            .strokes()
            .iter()
            .filter(|s| s.style.tool == Tool::Pen && self.selection.contains(&s.id))
            .cloned()
            .collect()
    }

    /// Replaces the selected ink with recognized text, or with a LaTeX formula, as one undo step.
    pub fn replace_ink(&mut self, content: &str, math: bool) -> bool {
        let ink = self.selected_ink();
        let Some(bounds) = ink.iter().map(ductus_ink::bounds).reduce(|a, b| a.union(b)) else {
            return false;
        };
        if content.trim().is_empty() {
            return false;
        }
        let ids: Vec<StrokeId> = ink.iter().map(|s| s.id).collect();
        let id = self.doc.alloc_id();
        let added = if math {
            let at_one = crate::math::measured(MathBox {
                id,
                source: ductus_math::strip_delimiters(content).to_owned(),
                syntax: "latex".into(),
                x: bounds.x0,
                y: bounds.y0,
                width: 0.0,
                height: 0.0,
                scale: 1.0,
                color: self.pen.color,
            });
            // About as tall as the handwriting it replaces.
            let scale = match at_one.height > 0.0 {
                true => (bounds.height() / at_one.height).clamp(0.8, 4.0),
                false => 1.5,
            };
            Command::AddMaths(vec![crate::math::measured(MathBox { scale, ..at_one })])
        } else {
            let lines = content.lines().count().max(1) as f64;
            Command::AddTexts(vec![TextBox {
                id,
                markdown: content.to_owned(),
                x: bounds.x0,
                y: bounds.y0,
                width: bounds.width().max(160.0),
                height: lines * TextBox::FONT_SIZE * 1.5,
                scale: 1.0,
            }])
        };
        let Some(cmd) = Command::batch([Command::remove(&self.doc, &ids), Some(added)]) else {
            return false;
        };
        self.clear_selection();
        self.tiles
            .invalidate(TileLayer::Base, bounds.inflate(8.0, 8.0));
        self.history.apply(&mut self.doc, cmd);
        self.sync_layout();
        true
    }
}
