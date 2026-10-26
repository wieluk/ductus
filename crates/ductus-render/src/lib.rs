use ductus_core::{
    Document, InkPoint, PageKind, PageSettings, PatternStyle, Rgba, Stroke, StrokeStyle, Tool,
};
use kurbo::{PathEl, Rect};
use tiny_skia::{
    Color, FillRule, FilterQuality, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Shader,
    Transform,
};

/// Highlights go beneath ink so they never dull it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Highlight,
    Ink,
}

#[derive(Debug, Clone)]
pub struct StrokePath {
    pub bounds: Rect,
    pub layer: Layer,
    path: Path,
    color: Color,
}

impl StrokePath {
    pub fn new(stroke: &Stroke) -> Option<Self> {
        Self::from_points(&stroke.points, &stroke.style)
    }

    pub fn from_points(points: &[InkPoint], style: &StrokeStyle) -> Option<Self> {
        let outline = ductus_ink::outline(points, style);
        let path = to_skia_path(&outline)?;
        Some(Self {
            bounds: kurbo::Shape::bounding_box(&outline),
            layer: match style.tool {
                Tool::Pen => Layer::Ink,
                Tool::Highlighter => Layer::Highlight,
            },
            path,
            color: skia_color(style.color),
        })
    }
}

impl StrokePath {
    pub fn fill(path: &kurbo::BezPath, color: Rgba) -> Option<Self> {
        Some(Self {
            bounds: kurbo::Shape::bounding_box(path),
            layer: Layer::Ink,
            path: to_skia_path(path)?,
            color: skia_color(color),
        })
    }
}

fn to_skia_path(path: &kurbo::BezPath) -> Option<Path> {
    let mut b = PathBuilder::new();
    let f = |v: f64| v as f32;
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => b.move_to(f(p.x), f(p.y)),
            PathEl::LineTo(p) => b.line_to(f(p.x), f(p.y)),
            PathEl::QuadTo(c, p) => b.quad_to(f(c.x), f(c.y), f(p.x), f(p.y)),
            PathEl::CurveTo(c1, c2, p) => {
                b.cubic_to(f(c1.x), f(c1.y), f(c2.x), f(c2.y), f(p.x), f(p.y))
            }
            PathEl::ClosePath => b.close(),
        }
    }
    b.finish()
}

pub fn skia_color(c: Rgba) -> Color {
    Color::from_rgba(c.r, c.g, c.b, c.a).unwrap_or(Color::BLACK)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub origin: kurbo::Point,
    /// Device pixels per document unit (zoom × device pixel ratio).
    pub scale: f64,
}

impl Viewport {
    pub fn doc_rect(&self, width: u32, height: u32) -> Rect {
        Rect::new(
            self.origin.x,
            self.origin.y,
            self.origin.x + f64::from(width) / self.scale,
            self.origin.y + f64::from(height) / self.scale,
        )
    }

    fn transform(&self) -> Transform {
        let s = self.scale as f32;
        Transform::from_row(
            s,
            0.0,
            0.0,
            s,
            (-self.origin.x * self.scale) as f32,
            (-self.origin.y * self.scale) as f32,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageLayout {
    pub settings: PageSettings,
    pub sheets: Vec<Rect>,
}

impl PageLayout {
    pub fn for_document(doc: &Document, spare: usize) -> Self {
        Self {
            settings: *doc.settings(),
            sheets: doc
                .sheets(spare)
                .into_iter()
                .map(|(x, y, w, h)| Rect::new(x, y, x + w, y + h))
                .collect(),
        }
    }

    pub fn sheet_rects(&self) -> &[Rect] {
        &self.sheets
    }

    pub fn is_paper(&self) -> bool {
        !matches!(self.settings.kind, PageKind::Endless)
    }
}

pub trait SheetPainter {
    fn paint(&self, index: usize, sheet: Rect, pixmap: &mut Pixmap, vp: &Viewport);
}

fn solid(color: Rgba) -> Paint<'static> {
    Paint {
        shader: Shader::SolidColor(skia_color(color)),
        anti_alias: false,
        ..Paint::default()
    }
}

fn fill_px(pixmap: &mut Pixmap, x0: f64, y0: f64, x1: f64, y1: f64, paint: &Paint) {
    let (x0, y0, x1, y1) = (x0.round(), y0.round(), x1.round(), y1.round());
    if let Some(r) = tiny_skia::Rect::from_ltrb(x0 as f32, y0 as f32, x1 as f32, y1 as f32) {
        pixmap.fill_rect(r, paint, Transform::identity(), None);
    }
}

fn render_pattern(
    pixmap: &mut Pixmap,
    vp: &Viewport,
    settings: &PageSettings,
    area: Rect,
    origin: kurbo::Point,
) {
    let bg = &settings.background;
    let step = bg.spacing * vp.scale;
    if bg.pattern == PatternStyle::Blank || bg.spacing <= 0.0 || step < 6.0 {
        return; // too dense to be useful at this zoom
    }
    let visible = area.intersect(vp.doc_rect(pixmap.width(), pixmap.height()));
    if visible.is_zero_area() {
        return;
    }
    let paint = solid(bg.ink);
    let px = |x: f64| (x - vp.origin.x) * vp.scale;
    let py = |y: f64| (y - vp.origin.y) * vp.scale;
    let first = |lo: f64, o: f64| ((lo - o) / bg.spacing).floor() as i64 + 1;
    let last = |hi: f64, o: f64| ((hi - o) / bg.spacing).ceil() as i64 - 1;
    let line = (vp.scale.sqrt()).clamp(1.0, 2.0).round();

    let rows = first(visible.y0.max(area.y0), origin.y)..=last(visible.y1.min(area.y1), origin.y);
    let cols = first(visible.x0.max(area.x0), origin.x)..=last(visible.x1.min(area.x1), origin.x);
    match bg.pattern {
        PatternStyle::Blank => {}
        PatternStyle::Lined | PatternStyle::Grid => {
            for k in rows.clone() {
                let y = py(origin.y + k as f64 * bg.spacing);
                fill_px(pixmap, px(visible.x0), y, px(visible.x1), y + line, &paint);
            }
            if bg.pattern == PatternStyle::Grid {
                for k in cols {
                    let x = px(origin.x + k as f64 * bg.spacing);
                    fill_px(pixmap, x, py(visible.y0), x + line, py(visible.y1), &paint);
                }
            }
        }
        PatternStyle::Dotted => {
            let dot = (1.5 * vp.scale.sqrt()).clamp(2.0, 4.0).round();
            for j in rows {
                for k in cols.clone() {
                    let x = px(origin.x + k as f64 * bg.spacing) - dot / 2.0;
                    let y = py(origin.y + j as f64 * bg.spacing) - dot / 2.0;
                    fill_px(pixmap, x, y, x + dot, y + dot, &paint);
                }
            }
        }
    }
}

pub fn render_page_background(
    pixmap: &mut Pixmap,
    vp: &Viewport,
    layout: &PageLayout,
    desk: Rgba,
    painter: Option<&dyn SheetPainter>,
) {
    let settings = &layout.settings;
    if !layout.is_paper() {
        pixmap.fill(skia_color(settings.background.paper));
        let area = vp.doc_rect(pixmap.width(), pixmap.height());
        render_pattern(
            pixmap,
            vp,
            settings,
            area.inflate(1.0, 1.0),
            kurbo::Point::ZERO,
        );
        return;
    }
    pixmap.fill(skia_color(desk));
    let view = vp.doc_rect(pixmap.width(), pixmap.height());
    let edge = solid(Rgba::rgb(
        (desk.r * 0.85).min(1.0),
        (desk.g * 0.85).min(1.0),
        (desk.b * 0.86).min(1.0),
    ));
    let paper = solid(settings.background.paper);
    for (index, sheet) in layout.sheet_rects().iter().copied().enumerate() {
        if !overlaps(sheet, view) {
            continue;
        }
        let (x0, y0) = (
            (sheet.x0 - vp.origin.x) * vp.scale,
            (sheet.y0 - vp.origin.y) * vp.scale,
        );
        let (x1, y1) = (
            (sheet.x1 - vp.origin.x) * vp.scale,
            (sheet.y1 - vp.origin.y) * vp.scale,
        );
        // A one pixel edge around each sheet separates it from the desk.
        fill_px(pixmap, x0 - 1.0, y0 - 1.0, x1 + 1.0, y1 + 1.0, &edge);
        fill_px(pixmap, x0, y0, x1, y1, &paper);
        match painter {
            Some(painter) => painter.paint(index, sheet, pixmap, vp),
            None => match settings.ruled_margins() {
                Some((top, left)) => {
                    // Lines start below the heading room; the margin line runs the full height.
                    let ruled = Rect::new(sheet.x0, sheet.y0 + top - 1.0, sheet.x1, sheet.y1);
                    let origin = kurbo::Point::new(sheet.x0, sheet.y0 + top);
                    render_pattern(pixmap, vp, settings, ruled, origin);
                    let x = (sheet.x0 + left - vp.origin.x) * vp.scale;
                    let line = (vp.scale.sqrt()).clamp(1.0, 2.0).round();
                    fill_px(
                        pixmap,
                        x,
                        y0,
                        x + line,
                        y1,
                        &solid(ductus_core::MARGIN_LINE),
                    );
                }
                None => render_pattern(pixmap, vp, settings, sheet, sheet.origin()),
            },
        }
    }
}

pub fn ink_clip(pixmap: &Pixmap, vp: &Viewport, layout: &PageLayout) -> Option<Mask> {
    if !layout.is_paper() {
        return None;
    }
    let mut mask = Mask::new(pixmap.width(), pixmap.height())?;
    let view = vp.doc_rect(pixmap.width(), pixmap.height());
    for sheet in layout.sheet_rects().iter().filter(|s| overlaps(**s, view)) {
        let r = tiny_skia::Rect::from_ltrb(
            sheet.x0 as f32,
            sheet.y0 as f32,
            sheet.x1 as f32,
            sheet.y1 as f32,
        )?;
        mask.fill_path(
            &PathBuilder::from_rect(r),
            FillRule::Winding,
            false,
            vp.transform(),
        );
    }
    Some(mask)
}

pub fn render_strokes<'a>(
    pixmap: &mut Pixmap,
    vp: &Viewport,
    strokes: impl IntoIterator<Item = &'a StrokePath>,
    clip: Option<&Mask>,
) {
    render_strokes_moved(pixmap, vp, strokes, kurbo::Affine::IDENTITY, clip);
}

/// Draws outlines moved by `t` (a selection being dragged) without building new ones.
pub fn render_strokes_moved<'a>(
    pixmap: &mut Pixmap,
    vp: &Viewport,
    strokes: impl IntoIterator<Item = &'a StrokePath>,
    t: kurbo::Affine,
    clip: Option<&Mask>,
) {
    let area = vp.doc_rect(pixmap.width(), pixmap.height());
    let [a, b, c, d, e, f] = t.as_coeffs().map(|v| v as f32);
    let transform = vp
        .transform()
        .pre_concat(Transform::from_row(a, b, c, d, e, f));
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    for stroke in strokes {
        if !overlaps(t.transform_rect_bbox(stroke.bounds), area) {
            continue;
        }
        paint.shader = Shader::SolidColor(stroke.color);
        pixmap.fill_path(&stroke.path, &paint, FillRule::Winding, transform, clip);
    }
}

pub fn render_guide(
    pixmap: &mut Pixmap,
    vp: &Viewport,
    points: &[kurbo::Point],
    closed: bool,
    color: Rgba,
) {
    if points.len() < 2 {
        return;
    }
    let mut b = PathBuilder::new();
    let to_px = |p: &kurbo::Point| {
        (
            ((p.x - vp.origin.x) * vp.scale) as f32,
            ((p.y - vp.origin.y) * vp.scale) as f32,
        )
    };
    let (x, y) = to_px(&points[0]);
    b.move_to(x, y);
    for p in &points[1..] {
        let (x, y) = to_px(p);
        b.line_to(x, y);
    }
    if closed {
        b.close();
    }
    let Some(path) = b.finish() else { return };
    let stroke = tiny_skia::Stroke {
        width: 2.0,
        dash: tiny_skia::StrokeDash::new(vec![6.0, 4.0], 0.0),
        ..tiny_skia::Stroke::default()
    };
    let paint = Paint {
        shader: Shader::SolidColor(skia_color(color)),
        anti_alias: true,
        ..Paint::default()
    };
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

/// Smaller copies are averaged so pictures drawn small don't shimmer.
pub struct Picture {
    levels: Vec<Pixmap>,
}

impl Picture {
    pub fn decode_png(bytes: &[u8]) -> Option<Self> {
        Pixmap::decode_png(bytes).ok().map(Self::from_pixmap)
    }

    pub fn from_pixmap(full: Pixmap) -> Self {
        let mut levels = vec![full];
        loop {
            let last = levels.last().expect("starts with one level");
            let (w, h) = (last.width() / 2, last.height() / 2);
            if w < 64 || h < 64 {
                break;
            }
            let Some(mut half) = Pixmap::new(w, h) else {
                break;
            };
            half.draw_pixmap(
                0,
                0,
                last.as_ref(),
                &PixmapPaint {
                    quality: FilterQuality::Bilinear,
                    ..PixmapPaint::default()
                },
                Transform::from_scale(0.5, 0.5),
                None,
            );
            levels.push(half);
        }
        Self { levels }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.levels[0].width(), self.levels[0].height())
    }
}

pub fn render_picture(
    pixmap: &mut Pixmap,
    vp: &Viewport,
    picture: &Picture,
    rect: Rect,
    clip: Option<&Mask>,
) {
    let area = vp.doc_rect(pixmap.width(), pixmap.height());
    if !overlaps(rect, area) || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    // The smallest copy that still has at least one pixel per device pixel.
    let target = rect.width() * vp.scale;
    let level = picture
        .levels
        .iter()
        .rev()
        .find(|l| f64::from(l.width()) >= target)
        .unwrap_or(&picture.levels[0]);
    let transform = vp
        .transform()
        .pre_translate(rect.x0 as f32, rect.y0 as f32)
        .pre_scale(
            (rect.width() / f64::from(level.width())) as f32,
            (rect.height() / f64::from(level.height())) as f32,
        );
    pixmap.draw_pixmap(
        0,
        0,
        level.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..PixmapPaint::default()
        },
        transform,
        clip,
    );
}

pub fn render_handle(pixmap: &mut Pixmap, vp: &Viewport, at: kurbo::Point, color: Rgba) {
    let (x, y) = (
        ((at.x - vp.origin.x) * vp.scale) as f32,
        ((at.y - vp.origin.y) * vp.scale) as f32,
    );
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    for (radius, fill) in [(7.0, skia_color(color)), (5.0, Color::WHITE)] {
        let Some(circle) = PathBuilder::from_circle(x, y, radius) else {
            return;
        };
        paint.shader = Shader::SolidColor(fill);
        pixmap.fill_path(
            &circle,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

/// Unlike Rect::intersect().area(), this keeps zero-height bounds (flat lines).
pub fn overlaps(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 && a.x1 >= b.x0 && a.y0 <= b.y1 && a.y1 >= b.y0
}

#[cfg(test)]
mod tests;
