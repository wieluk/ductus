use crate::{Document, Image, MathBox, PageSettings, Stroke, StrokeId, TextBox};

#[derive(Debug, Clone)]
pub enum Command {
    AddStroke(Stroke),
    /// With their former indices, ascending, so undo restores the order.
    RemoveStrokes(Vec<(usize, Stroke)>),
    SetSettings {
        old: PageSettings,
        new: PageSettings,
    },
    Replace {
        before: Vec<Stroke>,
        after: Vec<Stroke>,
    },
    AddMany(Vec<Stroke>),
    /// No pieces means the whole stroke was erased.
    Split {
        index: usize,
        original: Stroke,
        pieces: Vec<Stroke>,
    },
    AddImages(Vec<Image>),
    RemoveImages(Vec<(usize, Image)>),
    ReplaceImages {
        before: Vec<Image>,
        after: Vec<Image>,
    },
    AddTexts(Vec<TextBox>),
    RemoveTexts(Vec<(usize, TextBox)>),
    ReplaceTexts {
        before: Vec<TextBox>,
        after: Vec<TextBox>,
    },
    AddMaths(Vec<MathBox>),
    RemoveMaths(Vec<(usize, MathBox)>),
    ReplaceMaths {
        before: Vec<MathBox>,
        after: Vec<MathBox>,
    },
    Batch(Vec<Command>),
}

impl Command {
    pub fn set_settings(doc: &Document, new: PageSettings) -> Option<Command> {
        let old = *doc.settings();
        (old != new).then_some(Command::SetSettings { old, new })
    }

    pub fn split(doc: &Document, id: StrokeId, pieces: Vec<Stroke>) -> Option<Command> {
        let index = doc.index_of(id)?;
        Some(Command::Split {
            index,
            original: doc.strokes()[index].clone(),
            pieces,
        })
    }

    pub fn remove_images(doc: &Document, ids: &[StrokeId]) -> Option<Command> {
        let mut items: Vec<(usize, Image)> = ids
            .iter()
            .filter_map(|id| {
                doc.image_index_of(*id)
                    .map(|i| (i, doc.images()[i].clone()))
            })
            .collect();
        if items.is_empty() {
            return None;
        }
        items.sort_by_key(|(i, _)| *i);
        items.dedup_by_key(|(i, _)| *i);
        Some(Command::RemoveImages(items))
    }

    pub fn remove_texts(doc: &Document, ids: &[StrokeId]) -> Option<Command> {
        let mut items: Vec<(usize, TextBox)> = ids
            .iter()
            .filter_map(|id| doc.text_index_of(*id).map(|i| (i, doc.texts()[i].clone())))
            .collect();
        if items.is_empty() {
            return None;
        }
        items.sort_by_key(|(i, _)| *i);
        items.dedup_by_key(|(i, _)| *i);
        Some(Command::RemoveTexts(items))
    }

    pub fn remove_maths(doc: &Document, ids: &[StrokeId]) -> Option<Command> {
        let mut items: Vec<(usize, MathBox)> = ids
            .iter()
            .filter_map(|id| doc.math_index_of(*id).map(|i| (i, doc.maths()[i].clone())))
            .collect();
        if items.is_empty() {
            return None;
        }
        items.sort_by_key(|(i, _)| *i);
        items.dedup_by_key(|(i, _)| *i);
        Some(Command::RemoveMaths(items))
    }

    pub fn batch(commands: impl IntoIterator<Item = Option<Command>>) -> Option<Command> {
        let mut commands: Vec<Command> = commands.into_iter().flatten().collect();
        match commands.len() {
            0 => None,
            1 => commands.pop(),
            _ => Some(Command::Batch(commands)),
        }
    }

    pub fn remove(doc: &Document, ids: &[StrokeId]) -> Option<Command> {
        let mut items: Vec<(usize, Stroke)> = ids
            .iter()
            .filter_map(|id| doc.index_of(*id).map(|i| (i, doc.strokes()[i].clone())))
            .collect();
        if items.is_empty() {
            return None;
        }
        items.sort_by_key(|(i, _)| *i);
        items.dedup_by_key(|(i, _)| *i);
        Some(Command::RemoveStrokes(items))
    }

    pub fn apply(&self, doc: &mut Document) {
        match self {
            Command::AddStroke(stroke) => {
                let end = doc.strokes().len();
                doc.insert(end, stroke.clone());
            }
            Command::RemoveStrokes(items) => {
                for (_, stroke) in items {
                    doc.remove(stroke.id);
                }
            }
            Command::SetSettings { new, .. } => doc.set_settings(*new),
            Command::Replace { after, .. } => doc.replace(after),
            Command::AddMany(strokes) => {
                for stroke in strokes {
                    let end = doc.strokes().len();
                    doc.insert(end, stroke.clone());
                }
            }
            Command::Split {
                index,
                original,
                pieces,
            } => {
                doc.remove(original.id);
                for (k, piece) in pieces.iter().enumerate() {
                    doc.insert(index + k, piece.clone());
                }
            }
            Command::AddImages(images) => {
                for image in images {
                    doc.insert_image(usize::MAX, image.clone());
                }
            }
            Command::RemoveImages(items) => {
                for (_, image) in items {
                    doc.remove_image(image.id);
                }
            }
            Command::ReplaceImages { after, .. } => doc.replace_images(after),
            Command::AddTexts(texts) => {
                for text in texts {
                    doc.insert_text(usize::MAX, text.clone());
                }
            }
            Command::RemoveTexts(items) => {
                for (_, text) in items {
                    doc.remove_text(text.id);
                }
            }
            Command::ReplaceTexts { after, .. } => doc.replace_texts(after),
            Command::AddMaths(maths) => {
                for math in maths {
                    doc.insert_math(usize::MAX, math.clone());
                }
            }
            Command::RemoveMaths(items) => {
                for (_, math) in items {
                    doc.remove_math(math.id);
                }
            }
            Command::ReplaceMaths { after, .. } => doc.replace_maths(after),
            Command::Batch(commands) => commands.iter().for_each(|c| c.apply(doc)),
        }
    }

    pub fn revert(&self, doc: &mut Document) {
        match self {
            Command::AddStroke(stroke) => {
                doc.remove(stroke.id);
            }
            Command::RemoveStrokes(items) => {
                // Ascending order restores every stroke to its original index.
                for (index, stroke) in items {
                    doc.insert(*index, stroke.clone());
                }
            }
            Command::SetSettings { old, .. } => doc.set_settings(*old),
            Command::Replace { before, .. } => doc.replace(before),
            Command::AddMany(strokes) => {
                for stroke in strokes.iter().rev() {
                    doc.remove(stroke.id);
                }
            }
            Command::Split {
                index,
                original,
                pieces,
            } => {
                for piece in pieces {
                    doc.remove(piece.id);
                }
                doc.insert(*index, original.clone());
            }
            Command::AddImages(images) => {
                for image in images.iter().rev() {
                    doc.remove_image(image.id);
                }
            }
            Command::RemoveImages(items) => {
                for (index, image) in items {
                    doc.insert_image(*index, image.clone());
                }
            }
            Command::ReplaceImages { before, .. } => doc.replace_images(before),
            Command::AddTexts(texts) => {
                for text in texts.iter().rev() {
                    doc.remove_text(text.id);
                }
            }
            Command::RemoveTexts(items) => {
                for (index, text) in items {
                    doc.insert_text(*index, text.clone());
                }
            }
            Command::ReplaceTexts { before, .. } => doc.replace_texts(before),
            Command::AddMaths(maths) => {
                for math in maths.iter().rev() {
                    doc.remove_math(math.id);
                }
            }
            Command::RemoveMaths(items) => {
                for (index, math) in items {
                    doc.insert_math(*index, math.clone());
                }
            }
            Command::ReplaceMaths { before, .. } => doc.replace_maths(before),
            Command::Batch(commands) => commands.iter().rev().for_each(|c| c.revert(doc)),
        }
    }
}

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
    /// Tells callers whether the page changed since it was saved.
    revision: u64,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, doc: &mut Document, cmd: Command) {
        cmd.apply(doc);
        self.undo.push(cmd);
        self.redo.clear();
        self.revision += 1;
    }

    /// For commands the caller has already applied.
    pub fn record(&mut self, cmd: Command) {
        self.undo.push(cmd);
        self.redo.clear();
        self.revision += 1;
    }

    pub fn undo(&mut self, doc: &mut Document) -> bool {
        let Some(cmd) = self.undo.pop() else {
            return false;
        };
        cmd.revert(doc);
        self.redo.push(cmd);
        self.revision += 1;
        true
    }

    pub fn redo(&mut self, doc: &mut Document) -> bool {
        let Some(cmd) = self.redo.pop() else {
            return false;
        };
        cmd.apply(doc);
        self.undo.push(cmd);
        self.revision += 1;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InkPoint, StrokeStyle};

    fn stroke(doc: &mut Document) -> Stroke {
        Stroke {
            id: doc.alloc_id(),
            style: StrokeStyle::default(),
            points: vec![InkPoint {
                x: 0.0,
                y: 0.0,
                pressure: 0.5,
                tilt_x: 0.0,
                tilt_y: 0.0,
                t: 0,
            }],
        }
    }

    fn ids(doc: &Document) -> Vec<u64> {
        doc.strokes().iter().map(|s| s.id.0).collect()
    }

    #[test]
    fn add_undo_redo() {
        let mut doc = Document::new();
        let mut h = History::new();
        let s = stroke(&mut doc);
        h.apply(&mut doc, Command::AddStroke(s));
        assert_eq!(ids(&doc), [1]);
        assert!(h.undo(&mut doc));
        assert!(doc.strokes().is_empty());
        assert!(h.redo(&mut doc));
        assert_eq!(ids(&doc), [1]);
        assert!(!h.redo(&mut doc));
    }

    #[test]
    fn remove_restores_original_order() {
        let mut doc = Document::new();
        let mut h = History::new();
        for _ in 0..5 {
            let s = stroke(&mut doc);
            h.apply(&mut doc, Command::AddStroke(s));
        }
        let cmd = Command::remove(&doc, &[StrokeId(4), StrokeId(2), StrokeId(99)]).unwrap();
        h.apply(&mut doc, cmd);
        assert_eq!(ids(&doc), [1, 3, 5]);
        h.undo(&mut doc);
        assert_eq!(ids(&doc), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn new_command_clears_redo() {
        let mut doc = Document::new();
        let mut h = History::new();
        let s = stroke(&mut doc);
        h.apply(&mut doc, Command::AddStroke(s));
        h.undo(&mut doc);
        let s = stroke(&mut doc);
        h.apply(&mut doc, Command::AddStroke(s));
        assert!(!h.can_redo());
        assert_eq!(h.revision(), 3);
    }

    #[test]
    fn batched_splits_undo_in_one_step() {
        let mut doc = Document::new();
        let mut h = History::new();
        for _ in 0..3 {
            let s = stroke(&mut doc);
            h.apply(&mut doc, Command::AddStroke(s));
        }
        let mut steps = Vec::new();
        let (a, b) = (stroke(&mut doc), stroke(&mut doc));
        let targets = [
            (StrokeId(2), vec![a.clone(), b]),
            (a.id, vec![]),
            (StrokeId(3), vec![]),
        ];
        for (id, pieces) in targets {
            let cmd = Command::split(&doc, id, pieces).unwrap();
            cmd.apply(&mut doc);
            steps.push(cmd);
        }
        assert_eq!(ids(&doc), [1, 5]);
        h.record(Command::Batch(steps));
        h.undo(&mut doc);
        assert_eq!(ids(&doc), [1, 2, 3]);
        h.redo(&mut doc);
        assert_eq!(ids(&doc), [1, 5]);
    }
}
