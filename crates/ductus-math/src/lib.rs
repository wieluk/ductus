//! Typst lays formulas out (no TeX installation needed); LaTeX goes through tylax first.

use ductus_core::{MathBox, Rgba, UNITS_PER_POINT};
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::layout::{Frame, FrameItem};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, TextItem};
use typst::utils::LazyHash;
use typst::visualize::{CurveItem, Geometry, Paint, Shape as TypstShape};
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    Latex,
    Typst,
}

impl Syntax {
    pub fn name(self) -> &'static str {
        match self {
            Syntax::Latex => "latex",
            Syntax::Typst => "typst",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "latex" => Some(Syntax::Latex),
            "typst" => Some(Syntax::Typst),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Formula {
    pub shapes: Vec<(BezPath, Rgba)>,
    pub width: f64,
    pub height: f64,
}

impl Formula {
    pub fn bounds(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width, self.height)
    }
}

pub fn render(source: &str, syntax: Syntax, size: f64) -> Result<Formula, String> {
    let math = match syntax {
        Syntax::Typst => source.to_owned(),
        Syntax::Latex => tylax::latex_to_typst(source),
    };
    if math.trim().is_empty() {
        return Err("The formula is empty.".into());
    }
    // A page exactly as big as the formula; display style, like $$…$$.
    let document = format!(
        "#set page(width: auto, height: auto, margin: 0pt, fill: none)\n\
         #set text(size: {size}pt)\n\
         $ {math} $\n"
    );
    let world = MathWorld::new(document);
    let compiled = typst::compile::<PagedDocument>(&world)
        .output
        .map_err(|errors| {
            errors
                .iter()
                .map(|e| e.message.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        })?;
    let page = compiled
        .pages()
        .first()
        .ok_or_else(|| "Typst produced no page.".to_string())?;
    let mut shapes = Vec::new();
    collect(&page.frame, Affine::IDENTITY, &mut shapes);
    Ok(Formula {
        shapes,
        width: page.frame.width().to_pt(),
        height: page.frame.height().to_pt(),
    })
}

type LayoutCache = HashMap<(String, String), Arc<Result<Formula, String>>>;

pub const BODY_SIZE: f64 = 12.0;

/// Cached: the same formula is drawn into many tiles and saved often.
pub fn layout(math: &MathBox) -> Arc<Result<Formula, String>> {
    static CACHE: OnceLock<Mutex<LayoutCache>> = OnceLock::new();
    let key = (math.syntax.clone(), math.source.clone());
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(found) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return found;
    }
    let syntax = Syntax::from_name(&math.syntax).unwrap_or(Syntax::Latex);
    let result = Arc::new(render(&math.source, syntax, BODY_SIZE));
    if let Ok(mut c) = cache.lock() {
        if c.len() > 512 {
            c.clear();
        }
        c.insert(key, Arc::clone(&result));
    }
    result
}

pub fn size(math: &MathBox) -> Option<(f64, f64)> {
    let formula = layout(math);
    let f = formula.as_ref().as_ref().ok()?;
    let k = UNITS_PER_POINT * math.scale;
    Some((f.width * k, f.height * k))
}

pub fn outlines(math: &MathBox) -> Vec<(BezPath, Rgba)> {
    let formula = layout(math);
    let Ok(f) = formula.as_ref() else {
        return Vec::new();
    };
    let m = Affine::translate((math.x, math.y)) * Affine::scale(UNITS_PER_POINT * math.scale);
    f.shapes
        .iter()
        .map(|(path, _)| (m * path.clone(), math.color))
        .collect()
}

fn color(paint: &Paint) -> Rgba {
    match paint {
        Paint::Solid(c) => {
            let [r, g, b, a] = c.to_vec4();
            Rgba { r, g, b, a }
        }
        // Gradients and tilings don't occur in plain formulas.
        _ => Rgba::BLACK,
    }
}

fn pt(p: typst::layout::Point) -> Point {
    Point::new(p.x.to_pt(), p.y.to_pt())
}

fn collect(frame: &Frame, at: Affine, out: &mut Vec<(BezPath, Rgba)>) {
    for (pos, item) in frame.items() {
        let here = at * Affine::translate(pt(*pos).to_vec2());
        match item {
            FrameItem::Group(group) => {
                let t = group.transform;
                let m = Affine::new([
                    t.sx.get(),
                    t.ky.get(),
                    t.kx.get(),
                    t.sy.get(),
                    t.tx.to_pt(),
                    t.ty.to_pt(),
                ]);
                collect(&group.frame, here * m, out);
            }
            FrameItem::Text(text) => text_outlines(text, here, out),
            FrameItem::Shape(shape, _) => shape_outlines(shape, here, out),
            _ => {}
        }
    }
}

struct Outline(BezPath);

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((f64::from(x), f64::from(y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((f64::from(x), f64::from(y)));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0
            .quad_to((f64::from(x1), f64::from(y1)), (f64::from(x), f64::from(y)));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.curve_to(
            (f64::from(x1), f64::from(y1)),
            (f64::from(x2), f64::from(y2)),
            (f64::from(x), f64::from(y)),
        );
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}

fn text_outlines(text: &TextItem, at: Affine, out: &mut Vec<(BezPath, Rgba)>) {
    let size = text.size.to_pt();
    let face = text.font.ttf();
    let scale = size / text.font.units_per_em();
    let fill = color(&text.fill);
    let mut x = 0.0;
    for glyph in &text.glyphs {
        let dx = x + glyph.x_offset.at(text.size).to_pt();
        let dy = -glyph.y_offset.at(text.size).to_pt();
        let mut outline = Outline(BezPath::new());
        let id = ttf_parser::GlyphId(glyph.id);
        if face.outline_glyph(id, &mut outline).is_some() {
            // Font units are y-up.
            let m = at * Affine::translate((dx, dy)) * Affine::scale_non_uniform(scale, -scale);
            out.push((m * outline.0, fill));
        }
        x += glyph.x_advance.at(text.size).to_pt();
    }
}

fn shape_outlines(shape: &TypstShape, at: Affine, out: &mut Vec<(BezPath, Rgba)>) {
    let path = match &shape.geometry {
        Geometry::Line(to) => {
            let mut p = BezPath::new();
            p.move_to(Point::ZERO);
            p.line_to(pt(*to));
            p
        }
        Geometry::Rect(size) => Rect::new(0.0, 0.0, size.x.to_pt(), size.y.to_pt()).to_path(0.1),
        Geometry::Curve(curve) => {
            let mut p = BezPath::new();
            for item in &curve.0 {
                match item {
                    CurveItem::Move(a) => p.move_to(pt(*a)),
                    CurveItem::Line(a) => p.line_to(pt(*a)),
                    CurveItem::Cubic(a, b, c) => p.curve_to(pt(*a), pt(*b), pt(*c)),
                    CurveItem::Close => p.close_path(),
                }
            }
            p
        }
    };
    if let Some(fill) = &shape.fill {
        out.push((at * path.clone(), color(fill)));
    }
    if let Some(stroke) = &shape.stroke {
        // Strokes (fraction bars, roots) become filled outlines too.
        let style = kurbo::Stroke::new(stroke.thickness.to_pt());
        let outline = kurbo::stroke(path, &style, &kurbo::StrokeOpts::default(), 0.01);
        out.push((at * outline, color(&stroke.paint)));
    }
}

struct Fonts {
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .collect();
        Fonts {
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
        }
    })
}

fn library() -> &'static LazyHash<Library> {
    static LIBRARY: OnceLock<LazyHash<Library>> = OnceLock::new();
    LIBRARY.get_or_init(|| LazyHash::new(Library::default()))
}

struct MathWorld {
    id: FileId,
    source: Source,
}

impl MathWorld {
    fn new(text: String) -> Self {
        let path = VirtualPath::new("main.typ").expect("a valid path");
        let id = RootedPath::new(VirtualRoot::Project, path).intern();
        Self {
            id,
            source: Source::new(id, text),
        }
    }
}

impl World for MathWorld {
    fn library(&self) -> &LazyHash<Library> {
        library()
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &fonts().book
    }

    fn main(&self) -> FileId {
        self.id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.id {
            Ok(self.source.clone())
        } else {
            Err(FileError::AccessDenied)
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.id {
            Ok(Bytes::from_string(self.source.clone()))
        } else {
            Err(FileError::AccessDenied)
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        fonts().fonts.get(index).cloned()
    }

    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mistakes_are_reported() {
        let err = render("frac(a, ", Syntax::Typst, 12.0).unwrap_err();
        assert!(!err.is_empty());
        assert!(render("   ", Syntax::Latex, 12.0).is_err());
    }
}

#[cfg(test)]
mod snippet_tests {
    use super::*;

    /// Every quick insert of the formula editor must compile in both syntaxes.
    #[test]
    fn all_editor_snippets_compile() {
        let js = include_str!("../../ductus-qt/qml/MathSnippets.qml");
        let field = |line: &str, key: &str| -> Option<String> {
            let start = line.find(&format!("\"{key}\": \""))? + key.len() + 5;
            let mut out = String::new();
            let mut chars = line[start..].chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => out.push(chars.next()?),
                    '"' => return Some(out),
                    c => out.push(c),
                }
            }
            None
        };
        let mut checked = 0;
        let mut failures = Vec::new();
        for line in js.lines().filter(|l| l.contains("\"label\"")) {
            let latex = field(line, "latex").unwrap();
            let typst = field(line, "typst").unwrap();
            let fill = |s: &str| {
                s.replace("{}", "{x}")
                    .replace("()", "(x)")
                    .replace(", )", ", x)")
            };
            for (source, syntax) in [(latex, Syntax::Latex), (typst, Syntax::Typst)] {
                let formula = format!("a {} b", fill(&source));
                if let Err(e) = render(&formula, syntax, 12.0) {
                    failures.push(format!("{syntax:?} {formula:?}: {e}"));
                }
                checked += 1;
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        assert!(checked > 200, "{checked} snippets checked");
    }
}
