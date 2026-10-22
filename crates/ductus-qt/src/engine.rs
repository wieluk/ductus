use crate::ffi::qobject::{LineJob, PointerSample, TextRow, TileInfo};
use ductus_canvas::{Canvas, Device, EraserMode, Sample, TILE_SIZE, TileKey, TileLayer, ToolMode};
use ductus_core::{Orientation, PageKind, PageSettings, PaperSize, PatternStyle, Rgba, StrokeId};
use kurbo::{Point, Vec2};

pub struct CanvasEngine(Canvas);

pub fn new_canvas_engine() -> Box<CanvasEngine> {
    Box::new(CanvasEngine(Canvas::new()))
}

fn sample(s: &PointerSample) -> Sample {
    Sample {
        x: s.x,
        y: s.y,
        pressure: s.has_pressure.then_some(s.pressure),
        // Qt reports degrees; the document stores tilt normalized to -1..1.
        tilt_x: s.tilt_x / 90.0,
        tilt_y: s.tilt_y / 90.0,
        time_ms: s.time_ms,
    }
}

fn layer(index: u8) -> TileLayer {
    if index == 0 {
        TileLayer::Base
    } else {
        TileLayer::Overlay
    }
}

pub(crate) fn styled(
    mut settings: PageSettings,
    kind: &str,
    landscape: bool,
    pattern: &str,
) -> PageSettings {
    let orientation = if landscape {
        Orientation::Landscape
    } else {
        Orientation::Portrait
    };
    if kind == "endless" {
        settings.kind = PageKind::Endless;
    } else if let Some(size) = PaperSize::from_name(kind) {
        settings.kind = PageKind::Paper { size, orientation };
    }
    if let Some(p) = PatternStyle::from_name(pattern) {
        if p != settings.background.pattern {
            settings.background.pattern = p;
            settings.background.spacing = p.default_spacing();
        }
    }
    settings
}

impl CanvasEngine {
    pub(crate) fn canvas(&self) -> &Canvas {
        &self.0
    }

    pub(crate) fn canvas_mut(&mut self) -> &mut Canvas {
        &mut self.0
    }

    pub fn page_kind(&self) -> String {
        match self.0.page_settings().kind {
            PageKind::Endless => "endless".into(),
            PageKind::Paper { size, .. } => size.name().into(),
            PageKind::Pdf => "pdf".into(),
        }
    }

    pub fn page_landscape(&self) -> bool {
        matches!(
            self.0.page_settings().kind,
            PageKind::Paper {
                orientation: Orientation::Landscape,
                ..
            }
        )
    }

    pub fn page_pattern(&self) -> String {
        self.0.page_settings().background.pattern.name().into()
    }

    pub fn set_page_style(&mut self, kind: &str, landscape: bool, pattern: &str) {
        let settings = *self.0.page_settings();
        if settings.kind == PageKind::Pdf {
            return; // the PDF defines the sheets
        }
        self.0
            .set_page_settings(styled(settings, kind, landscape, pattern));
    }

    pub fn set_pressure_gamma(&mut self, gamma: f64) {
        self.0.set_pressure_gamma(gamma);
    }

    pub fn set_smoothing_level(&mut self, level: u8) {
        self.0.set_smoothing_level(level);
    }

    pub fn set_desk_color(&mut self, r: f32, g: f32, b: f32) {
        self.0.set_desk(Rgba::rgb(r, g, b));
    }

    pub fn set_viewport(&mut self, width: f64, height: f64, dpr: f64) {
        self.0.set_viewport(width, height, dpr);
    }

    pub fn frame(&mut self) -> Vec<TileInfo> {
        self.0
            .frame()
            .into_iter()
            .map(|t| TileInfo {
                layer: u8::from(t.key.layer == TileLayer::Overlay),
                x: t.key.x,
                y: t.key.y,
                generation: t.generation,
                rect_x: t.rect.x0,
                rect_y: t.rect.y0,
                rect_w: t.rect.width(),
                rect_h: t.rect.height(),
            })
            .collect()
    }

    pub fn tile_pixels(&self, layer_index: u8, x: i32, y: i32) -> &[u8] {
        let key = TileKey {
            layer: layer(layer_index),
            x,
            y,
        };
        self.0.tile_pixels(key).unwrap_or_default()
    }

    pub fn tile_size(&self) -> u32 {
        TILE_SIZE
    }

    pub fn pointer_down(&mut self, device: u8, s: &PointerSample, erase_button: bool) -> bool {
        let device = match device {
            0 => Device::Pen,
            1 => Device::PenEraser,
            2 => Device::Mouse,
            _ => Device::Touch,
        };
        self.0.pointer_down(device, sample(s), erase_button)
    }

    pub fn pointer_move(&mut self, samples: &[PointerSample]) {
        let samples: Vec<_> = samples.iter().map(sample).collect();
        self.0.pointer_move(&samples);
    }

    pub fn pointer_up(&mut self) {
        self.0.pointer_up();
    }

    pub fn pointer_cancel(&mut self) {
        self.0.pointer_cancel();
    }

    pub fn is_drawing(&self) -> bool {
        self.0.is_drawing()
    }

    pub fn pan_by(&mut self, dx: f64, dy: f64) {
        self.0.pan_by(dx, dy);
    }

    pub fn zoom_around(&mut self, x: f64, y: f64, zoom: f64) {
        self.0.zoom_around(Point::new(x, y), zoom);
    }

    pub fn set_view(&mut self, zoom: f64, offset_x: f64, offset_y: f64) {
        self.0.set_view(zoom, Vec2::new(offset_x, offset_y));
    }

    pub fn zoom(&self) -> f64 {
        self.0.view().zoom
    }

    pub fn offset_x(&self) -> f64 {
        self.0.view().offset.x
    }

    pub fn offset_y(&self) -> f64 {
        self.0.view().offset.y
    }

    pub fn reset_view(&mut self) {
        self.0.reset_view();
    }

    pub fn needs_settle(&self) -> bool {
        self.0.needs_settle()
    }

    pub fn settle(&mut self) {
        self.0.settle();
    }

    pub fn set_tool(&mut self, tool: u8) {
        self.0.set_tool(match tool {
            1 => ToolMode::Highlighter,
            2 => ToolMode::Eraser,
            3 => ToolMode::Lasso,
            4 => ToolMode::Hand,
            _ => ToolMode::Pen,
        });
    }

    pub fn set_highlighter(&mut self, r: f32, g: f32, b: f32, width: f64) {
        self.0.set_highlighter_color(Rgba::rgb(r, g, b));
        self.0.set_highlighter_width(width);
    }

    pub fn set_pen_pattern(&mut self, name: &str) {
        self.0
            .set_pen_pattern(ductus_core::LinePattern::from_name(name).unwrap_or_default());
    }

    pub fn set_eraser_size(&mut self, radius: f64) {
        self.0.set_eraser_radius(radius);
    }

    pub fn set_eraser_whole_strokes(&mut self, on: bool) {
        self.0.set_eraser_mode(if on {
            EraserMode::Stroke
        } else {
            EraserMode::Partial
        });
    }

    pub fn hold(&mut self) -> String {
        self.0.hold().unwrap_or_default().into()
    }

    pub fn selection_count(&self) -> usize {
        self.0.selection_count()
    }

    pub fn text_rows(&self) -> Vec<TextRow> {
        self.0
            .text_views()
            .into_iter()
            .map(|t| TextRow {
                id: t.id.0,
                markdown: t.markdown,
                x: t.screen.x0,
                y: t.screen.y0,
                width: t.screen.width(),
                height: t.screen.height(),
                scale: t.scale,
                selected: t.selected,
            })
            .collect()
    }

    pub fn texts_of_page(&self) -> Vec<TextRow> {
        self.0
            .document()
            .texts()
            .iter()
            .map(|t| TextRow {
                id: t.id.0,
                markdown: t.markdown.clone(),
                x: t.x,
                y: t.y,
                width: t.width,
                height: t.height,
                scale: t.scale,
                selected: false,
            })
            .collect()
    }

    pub fn add_text(&mut self, x: f64, y: f64, width: f64, height: f64, markdown: &str) -> u64 {
        self.0
            .add_text(x, y, width, height, markdown)
            .map_or(0, |id| id.0)
    }

    pub fn set_text(&mut self, id: u64, markdown: &str, height: f64) -> bool {
        self.0.set_text(StrokeId(id), markdown, height)
    }

    pub fn set_text_height(&mut self, id: u64, height: f64) {
        self.0.set_text_height(StrokeId(id), height);
    }

    pub fn text_at(&self, x: f64, y: f64) -> u64 {
        let at = self.0.view().to_doc(Point::new(x, y));
        self.0.text_at(at).map_or(0, |id| id.0)
    }

    pub fn to_doc(&self, x: f64, y: f64) -> Vec<f64> {
        let p = self.0.view().to_doc(Point::new(x, y));
        vec![p.x, p.y]
    }

    pub fn begin_math(&mut self, x: f64, y: f64, syntax: &str, r: f32, g: f32, b: f32) {
        let at = self.0.view().to_doc(Point::new(x, y));
        self.0.begin_math(at, syntax, Rgba::rgb(r, g, b));
    }

    pub fn edit_math(&mut self, id: u64) -> Vec<String> {
        self.0
            .edit_math(StrokeId(id))
            .map(|(source, syntax)| vec![source, syntax])
            .unwrap_or_default()
    }

    pub fn set_math_draft(&mut self, source: &str, syntax: &str) -> String {
        self.0.set_math_draft(source, syntax)
    }

    pub fn commit_math(&mut self) -> bool {
        self.0.commit_math()
    }

    pub fn cancel_math(&mut self) {
        self.0.cancel_math();
    }

    pub fn math_at(&self, x: f64, y: f64) -> u64 {
        let at = self.0.view().to_doc(Point::new(x, y));
        self.0.math_at(at).map_or(0, |id| id.0)
    }

    pub fn math_draft_rect(&self) -> Vec<f64> {
        self.0
            .math_draft_screen_rect()
            .map(|r| vec![r.x0, r.y0, r.width(), r.height()])
            .unwrap_or_default()
    }

    pub fn focus_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.0.focus_rect(kurbo::Rect::new(x, y, x + w, y + h));
    }

    pub fn focus_sheet(&mut self, index: usize) {
        self.0.focus_sheet(index);
    }

    pub fn clear_mark(&mut self) {
        self.0.clear_mark();
    }

    pub fn has_selection_drag(&self) -> bool {
        self.0.transforming()
    }

    pub fn selection_has_ink(&self) -> bool {
        self.0.selection_has_ink()
    }

    pub fn selection_rect(&self) -> Vec<f64> {
        self.0
            .selection_screen_rect()
            .map(|r| vec![r.x0, r.y0, r.width(), r.height()])
            .unwrap_or_default()
    }

    pub fn clear_selection(&mut self) {
        self.0.clear_selection();
    }

    pub fn select_all(&mut self) {
        self.0.select_all();
    }

    pub fn delete_selection(&mut self) {
        self.0.delete_selection();
    }

    /// The selected ink as images to read: one per line of text, or one for a formula.
    pub fn selection_jobs(&self, math: bool) -> Vec<LineJob> {
        let ink = self.0.selected_ink();
        let job = |png, rect: kurbo::Rect| LineJob {
            png,
            key: String::new(),
            x: rect.x0,
            y: rect.y0,
            w: rect.width(),
            h: rect.height(),
        };
        if math {
            let all: Vec<usize> = (0..ink.len()).collect();
            let rect = ink.iter().map(ductus_ink::bounds).reduce(|a, b| a.union(b));
            return match (ductus_hwr::ink_png(&ink, &all, 896), rect) {
                (Some(png), Some(rect)) => vec![job(png, rect)],
                _ => Vec::new(),
            };
        }
        ductus_hwr::find_lines(&ink)
            .into_iter()
            .filter_map(|line| Some(job(ductus_hwr::line_png(&ink, &line, 96)?, line.rect)))
            .collect()
    }

    pub fn replace_ink(&mut self, content: &str, math: bool) -> bool {
        self.0.replace_ink(content, math)
    }

    pub fn copy_selection(&mut self) {
        self.0.copy_selection();
    }

    pub fn cut_selection(&mut self) {
        self.0.cut_selection();
    }

    pub fn paste(&mut self) {
        self.0.paste();
    }

    pub fn can_paste(&self) -> bool {
        self.0.can_paste()
    }

    pub fn insert_picture(&mut self, png: &[u8], x: f64, y: f64) -> bool {
        self.0.insert_picture(png.to_vec(), Some(Point::new(x, y)))
    }

    pub fn recolor_selection(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.0.recolor_selection(Rgba { r, g, b, a });
    }

    pub fn tool(&self) -> u8 {
        match self.0.tool() {
            ToolMode::Pen => 0,
            ToolMode::Highlighter => 1,
            ToolMode::Eraser => 2,
            ToolMode::Lasso => 3,
            ToolMode::Hand => 4,
        }
    }

    pub fn set_pen_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.0.set_pen_color(Rgba { r, g, b, a });
    }

    pub fn set_pen_width(&mut self, width: f64) {
        self.0.set_pen_width(width);
    }

    pub fn pen_width(&self) -> f64 {
        self.0.pen().width
    }

    pub fn set_finger_draws(&mut self, on: bool) {
        self.0.set_finger_draws(on);
    }

    pub fn finger_draws(&self) -> bool {
        self.0.finger_draws()
    }

    pub fn undo(&mut self) {
        self.0.undo();
    }

    pub fn redo(&mut self) {
        self.0.redo();
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// No page open: an empty sheet, without undo.
    pub fn unload(&mut self) {
        self.0.load_document(ductus_core::Document::default());
    }

    pub fn can_undo(&self) -> bool {
        self.0.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.0.can_redo()
    }

    pub fn revision(&self) -> u64 {
        self.0.revision()
    }
}
