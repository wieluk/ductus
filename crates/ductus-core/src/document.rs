use crate::{
    Image, MathBox, PageKind, PageSettings, SHEET_GAP, SheetRect, Stroke, StrokeId, TextBox,
    stack_sheets,
};

#[derive(Debug, Default, Clone)]
pub struct Document {
    strokes: Vec<Stroke>,
    images: Vec<Image>,
    texts: Vec<TextBox>,
    maths: Vec<MathBox>,
    next_id: u64,
    settings: PageSettings,
    pdf_sheets: Vec<(f64, f64)>,
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ids are reassigned so they stay unique within the document.
    pub fn with_contents(
        settings: PageSettings,
        strokes: impl IntoIterator<Item = Stroke>,
    ) -> Self {
        let mut doc = Self {
            settings,
            ..Self::default()
        };
        for mut stroke in strokes {
            stroke.id = doc.alloc_id();
            doc.strokes.push(stroke);
        }
        doc
    }

    pub fn with_images(mut self, images: impl IntoIterator<Item = Image>) -> Self {
        for mut image in images {
            image.id = self.alloc_id();
            self.images.push(image);
        }
        self
    }

    pub fn with_texts(mut self, texts: impl IntoIterator<Item = TextBox>) -> Self {
        for mut text in texts {
            text.id = self.alloc_id();
            self.texts.push(text);
        }
        self
    }

    pub fn with_maths(mut self, maths: impl IntoIterator<Item = MathBox>) -> Self {
        for mut math in maths {
            math.id = self.alloc_id();
            self.maths.push(math);
        }
        self
    }

    pub fn maths(&self) -> &[MathBox] {
        &self.maths
    }

    pub fn math_index_of(&self, id: StrokeId) -> Option<usize> {
        self.maths.iter().position(|m| m.id == id)
    }

    pub(crate) fn insert_math(&mut self, index: usize, math: MathBox) {
        self.next_id = self.next_id.max(math.id.0);
        self.maths.insert(index.min(self.maths.len()), math);
    }

    pub(crate) fn remove_math(&mut self, id: StrokeId) -> Option<(usize, MathBox)> {
        let index = self.math_index_of(id)?;
        Some((index, self.maths.remove(index)))
    }

    pub(crate) fn replace_maths(&mut self, maths: &[MathBox]) {
        for math in maths {
            if let Some(i) = self.math_index_of(math.id) {
                self.maths[i] = math.clone();
            }
        }
    }

    /// Outside the undo history: the size is derived from the source.
    pub fn update_math_quietly(&mut self, math: MathBox) {
        if let Some(i) = self.math_index_of(math.id) {
            self.maths[i] = math;
        }
    }

    pub fn texts(&self) -> &[TextBox] {
        &self.texts
    }

    pub fn text_index_of(&self, id: StrokeId) -> Option<usize> {
        self.texts.iter().position(|t| t.id == id)
    }

    pub(crate) fn insert_text(&mut self, index: usize, text: TextBox) {
        self.next_id = self.next_id.max(text.id.0);
        self.texts.insert(index.min(self.texts.len()), text);
    }

    pub(crate) fn remove_text(&mut self, id: StrokeId) -> Option<(usize, TextBox)> {
        let index = self.text_index_of(id)?;
        Some((index, self.texts.remove(index)))
    }

    /// Outside the undo history: the height is derived from the contents.
    pub fn update_text_quietly(&mut self, text: TextBox) {
        if let Some(i) = self.text_index_of(text.id) {
            self.texts[i] = text;
        }
    }

    pub(crate) fn replace_texts(&mut self, texts: &[TextBox]) {
        for text in texts {
            if let Some(i) = self.text_index_of(text.id) {
                self.texts[i] = text.clone();
            }
        }
    }

    pub fn strokes(&self) -> &[Stroke] {
        &self.strokes
    }

    pub fn images(&self) -> &[Image] {
        &self.images
    }

    pub fn settings(&self) -> &PageSettings {
        &self.settings
    }

    pub fn content_bottom(&self) -> f64 {
        self.strokes
            .iter()
            .flat_map(|s| s.points.iter().map(move |p| p.y + s.style.width / 2.0))
            .chain(self.images.iter().map(|i| i.y + i.height))
            .chain(self.texts.iter().map(|t| t.y + t.height))
            .chain(self.maths.iter().map(|m| m.y + m.height))
            .fold(0.0, f64::max)
    }

    pub fn sheets_used(&self) -> Option<usize> {
        let (_, h) = self.settings.kind.sheet_size()?;
        let pitch = h + SHEET_GAP;
        Some((self.content_bottom() / pitch).floor() as usize + 1)
    }

    /// Moves everything that starts below `y` down by `dy`, making room for a new sheet.
    pub fn shift_below(&mut self, y: f64, dy: f64) {
        for stroke in &mut self.strokes {
            let top = stroke
                .points
                .iter()
                .map(|p| p.y)
                .fold(f64::INFINITY, f64::min);
            if top >= y {
                for p in &mut stroke.points {
                    p.y += dy;
                }
            }
        }
        for image in self.images.iter_mut().filter(|i| i.y >= y) {
            image.y += dy;
        }
        for text in self.texts.iter_mut().filter(|t| t.y >= y) {
            text.y += dy;
        }
        for math in self.maths.iter_mut().filter(|m| m.y >= y) {
            math.y += dy;
        }
    }

    pub fn set_pdf_sheets(&mut self, sizes: Vec<(f64, f64)>) {
        self.pdf_sheets = sizes;
    }

    pub fn pdf_sheets(&self) -> &[(f64, f64)] {
        &self.pdf_sheets
    }

    pub fn sheets(&self, spare: usize) -> Vec<SheetRect> {
        match self.settings.kind {
            PageKind::Endless => Vec::new(),
            PageKind::Pdf => stack_sheets(&self.pdf_sheets),
            PageKind::Paper { .. } => {
                let size = self.settings.kind.sheet_size().expect("paper has a size");
                let count = self.sheets_used().unwrap_or(1) + spare;
                stack_sheets(&vec![size; count])
            }
        }
    }

    pub fn alloc_id(&mut self) -> StrokeId {
        self.next_id += 1;
        StrokeId(self.next_id)
    }

    pub fn index_of(&self, id: StrokeId) -> Option<usize> {
        self.strokes.iter().position(|s| s.id == id)
    }

    pub(crate) fn insert(&mut self, index: usize, stroke: Stroke) {
        self.next_id = self.next_id.max(stroke.id.0);
        self.strokes.insert(index.min(self.strokes.len()), stroke);
    }

    pub(crate) fn remove(&mut self, id: StrokeId) -> Option<(usize, Stroke)> {
        let index = self.index_of(id)?;
        Some((index, self.strokes.remove(index)))
    }

    pub(crate) fn replace(&mut self, strokes: &[Stroke]) {
        for stroke in strokes {
            if let Some(i) = self.index_of(stroke.id) {
                self.strokes[i] = stroke.clone();
            }
        }
    }

    pub fn image_index_of(&self, id: StrokeId) -> Option<usize> {
        self.images.iter().position(|i| i.id == id)
    }

    pub(crate) fn insert_image(&mut self, index: usize, image: Image) {
        self.next_id = self.next_id.max(image.id.0);
        self.images.insert(index.min(self.images.len()), image);
    }

    pub(crate) fn remove_image(&mut self, id: StrokeId) -> Option<(usize, Image)> {
        let index = self.image_index_of(id)?;
        Some((index, self.images.remove(index)))
    }

    pub(crate) fn replace_images(&mut self, images: &[Image]) {
        for image in images {
            if let Some(i) = self.image_index_of(image.id) {
                self.images[i] = image.clone();
            }
        }
    }

    pub(crate) fn set_settings(&mut self, settings: PageSettings) {
        self.settings = settings;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InkPoint, StrokeStyle};

    fn text_at(y: f64) -> TextBox {
        TextBox {
            id: StrokeId(0),
            markdown: "note".into(),
            x: 10.0,
            y,
            width: 100.0,
            height: 20.0,
            scale: 1.0,
        }
    }

    #[test]
    fn shifting_moves_only_what_starts_below() {
        let point = |y: f64| InkPoint {
            x: 5.0,
            y,
            pressure: 0.5,
            tilt_x: 0.0,
            tilt_y: 0.0,
            t: 0,
        };
        let stroke = |y0: f64, y1: f64| Stroke {
            id: StrokeId(0),
            style: StrokeStyle::default(),
            points: vec![point(y0), point(y1)],
        };
        let mut doc = Document::with_contents(
            PageSettings::default(),
            [stroke(10.0, 50.0), stroke(120.0, 150.0)],
        )
        .with_texts([text_at(20.0), text_at(130.0)]);
        doc.shift_below(100.0, 500.0);
        assert_eq!(doc.strokes()[0].points[1].y, 50.0);
        assert_eq!(doc.strokes()[1].points[0].y, 620.0);
        assert_eq!(doc.texts()[0].y, 20.0);
        assert_eq!(doc.texts()[1].y, 630.0);
    }
}
