mod tiles;
mod view;

pub use tiles::{TILE_SIZE, TileKey, TileLayer};
pub use view::{MAX_ZOOM, MIN_ZOOM, View};

use ductus_core::{
    Command, Document, History, InkPoint, PageSettings, Rgba, Stroke, StrokeId, StrokeStyle, Tool,
};
use ductus_ink::{PressureCurve, RawSample, Shape, Smoothing, StrokeBuilder};
use ductus_render::{Layer, PageLayout, SheetPainter, StrokePath, Viewport};
use kurbo::{Affine, Point, Rect, Vec2};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use tiles::{Tile, TileCache};
use tiny_skia::{Pixmap, PixmapPaint, Transform};

const ERASER_RADIUS: f64 = 8.0;
/// Trailing points whose outline still changes while drawing.
const LIVE_TAIL: usize = 4;
const SHAPE_STEP: f64 = 2.0;
/// Small strokes never snap, so resting the pen while writing keeps letters as written.
const MIN_SHAPE_SIZE: f64 = 28.0;
/// Pasted strokes are offset so they don't hide the originals.
const PASTE_OFFSET: f64 = 24.0;
const SELECTION_COLOR: Rgba = Rgba::rgb(0.35, 0.26, 0.75);
/// Tiles kept around the visible ones, so short scrolls back don't re-render.
const TILE_MARGIN: i32 = 2;

// The clipboard is shared between the canvases of a thread (every page and split view), so
// copying on one page and pasting on another works. `Rc` is enough: the UI thread owns them.
thread_local! {
    static CLIPBOARD: Rc<RefCell<selection::Clipboard>> =
        Rc::new(RefCell::new(selection::Clipboard::default()));
}

fn shared_clipboard() -> Rc<RefCell<selection::Clipboard>> {
    CLIPBOARD.with(Rc::clone)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolMode {
    #[default]
    Pen,
    Highlighter,
    Eraser,
    Lasso,
    /// Moves and resizes text, formulas and pictures; elsewhere the page pans.
    Hand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EraserMode {
    #[default]
    Partial,
    Stroke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Pen,
    PenEraser,
    Mouse,
    Touch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub x: f64,
    pub y: f64,
    pub pressure: Option<f64>,
    pub tilt_x: f64,
    pub tilt_y: f64,
    pub time_ms: u32,
}

impl Sample {
    pub fn at(x: f64, y: f64, time_ms: u32) -> Self {
        Self {
            x,
            y,
            pressure: None,
            tilt_x: 0.0,
            tilt_y: 0.0,
            time_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileView {
    pub key: TileKey,
    pub generation: u64,
    pub rect: Rect,
}

enum Gesture {
    Drawing {
        builder: Box<StrokeBuilder>,
        tail: Option<Rect>,
        shape: Option<ShapeDraft>,
        pen: Point,
    },
    Erasing {
        /// Partial erasing edits the document as it goes; these undo it.
        splits: Vec<Command>,
    },
    Lasso {
        points: Vec<Point>,
    },
    MovingSelection {
        start: Point,
    },
    ScalingSelection {
        anchor: Point,
        start: Point,
    },
    RotatingSelection {
        center: Point,
        start: Point,
    },
}

struct ShapeDraft {
    snapped: Shape,
    current: Shape,
    pen_at_snap: Point,
    pressure: f32,
}

impl ShapeDraft {
    fn points(&self) -> Vec<InkPoint> {
        self.current
            .outline_points(SHAPE_STEP)
            .into_iter()
            .enumerate()
            .map(|(i, p)| InkPoint {
                x: p.x,
                y: p.y,
                pressure: self.pressure,
                tilt_x: 0.0,
                tilt_y: 0.0,
                t: i as u32 * 8,
            })
            .collect()
    }
}

pub struct Canvas {
    doc: Document,
    history: History,
    view: View,
    tool: ToolMode,
    pen: StrokeStyle,
    highlighter: StrokeStyle,
    curve: PressureCurve,
    smoothing: Smoothing,
    finger_draws: bool,
    eraser_mode: EraserMode,
    eraser_radius: f64,
    desk: Rgba,
    layout: PageLayout,
    gesture: Option<Gesture>,
    /// Hidden until the gesture ends, then removed in one undo step.
    erased: HashSet<StrokeId>,
    paths: HashMap<StrokeId, Outline>,
    active: Option<StrokePath>,
    /// Pens report faster than the screen refreshes, so the live outline is built per frame.
    active_stale: bool,
    tiles: TileCache,
    painter: Option<Box<dyn SheetPainter>>,
    /// Cached so ink edits don't repaint PDF pages.
    backgrounds: HashMap<(i32, i32), Pixmap>,
    selection: HashSet<StrokeId>,
    selection_transform: Affine,
    clipboard: Rc<RefCell<selection::Clipboard>>,
    mark: Option<Rect>,
    math_draft: Option<math::MathDraft>,
    pictures: HashMap<String, selection::PictureData>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            history: History::new(),
            view: View::default(),
            tool: ToolMode::Pen,
            pen: StrokeStyle::default(),
            highlighter: StrokeStyle {
                tool: Tool::Highlighter,
                color: Rgba {
                    r: 1.0,
                    g: 0.85,
                    b: 0.1,
                    a: 0.4,
                },
                width: 18.0,
                ..StrokeStyle::default()
            },
            curve: PressureCurve::default(),
            smoothing: Smoothing::default(),
            finger_draws: false,
            eraser_mode: EraserMode::default(),
            eraser_radius: ERASER_RADIUS,
            desk: Rgba::rgb(0.86, 0.86, 0.89),
            layout: PageLayout::for_document(&Document::new(), 1),
            painter: None,
            backgrounds: HashMap::new(),
            selection: HashSet::new(),
            selection_transform: Affine::IDENTITY,
            clipboard: shared_clipboard(),
            pictures: HashMap::new(),
            math_draft: None,
            mark: None,
            gesture: None,
            erased: HashSet::new(),
            paths: HashMap::new(),
            active: None,
            active_stale: false,
            tiles: TileCache::new(1.0),
        }
    }

    pub fn tool(&self) -> ToolMode {
        self.tool
    }

    pub fn eraser_mode(&self) -> EraserMode {
        self.eraser_mode
    }

    pub fn set_eraser_mode(&mut self, mode: EraserMode) {
        self.eraser_mode = mode;
    }

    pub fn eraser_radius(&self) -> f64 {
        self.eraser_radius
    }

    pub fn set_eraser_radius(&mut self, radius: f64) {
        self.eraser_radius = radius.clamp(2.0, 80.0);
    }

    pub fn set_tool(&mut self, tool: ToolMode) {
        self.tool = tool;
    }

    pub fn pen(&self) -> &StrokeStyle {
        &self.pen
    }

    pub fn set_pen_color(&mut self, color: Rgba) {
        self.pen.color = color;
    }

    pub fn set_pen_width(&mut self, width: f64) {
        self.pen.width = width.clamp(0.25, 100.0);
    }

    pub fn set_pen_pattern(&mut self, pattern: ductus_core::LinePattern) {
        self.pen.pattern = pattern;
    }

    pub fn highlighter(&self) -> &StrokeStyle {
        &self.highlighter
    }

    pub fn set_highlighter_color(&mut self, color: Rgba) {
        self.highlighter.color = Rgba {
            a: self.highlighter.color.a,
            ..color
        };
    }

    pub fn set_highlighter_width(&mut self, width: f64) {
        self.highlighter.width = width.clamp(2.0, 100.0);
    }

    pub fn set_pressure_gamma(&mut self, gamma: f64) {
        self.curve.gamma = gamma.clamp(0.2, 5.0);
    }

    pub fn set_smoothing_level(&mut self, level: u8) {
        self.smoothing = match level {
            0 => Smoothing {
                enabled: false,
                ..Smoothing::default()
            },
            1 => Smoothing {
                min_cutoff: 5.0,
                ..Smoothing::default()
            },
            3 => Smoothing {
                min_cutoff: 0.8,
                beta: 0.03,
                ..Smoothing::default()
            },
            _ => Smoothing::default(),
        };
    }

    pub fn finger_draws(&self) -> bool {
        self.finger_draws
    }

    pub fn set_finger_draws(&mut self, on: bool) {
        self.finger_draws = on;
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn load_document(&mut self, doc: Document) {
        self.pointer_cancel();
        self.selection.clear();
        self.math_draft = None;
        self.mark = None;
        self.doc = doc;
        self.measure_maths();
        // Pictures are added before the page that shows them is loaded.
        let used: HashSet<&str> = self
            .doc
            .images()
            .iter()
            .map(|i| i.source.as_str())
            .collect();
        self.pictures.retain(|name, _| used.contains(name.as_str()));
        self.history = History::new();
        self.paths.clear();
        self.sync_layout();
        self.tiles.invalidate_layer(TileLayer::Base);
        self.reset_view();
    }

    pub fn page_settings(&self) -> &PageSettings {
        self.doc.settings()
    }

    pub fn set_page_settings(&mut self, settings: PageSettings) {
        let kind_changed = settings.kind != self.doc.settings().kind;
        if let Some(cmd) = Command::set_settings(&self.doc, settings) {
            self.history.apply(&mut self.doc, cmd);
            self.sync_layout();
            if kind_changed {
                self.reset_view();
            }
        }
    }

    pub fn layout(&self) -> &PageLayout {
        &self.layout
    }

    pub fn set_desk(&mut self, color: Rgba) {
        if color != self.desk {
            self.desk = color;
            if self.layout.is_paper() {
                self.repaint_backgrounds();
            }
        }
    }

    pub fn set_sheet_painter(&mut self, painter: Option<Box<dyn SheetPainter>>) {
        self.painter = painter;
        self.repaint_backgrounds();
    }

    fn repaint_backgrounds(&mut self) {
        self.backgrounds.clear();
        self.tiles.invalidate_layer(TileLayer::Base);
    }

    /// Paper pages always show one blank sheet after the last one with ink.
    fn sync_layout(&mut self) {
        let layout = PageLayout::for_document(&self.doc, 1);
        if layout != self.layout {
            self.layout = layout;
            self.repaint_backgrounds();
        }
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn set_viewport(&mut self, width: f64, height: f64, dpr: f64) {
        let dpr_changed = (dpr - self.view.dpr).abs() > f64::EPSILON;
        self.view.width = width;
        self.view.height = height;
        self.view.dpr = dpr;
        if dpr_changed {
            self.settle();
        }
    }

    pub fn pan_by(&mut self, dx: f64, dy: f64) {
        self.view.offset += Vec2::new(dx, dy);
    }

    pub fn zoom_around(&mut self, anchor: Point, zoom: f64) {
        self.view.zoom_around(anchor, zoom);
    }

    pub fn set_view(&mut self, zoom: f64, offset: Vec2) {
        self.view.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.view.offset = offset;
    }

    pub fn reset_view(&mut self) {
        let View {
            width, height, dpr, ..
        } = self.view;
        let mut view = View {
            width,
            height,
            dpr,
            ..View::default()
        };
        if let Some(first) = self.layout.sheet_rects().first() {
            let left = ((width - first.width()) / 2.0).max(24.0);
            view.offset = Vec2::new((left - first.x0).round(), (24.0 - first.y0).round());
        }
        self.view = view;
        self.settle();
    }

    fn target_scale(&self) -> f64 {
        self.view.zoom * self.view.dpr
    }

    pub fn needs_settle(&self) -> bool {
        (self.tiles.scale - self.target_scale()).abs() > 1e-9
    }

    pub fn settle(&mut self) {
        if self.needs_settle() {
            self.tiles.rescale(self.target_scale());
            self.backgrounds.clear();
        }
    }

    pub fn is_drawing(&self) -> bool {
        self.gesture.is_some()
    }

    /// False when nothing starts (touch without finger drawing, say): the caller pans.
    pub fn pointer_down(&mut self, device: Device, sample: Sample, erase_button: bool) -> bool {
        let at = self.view.to_doc(Point::new(sample.x, sample.y));
        if self.tool == ToolMode::Hand && !erase_button && device != Device::PenEraser {
            self.pointer_cancel();
            let gesture = self.begin_hand_gesture(at);
            let grabbed = gesture.is_some();
            self.gesture = gesture;
            self.redraw_overlay();
            return grabbed;
        }
        if device == Device::Touch && !self.finger_draws {
            return false;
        }
        self.pointer_cancel();
        let erase = erase_button || device == Device::PenEraser || self.tool == ToolMode::Eraser;
        if self.tool == ToolMode::Lasso && !erase {
            let gesture = self.begin_selection_gesture(at);
            self.gesture = Some(gesture);
            self.redraw_overlay();
            return true;
        }
        self.clear_selection();
        self.gesture = Some(if erase {
            Gesture::Erasing { splits: Vec::new() }
        } else {
            let style = match self.tool {
                ToolMode::Highlighter => self.highlighter,
                _ => self.pen,
            };
            Gesture::Drawing {
                builder: Box::new(StrokeBuilder::new(
                    style,
                    self.curve,
                    self.smoothing,
                    self.view.zoom,
                )),
                tail: None,
                shape: None,
                pen: at,
            }
        });
        self.pointer_move(&[sample]);
        true
    }

    pub fn pointer_move(&mut self, samples: &[Sample]) {
        if self.gesture.is_none() || samples.is_empty() {
            return;
        }
        let raw: Vec<RawSample> = samples
            .iter()
            .map(|s| {
                let p = self.view.to_doc(Point::new(s.x, s.y));
                RawSample {
                    x: p.x,
                    y: p.y,
                    pressure: s.pressure,
                    tilt_x: s.tilt_x,
                    tilt_y: s.tilt_y,
                    time_ms: s.time_ms,
                }
            })
            .collect();

        let pen = raw
            .last()
            .map(|s| Point::new(s.x, s.y))
            .unwrap_or(Point::ZERO);
        match &mut self.gesture {
            Some(Gesture::Drawing {
                shape: Some(draft),
                pen: last,
                builder,
                ..
            }) => {
                *last = pen;
                draft.current = draft.snapped.adjusted(draft.pen_at_snap, pen);
                self.active = StrokePath::from_points(&draft.points(), builder.style());
                self.redraw_overlay();
            }
            Some(Gesture::Drawing {
                builder,
                tail,
                pen: last,
                ..
            }) => {
                *last = pen;
                for sample in raw {
                    builder.push(sample);
                }
                let new_tail = tail_bounds(builder.points(), builder.style());
                let dirty = match (*tail, new_tail) {
                    (Some(a), Some(b)) => Some(a.union(b)),
                    (a, b) => a.or(b),
                };
                *tail = new_tail;
                self.active_stale = true;
                if let Some(dirty) = dirty {
                    self.tiles.invalidate(TileLayer::Overlay, dirty);
                }
            }
            Some(Gesture::Erasing { splits }) if self.eraser_mode == EraserMode::Partial => {
                let radius = self.eraser_radius / self.view.zoom;
                for sample in &raw {
                    let center = Point::new(sample.x, sample.y);
                    let cuts: Vec<_> = self
                        .doc
                        .strokes()
                        .iter()
                        .filter_map(|s| {
                            ductus_ink::erase_circle(s, center, radius).map(|p| (s.id, p))
                        })
                        .collect();
                    for (id, pieces) in cuts {
                        let style = self.doc.strokes()[self.doc.index_of(id).unwrap()].style;
                        let pieces = pieces
                            .into_iter()
                            .map(|points| Stroke {
                                id: self.doc.alloc_id(),
                                style,
                                points,
                            })
                            .collect();
                        if let Some(cmd) = Command::split(&self.doc, id, pieces) {
                            if let Command::Split { original, .. } = &cmd {
                                self.tiles
                                    .invalidate(TileLayer::Base, ductus_ink::bounds(original));
                            }
                            cmd.apply(&mut self.doc);
                            splits.push(cmd);
                        }
                    }
                }
            }
            Some(Gesture::Erasing { .. }) => {
                let radius = self.eraser_radius / self.view.zoom;
                for sample in &raw {
                    let center = Point::new(sample.x, sample.y);
                    for stroke in self.doc.strokes() {
                        if !self.erased.contains(&stroke.id)
                            && ductus_ink::hits_circle(stroke, center, radius)
                        {
                            self.erased.insert(stroke.id);
                            self.tiles
                                .invalidate(TileLayer::Base, ductus_ink::bounds(stroke));
                        }
                    }
                }
            }
            Some(Gesture::Lasso { points }) => {
                points.extend(raw.iter().map(|s| Point::new(s.x, s.y)));
                self.redraw_overlay();
            }
            Some(Gesture::MovingSelection { start }) => {
                let delta = pen - *start;
                self.selection_transform = Affine::translate(self.clamp_move(delta));
                if self.overlay_shift().is_none() {
                    self.redraw_overlay();
                }
            }
            Some(Gesture::RotatingSelection { center, start }) => {
                let mut angle = (pen - *center).atan2() - (*start - *center).atan2();
                // Snaps to multiples of 15°.
                let step = std::f64::consts::PI / 12.0;
                let snapped = (angle / step).round() * step;
                if (angle - snapped).abs() < 0.05 {
                    angle = snapped;
                }
                let c = center.to_vec2();
                self.selection_transform =
                    Affine::translate(c) * Affine::rotate(angle) * Affine::translate(-c);
                self.redraw_overlay();
            }
            Some(Gesture::ScalingSelection { anchor, start }) => {
                // Proportional: project the pen onto the frame's diagonal.
                let diagonal = *start - *anchor;
                let scale = ((pen - *anchor).dot(diagonal) / diagonal.hypot2()).max(0.05);
                let a = anchor.to_vec2();
                self.selection_transform =
                    Affine::translate(a) * Affine::scale(scale) * Affine::translate(-a);
                self.redraw_overlay();
            }
            None => {}
        }
    }

    pub fn hold(&mut self) -> Option<&'static str> {
        let Some(Gesture::Drawing {
            builder,
            shape: shape @ None,
            pen,
            ..
        }) = &mut self.gesture
        else {
            return None;
        };
        let size = points_bounds(builder.points())?.size() * self.view.zoom;
        if size.width.max(size.height) < MIN_SHAPE_SIZE {
            return None;
        }
        let recognized = ductus_ink::recognize(builder.points())?;
        let mut pressures: Vec<f32> = builder.points().iter().map(|p| p.pressure).collect();
        pressures.sort_by(f32::total_cmp);
        let draft = ShapeDraft {
            current: recognized.clone(),
            snapped: recognized,
            pen_at_snap: *pen,
            pressure: pressures[pressures.len() / 2],
        };
        let kind = draft.current.kind();
        self.active = StrokePath::from_points(&draft.points(), builder.style());
        *shape = Some(draft);
        self.redraw_overlay();
        Some(kind)
    }

    pub fn pointer_up(&mut self) {
        match self.gesture.take() {
            Some(Gesture::Drawing {
                builder,
                shape: Some(draft),
                ..
            }) => {
                let stroke = Stroke {
                    id: self.doc.alloc_id(),
                    style: *builder.style(),
                    points: draft.points(),
                };
                self.commit_stroke(stroke);
            }
            Some(Gesture::Drawing { builder, .. }) => {
                let id = self.doc.alloc_id();
                if let Some(stroke) = builder.finish(id) {
                    if let Some(path) = StrokePath::new(&stroke) {
                        self.tiles.invalidate(TileLayer::Base, path.bounds);
                        self.paths.insert(id, Outline::Built(Some(path)));
                    }
                    self.history
                        .apply(&mut self.doc, Command::AddStroke(stroke));
                    self.sync_layout();
                }
            }
            Some(Gesture::Erasing { splits }) if !splits.is_empty() => {
                self.history.record(Command::Batch(splits));
                self.sync_layout();
            }
            Some(Gesture::Erasing { .. }) => {
                let ids: Vec<_> = self.erased.drain().collect();
                if let Some(cmd) = Command::remove(&self.doc, &ids) {
                    self.history.apply(&mut self.doc, cmd);
                    self.sync_layout();
                }
            }
            Some(Gesture::Lasso { points }) => self.finish_lasso(&points),
            Some(
                Gesture::MovingSelection { .. }
                | Gesture::ScalingSelection { .. }
                | Gesture::RotatingSelection { .. },
            ) => self.finish_transform(),
            None => {}
        }
        self.end_live_layer();
    }

    fn commit_stroke(&mut self, stroke: Stroke) {
        if let Some(path) = StrokePath::new(&stroke) {
            self.tiles.invalidate(TileLayer::Base, path.bounds);
            self.paths.insert(stroke.id, Outline::Built(Some(path)));
        }
        self.history
            .apply(&mut self.doc, Command::AddStroke(stroke));
        self.sync_layout();
    }

    pub fn pointer_cancel(&mut self) {
        if let Some(
            Gesture::MovingSelection { .. }
            | Gesture::ScalingSelection { .. }
            | Gesture::RotatingSelection { .. },
        ) = self.gesture
        {
            self.selection_transform = Affine::IDENTITY;
            if let Some(b) = self.selection_bounds() {
                self.tiles.invalidate(TileLayer::Base, b);
            }
        }
        if let Some(Gesture::Erasing { splits }) = &self.gesture {
            for cmd in splits.iter().rev() {
                if let Command::Split { original, .. } = cmd {
                    self.tiles
                        .invalidate(TileLayer::Base, ductus_ink::bounds(original));
                }
                cmd.revert(&mut self.doc);
            }
        }
        if self.gesture.take().is_some() {
            for id in std::mem::take(&mut self.erased) {
                if let Some(outline) = self.paths.get(&id) {
                    self.tiles.invalidate(TileLayer::Base, outline.bounds());
                }
            }
            self.end_live_layer();
        }
    }

    fn end_live_layer(&mut self) {
        self.active = None;
        self.active_stale = false;
        self.tiles.drop_layer(TileLayer::Overlay);
    }

    fn redraw_overlay(&mut self) {
        self.tiles.drop_layer(TileLayer::Overlay);
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn revision(&self) -> u64 {
        self.history.revision()
    }

    pub fn undo(&mut self) {
        self.cancel_math();
        self.clear_selection();
        if self.history.undo(&mut self.doc) {
            // Moves and recolors keep stroke ids, so cached outlines may be stale.
            self.paths.clear();
            self.tiles.invalidate_layer(TileLayer::Base);
            self.sync_layout();
        }
    }

    pub fn redo(&mut self) {
        self.cancel_math();
        self.clear_selection();
        if self.history.redo(&mut self.doc) {
            // Moves and recolors keep stroke ids, so cached outlines may be stale.
            self.paths.clear();
            self.tiles.invalidate_layer(TileLayer::Base);
            self.sync_layout();
        }
    }

    pub fn clear(&mut self) {
        let ids: Vec<_> = self.doc.strokes().iter().map(|s| s.id).collect();
        if let Some(cmd) = Command::remove(&self.doc, &ids) {
            self.history.apply(&mut self.doc, cmd);
            self.tiles.invalidate_layer(TileLayer::Base);
            self.sync_layout();
        }
    }

    pub fn frame(&mut self) -> Vec<TileView> {
        if self.view.width <= 0.0 || self.view.height <= 0.0 {
            return Vec::new();
        }
        // Showing tiles scaled is fine while a gesture runs, but not by much.
        let ratio = self.target_scale() / self.tiles.scale;
        if !(0.5..=2.0).contains(&ratio) {
            self.tiles.rescale(self.target_scale());
            self.backgrounds.clear();
        }
        if std::mem::take(&mut self.active_stale) {
            if let Some(Gesture::Drawing {
                builder,
                shape: None,
                ..
            }) = &self.gesture
            {
                self.active = StrokePath::from_points(builder.points(), builder.style());
            }
        }
        let visible = self.view.visible_doc_rect();
        let range = self.tiles.range(visible);
        // Outlines are built once they show: a big page opens without outlining all of it.
        let shown = self
            .tiles
            .tile_doc_rect(range.0, range.1)
            .union(self.tiles.tile_doc_rect(range.2, range.3));
        let moving = self.transforming();
        for stroke in self.doc.strokes() {
            let outline = self
                .paths
                .entry(stroke.id)
                .or_insert_with(|| Outline::Pending(Outline::reach(stroke)));
            if let Outline::Pending(reach) = *outline {
                if ductus_render::overlaps(reach, shown)
                    || (moving && self.selection.contains(&stroke.id))
                {
                    *outline = Outline::Built(StrokePath::new(stroke));
                }
            }
        }

        let mut out = Vec::new();
        for y in range.1..=range.3 {
            for x in range.0..=range.2 {
                out.push(self.ensure_tile(TileKey {
                    layer: TileLayer::Base,
                    x,
                    y,
                }));
            }
        }
        self.tiles
            .evict_outside(TileLayer::Base, range, TILE_MARGIN);
        // Otherwise every PDF page scrolled past would stay in memory.
        self.backgrounds
            .retain(|&(x, y), _| tiles::within(range, TILE_MARGIN, x, y));

        if let Some(bounds) = self.overlay_bounds() {
            let visible = visible - self.overlay_offset();
            let range = self.tiles.range(visible);
            self.tiles
                .evict_outside(TileLayer::Overlay, range, TILE_MARGIN);
            let area = bounds.intersect(visible);
            if ductus_render::overlaps(bounds, visible) {
                let r = self.tiles.range(area);
                for y in r.1..=r.3 {
                    for x in r.0..=r.2 {
                        out.push(self.ensure_tile(TileKey {
                            layer: TileLayer::Overlay,
                            x,
                            y,
                        }));
                    }
                }
            }
        }
        out
    }

    fn ensure_tile(&mut self, key: TileKey) -> TileView {
        let doc_rect = self.tiles.tile_doc_rect(key.x, key.y);
        let needs_render = self.tiles.tiles.get(&key).is_none_or(|t| t.dirty);
        if needs_render {
            let generation = self.tiles.next_generation();
            let mut pixmap = match self.tiles.tiles.remove(&key) {
                Some(tile) => tile.pixmap,
                None => Pixmap::new(TILE_SIZE, TILE_SIZE).expect("tile size is non-zero"),
            };
            let vp = Viewport {
                origin: doc_rect.origin(),
                scale: self.tiles.scale,
            };
            match key.layer {
                TileLayer::Base => {
                    self.paint_background(key.x, key.y, &mut pixmap, &vp);
                    self.render_pictures_into(&mut pixmap, &vp);
                    self.render_maths_into(&mut pixmap, &vp);
                    self.render_strokes_into(&mut pixmap, &vp);
                }
                TileLayer::Overlay => self.render_overlay(&mut pixmap, &vp),
            }
            self.tiles.tiles.insert(
                key,
                Tile {
                    pixmap,
                    generation,
                    dirty: false,
                },
            );
        }
        let shown = match key.layer {
            TileLayer::Base => doc_rect,
            TileLayer::Overlay => doc_rect + self.overlay_offset(),
        };
        TileView {
            key,
            generation: self.tiles.tiles[&key].generation,
            rect: self.display_view().to_screen_rect(shown),
        }
    }

    /// Whole device pixels, so shifted tiles stay sharp.
    fn overlay_offset(&self) -> Vec2 {
        let Some(shift) = self.overlay_shift() else {
            return Vec2::ZERO;
        };
        let px = self.target_scale();
        Vec2::new((shift.x * px).round() / px, (shift.y * px).round() / px)
    }

    /// The offset snaps to whole device pixels so settled tiles show 1:1, without blur.
    fn display_view(&self) -> View {
        let mut view = self.view;
        if !self.needs_settle() {
            let dpr = view.dpr;
            view.offset = Vec2::new(
                (view.offset.x * dpr).round() / dpr,
                (view.offset.y * dpr).round() / dpr,
            );
        }
        view
    }

    fn paint_background(&mut self, x: i32, y: i32, pixmap: &mut Pixmap, vp: &Viewport) {
        let Some(painter) = self.painter.as_deref() else {
            ductus_render::render_page_background(pixmap, vp, &self.layout, self.desk, None);
            return;
        };
        let background = self.backgrounds.entry((x, y)).or_insert_with(|| {
            let mut bg = Pixmap::new(TILE_SIZE, TILE_SIZE).expect("tile size is non-zero");
            ductus_render::render_page_background(
                &mut bg,
                vp,
                &self.layout,
                self.desk,
                Some(painter),
            );
            bg
        });
        pixmap.data_mut().copy_from_slice(background.data());
    }

    fn render_strokes_into(&self, pixmap: &mut Pixmap, vp: &Viewport) {
        let clip = ductus_render::ink_clip(pixmap, vp, &self.layout);
        for layer in [Layer::Highlight, Layer::Ink] {
            let strokes = self
                .doc
                .strokes()
                .iter()
                .filter(|s| !self.is_hidden(s.id))
                .filter_map(|s| self.paths.get(&s.id)?.built())
                .filter(|p| p.layer == layer);
            ductus_render::render_strokes(pixmap, vp, strokes, clip.as_ref());
        }
    }

    pub fn tile_pixels(&self, key: TileKey) -> Option<&[u8]> {
        self.tiles.tiles.get(&key).map(|t| t.pixmap.data())
    }

    pub fn snapshot(&mut self) -> Option<Pixmap> {
        let dpr = self.view.dpr;
        let mut target = Pixmap::new(
            (self.view.width * dpr).round() as u32,
            (self.view.height * dpr).round() as u32,
        )?;
        for tile in self.frame() {
            let Some(src) = self.tiles.tiles.get(&tile.key) else {
                continue;
            };
            let scale = (tile.rect.width() * dpr / f64::from(TILE_SIZE)) as f32;
            let transform = Transform::from_row(
                scale,
                0.0,
                0.0,
                scale,
                (tile.rect.x0 * dpr) as f32,
                (tile.rect.y0 * dpr) as f32,
            );
            target.draw_pixmap(
                0,
                0,
                src.pixmap.as_ref(),
                &PixmapPaint::default(),
                transform,
                None,
            );
        }
        Some(target)
    }
}

/// A stroke's outline, or until it is first shown, where it could reach.
enum Outline {
    Pending(Rect),
    Built(Option<StrokePath>),
}

impl Outline {
    /// Dotted lines draw their dots a little wider than the pen.
    fn reach(stroke: &Stroke) -> Rect {
        let extra = stroke.style.width * 0.1;
        ductus_ink::bounds(stroke).inflate(extra, extra)
    }

    fn bounds(&self) -> Rect {
        match self {
            Outline::Pending(reach) => *reach,
            Outline::Built(path) => path.as_ref().map_or(Rect::ZERO, |p| p.bounds),
        }
    }

    fn built(&self) -> Option<&StrokePath> {
        match self {
            Outline::Built(path) => path.as_ref(),
            Outline::Pending(_) => None,
        }
    }
}

fn points_bounds(points: &[ductus_core::InkPoint]) -> Option<Rect> {
    let first = points.first()?;
    Some(points.iter().fold(
        Rect::from_points(Point::new(first.x, first.y), Point::new(first.x, first.y)),
        |r, p| r.union_pt(Point::new(p.x, p.y)),
    ))
}

fn tail_bounds(points: &[ductus_core::InkPoint], style: &StrokeStyle) -> Option<Rect> {
    let r = points_bounds(&points[points.len().saturating_sub(LIVE_TAIL)..])?;
    let pad = style.width / 2.0 + 1.0;
    Some(r.inflate(pad, pad))
}

mod convert;
mod focus;
mod math;
mod selection;
mod texts;

pub use texts::TextView;

#[cfg(test)]
mod tests;
