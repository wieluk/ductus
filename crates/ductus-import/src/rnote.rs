//! Rnote (.rnote): gzipped JSON of the engine's state, in pixels at the document's DPI.

use crate::sheets::{Sheet, assemble};
use crate::{Entry, Import, ImportedPage, picture, plain_markdown};
use base64::Engine as _;
use ductus_core::{
    Document, Image, InkPoint, LinePattern, PageBackground, PageKind, PageSettings, PatternStyle,
    Rgba, Stroke, StrokeId, StrokeStyle, TextBox, Tool,
};
use ductus_format::Error;
use serde_json::Value;
use std::f64::consts::TAU;
use std::io::Read as _;
use std::path::Path;

/// Ductus draws the thinnest part of a pressure stroke at this share of its width.
const MIN_WIDTH_RATIO: f64 = 0.25;
/// Room left around the notes of an infinite canvas.
const MARGIN: f64 = 40.0;
/// Vector images (such as PDF pages) become pictures with this many pixels per unit.
const RASTER_SCALE: f64 = 2.0;

type Affine = [f64; 6];

fn not_rnote<E>(_: E) -> Error {
    Error::Invalid("This isn't an Rnote file.".into())
}

pub(crate) fn read(path: &Path) -> Result<Import, Error> {
    let raw = std::fs::read(path)?;
    // Usually gzipped; some sync setups keep it unpacked for diffing.
    let root: Value = if raw.starts_with(&[0x1f, 0x8b]) {
        let mut json = Vec::new();
        flate2::read::MultiGzDecoder::new(&raw[..])
            .read_to_end(&mut json)
            .map_err(not_rnote)?;
        serde_json::from_slice(&json).map_err(not_rnote)?
    } else {
        serde_json::from_slice(&raw).map_err(not_rnote)?
    };
    let snapshot = root
        .pointer("/data/engine_snapshot")
        .ok_or_else(|| {
            let version = root["version"].as_str().unwrap_or("unknown");
            Error::Invalid(format!(
                "This Rnote file (version {version}) is too old to import. Open and save it in a current Rnote first."
            ))
        })?;
    let name = path
        .file_stem()
        .map_or_else(|| "Imported".into(), |s| s.to_string_lossy().into_owned());
    convert(snapshot, name)
}

enum Item {
    Stroke(Stroke),
    Image(Image),
    Text(TextBox),
}

impl Item {
    fn top(&self) -> f64 {
        match self {
            Item::Stroke(s) => s.points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
            Item::Image(i) => i.y,
            Item::Text(t) => t.y,
        }
    }

    fn left(&self) -> f64 {
        match self {
            Item::Stroke(s) => s.points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
            Item::Image(i) => i.x,
            Item::Text(t) => t.x,
        }
    }

    fn shift(&mut self, dx: f64, dy: f64) {
        match self {
            Item::Stroke(s) => {
                for p in &mut s.points {
                    p.x += dx;
                    p.y += dy;
                }
            }
            Item::Image(i) => (i.x, i.y) = (i.x + dx, i.y + dy),
            Item::Text(t) => (t.x, t.y) = (t.x + dx, t.y + dy),
        }
    }
}

struct Converter {
    /// Ductus units per Rnote pixel.
    k: f64,
    pictures: Vec<(String, Vec<u8>)>,
    skipped: usize,
}

fn convert(snapshot: &Value, name: String) -> Result<Import, Error> {
    let config = &snapshot["document"]["config"];
    let format = &config["format"];
    let dpi = num(&format["dpi"]).filter(|d| *d > 0.0).unwrap_or(96.0);
    let mut c = Converter {
        k: 96.0 / dpi,
        pictures: Vec::new(),
        skipped: 0,
    };
    let background = background(&config["background"], c.k);

    let order = chrono_order(&snapshot["chrono_components"]);
    let mut items: Vec<(u64, Item)> = Vec::new();
    let components = snapshot["stroke_components"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    for (i, component) in components.iter().enumerate() {
        let Some(value) = component.get("value").filter(|v| !v.is_null()) else {
            continue;
        };
        let t = order.get(i).copied().flatten().unwrap_or(i as u64);
        for item in c.stroke(value) {
            items.push((t, item));
        }
    }
    items.sort_by_key(|(t, _)| *t);
    let items: Vec<Item> = items.into_iter().map(|(_, item)| item).collect();

    let mut warnings = Vec::new();
    if c.skipped > 0 {
        warnings.push(format!(
            "{} element(s) Ductus can't show yet were left out.",
            c.skipped
        ));
    }
    let layout = config
        .get("layout")
        .or_else(|| config.get("expand_mode"))
        .and_then(Value::as_str)
        .unwrap_or("infinite");
    let page = match layout {
        "fixed_size" | "continuous_vertical" | "endless_vertical" => {
            let width = num(&format["width"]).unwrap_or(1123.0) * c.k;
            let height = num(&format["height"]).unwrap_or(1587.0) * c.k;
            let doc_height = num(&snapshot["document"]["height"]).unwrap_or(height / c.k) * c.k;
            paged(
                name,
                items,
                (width, height),
                doc_height,
                background,
                c.pictures,
            )?
        }
        _ => endless(name, items, background, c.pictures),
    };
    Ok(Import {
        entry: Entry::Page(page),
        warnings,
    })
}

/// Each item goes onto the page it starts on.
fn paged(
    name: String,
    items: Vec<Item>,
    (width, height): (f64, f64),
    doc_height: f64,
    background: PageBackground,
    pictures: Vec<(String, Vec<u8>)>,
) -> Result<ImportedPage, Error> {
    let page_of = |y: f64| (y / height).floor().max(0.0) as usize;
    let count = items
        .iter()
        .map(|item| page_of(item.top()) + 1)
        .chain([((doc_height / height).round() as usize).max(1)])
        .max()
        .unwrap_or(1);
    let mut sheets: Vec<Sheet> = (0..count)
        .map(|_| Sheet {
            width,
            height,
            background: Some(background),
            ..Sheet::default()
        })
        .collect();
    for mut item in items {
        let index = page_of(item.top()).min(count - 1);
        item.shift(0.0, -(index as f64) * height);
        let sheet = &mut sheets[index];
        match item {
            Item::Stroke(s) => sheet.strokes.push(s),
            Item::Image(i) => sheet.images.push(i),
            Item::Text(t) => sheet.texts.push(t),
        }
    }
    assemble(name, sheets, &[], pictures)
}

/// Moved so nothing lies left of or above the page's corner.
fn endless(
    name: String,
    items: Vec<Item>,
    background: PageBackground,
    pictures: Vec<(String, Vec<u8>)>,
) -> ImportedPage {
    let left = items.iter().map(Item::left).fold(f64::INFINITY, f64::min);
    let top = items.iter().map(Item::top).fold(f64::INFINITY, f64::min);
    let dx = if left.is_finite() {
        (MARGIN - left).max(0.0)
    } else {
        0.0
    };
    let dy = if top.is_finite() {
        (MARGIN - top).max(0.0)
    } else {
        0.0
    };
    let (mut strokes, mut images, mut texts) = (Vec::new(), Vec::new(), Vec::new());
    for mut item in items {
        item.shift(dx, dy);
        match item {
            Item::Stroke(s) => strokes.push(s),
            Item::Image(i) => images.push(i),
            Item::Text(t) => texts.push(t),
        }
    }
    let settings = PageSettings {
        kind: PageKind::Endless,
        background,
    };
    ImportedPage {
        name,
        document: Document::with_contents(settings, strokes)
            .with_images(images)
            .with_texts(texts),
        pdf: None,
        pictures,
    }
}

/// The time each stroke was drawn, by its slot.
fn chrono_order(chrono: &Value) -> Vec<Option<u64>> {
    chrono.as_array().map_or_else(Vec::new, |slots| {
        slots
            .iter()
            .map(|slot| slot.pointer("/value/t").and_then(Value::as_u64))
            .collect()
    })
}

fn background(bg: &Value, k: f64) -> PageBackground {
    let defaults = PageBackground::default();
    let pattern = match bg["pattern"].as_str() {
        Some("lines") => PatternStyle::Lined,
        Some("grid" | "isometric_grid") => PatternStyle::Grid,
        Some("dots" | "isometric_dots") => PatternStyle::Dotted,
        Some(_) => PatternStyle::Blank,
        None => defaults.pattern,
    };
    let spacing = vec2(&bg["pattern_size"]).map_or(pattern.default_spacing(), |(_, h)| h * k);
    PageBackground {
        pattern,
        spacing,
        paper: color(&bg["color"]).map_or(defaults.paper, |c| Rgba { a: 1.0, ..c }),
        ink: color(&bg["pattern_color"]).map_or(defaults.ink, |c| Rgba { a: 1.0, ..c }),
    }
}

impl Converter {
    fn stroke(&mut self, value: &Value) -> Vec<Item> {
        let items = if let Some(v) = value.get("brushstroke") {
            self.brush(v).map(Item::Stroke).into_iter().collect()
        } else if let Some(v) = value.get("shapestroke") {
            self.shape(v).into_iter().map(Item::Stroke).collect()
        } else if let Some(v) = value.get("textstroke") {
            self.text(v).map(Item::Text).into_iter().collect()
        } else if let Some(v) = value.get("bitmapimage") {
            self.bitmap(v).map(Item::Image).into_iter().collect()
        } else if let Some(v) = value.get("vectorimage") {
            self.vector(v).map(Item::Image).into_iter().collect()
        } else {
            Vec::new()
        };
        if items.is_empty() {
            self.skipped += 1;
        }
        items
    }

    fn brush(&self, v: &Value) -> Option<Stroke> {
        let path = &v["path"];
        let mut points = vec![element(&path["start"])?];
        for segment in path["segments"].as_array()? {
            let from = *points.last()?;
            if let Some(line) = segment.get("lineto") {
                points.push(element(&line["end"])?);
            } else if let Some(quad) = segment.get("quadbezto") {
                let cp = vec2(&quad["cp"])?;
                let end = element(&quad["end"])?;
                points.extend(sample(8, |t| {
                    let u = 1.0 - t;
                    let at = |a: f64, b: f64, c: f64| u * u * a + 2.0 * u * t * b + t * t * c;
                    (
                        at(from.0, cp.0, end.0),
                        at(from.1, cp.1, end.1),
                        from.2 + (end.2 - from.2) * t,
                    )
                }));
            } else if let Some(cubic) = segment.get("cubbezto") {
                let (cp1, cp2) = (vec2(&cubic["cp1"])?, vec2(&cubic["cp2"])?);
                let end = element(&cubic["end"])?;
                points.extend(sample(8, |t| {
                    let u = 1.0 - t;
                    let at = |a: f64, b: f64, c: f64, d: f64| {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                    };
                    (
                        at(from.0, cp1.0, cp2.0, end.0),
                        at(from.1, cp1.1, cp2.1, end.1),
                        from.2 + (end.2 - from.2) * t,
                    )
                }));
            }
        }
        let (kind, options) = style(&v["style"])?;
        let curve = match kind {
            "rough" => "const",
            "textured" => "linear",
            _ => options["pressure_curve"].as_str().unwrap_or("linear"),
        };
        let ratio = |p: f64| match curve {
            "const" => 1.0,
            "sqrt" => p.sqrt(),
            "cbrt" => p.cbrt(),
            "pow2" => p.powi(2),
            "pow3" => p.powi(3),
            _ => p,
        };
        let mut style = self.line_style(kind, options)?;
        if style.tool == Tool::Highlighter {
            return Some(Stroke {
                id: StrokeId(0),
                style,
                points: self.points(points.iter().map(|&(x, y, _)| (x, y, 1.0))),
            });
        }
        style.pattern = match options["line_style"].as_str() {
            Some("dotted") => LinePattern::Dotted,
            Some(s) if s.starts_with("dashed") => LinePattern::Dashed,
            _ => LinePattern::Solid,
        };
        let points = points.iter().map(|&(x, y, p)| {
            let r = ratio(p.clamp(0.0, 1.0));
            (
                x,
                y,
                ((r - MIN_WIDTH_RATIO) / (1.0 - MIN_WIDTH_RATIO)).clamp(0.0, 1.0),
            )
        });
        Some(Stroke {
            id: StrokeId(0),
            style,
            points: self.points(points),
        })
    }

    /// Without a stroke color (only filled) there is nothing to draw as ink.
    fn line_style(&self, kind: &str, options: &Value) -> Option<StrokeStyle> {
        let mut color = color(&options["stroke_color"])?;
        let width = num(&options["stroke_width"]).unwrap_or(2.0) * self.k;
        // See-through ink is how Rnote highlights.
        let tool = if color.a < 0.95 && kind != "textured" {
            color.a = 1.0;
            Tool::Highlighter
        } else {
            Tool::Pen
        };
        Some(StrokeStyle {
            tool,
            color,
            width,
            pattern: LinePattern::Solid,
        })
    }

    fn points(&self, points: impl Iterator<Item = (f64, f64, f64)>) -> Vec<InkPoint> {
        let points = points
            .map(|(x, y, p)| InkPoint {
                x: x * self.k,
                y: y * self.k,
                pressure: p as f32,
                tilt_x: 0.0,
                tilt_y: 0.0,
                t: 0,
            })
            .collect::<Vec<_>>();
        crate::densify(points)
    }

    fn shape(&self, v: &Value) -> Vec<Stroke> {
        let Some((kind, options)) = style(&v["style"]) else {
            return Vec::new();
        };
        let Some(style) = self.line_style(kind, options) else {
            return Vec::new();
        };
        outlines(&v["shape"])
            .into_iter()
            .filter(|line| !line.is_empty())
            .map(|line| Stroke {
                id: StrokeId(0),
                style,
                points: self.points(line.into_iter().map(|(x, y)| (x, y, 1.0))),
            })
            .collect()
    }

    fn text(&self, v: &Value) -> Option<TextBox> {
        let text = v["text"].as_str()?;
        if text.trim().is_empty() {
            return None;
        }
        let at = affine(v).map_or((0.0, 0.0), |a| (a[4], a[5]));
        let text_style = &v["text_style"];
        let size = num(&text_style["font_size"]).unwrap_or(32.0);
        let lines: Vec<&str> = text.lines().collect();
        let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let width =
            num(&text_style["max_width"]).unwrap_or(longest.max(4) as f64 * size * 0.6) + 8.0;
        Some(TextBox {
            id: StrokeId(0),
            markdown: plain_markdown(text),
            x: at.0 * self.k,
            y: at.1 * self.k,
            width: width * self.k,
            height: lines.len().max(1) as f64 * size * 1.35 * self.k + 8.0,
            scale: size * self.k / TextBox::FONT_SIZE,
        })
    }

    fn bitmap(&mut self, v: &Value) -> Option<Image> {
        let image = &v["image"];
        let (w, h) = (
            image["pixel_width"].as_u64()? as u32,
            image["pixel_height"].as_u64()? as u32,
        );
        let mut data = base64::engine::general_purpose::STANDARD
            .decode(image["data"].as_str()?)
            .ok()?;
        // Rnote keeps premultiplied RGBA.
        for px in data.chunks_exact_mut(4) {
            let a = u32::from(px[3]);
            if a > 0 && a < 255 {
                for c in &mut px[..3] {
                    *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        let rgba = image::RgbaImage::from_raw(w, h, data)?;
        let mut png = std::io::Cursor::new(Vec::new());
        rgba.write_to(&mut png, image::ImageFormat::Png).ok()?;
        self.place(&png.into_inner(), rectangle(&v["rectangle"])?)
    }

    fn vector(&mut self, v: &Value) -> Option<Image> {
        let rect = rectangle(&v["rectangle"])?;
        let tree =
            resvg::usvg::Tree::from_str(v["svg_data"].as_str()?, &resvg::usvg::Options::default())
                .ok()?;
        let size = tree.size();
        let scale = |extent: f64| (extent * self.k * RASTER_SCALE).clamp(1.0, 4096.0);
        let (w, h) = (scale(rect.2 - rect.0), scale(rect.3 - rect.1));
        let mut pixmap = resvg::tiny_skia::Pixmap::new(w as u32, h as u32)?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(
                w as f32 / size.width(),
                h as f32 / size.height(),
            ),
            &mut pixmap.as_mut(),
        );
        self.place(&pixmap.encode_png().ok()?, rect)
    }

    fn place(&mut self, bytes: &[u8], (x0, y0, x1, y1): (f64, f64, f64, f64)) -> Option<Image> {
        let (name, png) = picture(bytes)?;
        if !self.pictures.iter().any(|(n, _)| *n == name) {
            self.pictures.push((name.clone(), png));
        }
        Some(Image {
            id: StrokeId(0),
            source: name,
            x: x0 * self.k,
            y: y0 * self.k,
            width: (x1 - x0) * self.k,
            height: (y1 - y0) * self.k,
        })
    }
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn vec2(v: &Value) -> Option<(f64, f64)> {
    let a = v.as_array()?;
    Some((num(a.first()?)?, num(a.get(1)?)?))
}

fn element(v: &Value) -> Option<(f64, f64, f64)> {
    let (x, y) = vec2(&v["pos"])?;
    Some((x, y, num(&v["pressure"]).unwrap_or(0.5)))
}

fn color(v: &Value) -> Option<Rgba> {
    let channel = |k: &str| num(&v[k]).map(|c| c.clamp(0.0, 1.0) as f32);
    Some(Rgba {
        r: channel("r")?,
        g: channel("g")?,
        b: channel("b")?,
        a: channel("a").unwrap_or(1.0),
    })
}

/// `{"smooth": {...}}` as ("smooth", {...}).
fn style(v: &Value) -> Option<(&str, &Value)> {
    let (kind, options) = v.as_object()?.iter().next()?;
    Some((kind.as_str(), options))
}

/// Rnote 0.15 stores glam's six numbers; 0.13 a 3x3 nalgebra matrix by columns.
fn affine(v: &Value) -> Option<Affine> {
    let numbers = |v: &Value| -> Option<Vec<f64>> { v.as_array()?.iter().map(num).collect() };
    if let Some(a) = v.get("affine").and_then(numbers).filter(|a| a.len() == 6) {
        return a.try_into().ok();
    }
    let m = v
        .pointer("/transform/affine")
        .and_then(numbers)
        .filter(|m| m.len() == 9)?;
    Some([m[0], m[1], m[3], m[4], m[6], m[7]])
}

fn apply(a: &Affine, (x, y): (f64, f64)) -> (f64, f64) {
    (a[0] * x + a[2] * y + a[4], a[1] * x + a[3] * y + a[5])
}

fn half_extents(v: &Value) -> Option<(f64, f64)> {
    vec2(&v["cuboid"]["half_extents"])
}

fn corners(v: &Value) -> Option<Vec<(f64, f64)>> {
    let (hx, hy) = half_extents(v)?;
    let a = affine(v)?;
    Some(
        [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)]
            .into_iter()
            .map(|p| apply(&a, p))
            .collect(),
    )
}

/// The bounds of a rotated rectangle, as (x0, y0, x1, y1).
fn rectangle(v: &Value) -> Option<(f64, f64, f64, f64)> {
    let corners = corners(v)?;
    let xs = corners.iter().map(|c| c.0);
    let ys = corners.iter().map(|c| c.1);
    Some((
        xs.clone().fold(f64::INFINITY, f64::min),
        ys.clone().fold(f64::INFINITY, f64::min),
        xs.fold(f64::NEG_INFINITY, f64::max),
        ys.fold(f64::NEG_INFINITY, f64::max),
    ))
}

fn sample<T>(steps: usize, f: impl Fn(f64) -> T) -> impl Iterator<Item = T> {
    (1..=steps).map(move |i| f(i as f64 / steps as f64))
}

/// A shape as the lines that draw it.
fn outlines(shape: &Value) -> Vec<Vec<(f64, f64)>> {
    let Some((kind, v)) = style(shape) else {
        return Vec::new();
    };
    let line = || Some(vec![vec2(&v["start"])?, vec2(&v["end"])?]);
    let path = |closed: bool| {
        let start = vec2(&v["start"])?;
        let mut points = vec![start];
        points.extend(v["path"].as_array()?.iter().filter_map(vec2));
        if closed {
            points.push(start);
        }
        Some(points)
    };
    let lines = match kind {
        "line" => line().map(|l| vec![l]),
        "arrow" => (|| {
            let (start, tip) = (vec2(&v["start"])?, vec2(&v["tip"])?);
            let (dx, dy) = (start.0 - tip.0, start.1 - tip.1);
            let len = dx.hypot(dy).max(f64::EPSILON);
            let head = (len * 0.3).min(24.0);
            let side = |angle: f64| {
                let (s, c) = angle.sin_cos();
                (
                    tip.0 + (dx * c - dy * s) / len * head,
                    tip.1 + (dx * s + dy * c) / len * head,
                )
            };
            let angle = 30f64.to_radians();
            Some(vec![vec![start, tip], vec![side(angle), tip, side(-angle)]])
        })(),
        "rect" => corners(v).map(|mut c| {
            c.push(c[0]);
            vec![c]
        }),
        "ellipse" => (|| {
            let (rx, ry) = vec2(&v["radii"])?;
            let a = affine(v)?;
            let points = (0..=64)
                .map(|i| {
                    let t = TAU * f64::from(i) / 64.0;
                    apply(&a, (rx * t.cos(), ry * t.sin()))
                })
                .collect();
            Some(vec![points])
        })(),
        "quadbez" => (|| {
            let (s, cp, e) = (vec2(&v["start"])?, vec2(&v["cp"])?, vec2(&v["end"])?);
            let mut points = vec![s];
            points.extend(sample(24, |t| {
                let u = 1.0 - t;
                let at = |a: f64, b: f64, c: f64| u * u * a + 2.0 * u * t * b + t * t * c;
                (at(s.0, cp.0, e.0), at(s.1, cp.1, e.1))
            }));
            Some(vec![points])
        })(),
        "cubbez" => (|| {
            let (s, c1, c2, e) = (
                vec2(&v["start"])?,
                vec2(&v["cp1"])?,
                vec2(&v["cp2"])?,
                vec2(&v["end"])?,
            );
            let mut points = vec![s];
            points.extend(sample(32, |t| {
                let u = 1.0 - t;
                let at = |a: f64, b: f64, c: f64, d: f64| {
                    u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                };
                (at(s.0, c1.0, c2.0, e.0), at(s.1, c1.1, c2.1, e.1))
            }));
            Some(vec![points])
        })(),
        "polyline" => path(false).map(|p| vec![p]),
        "polygon" => path(true).map(|p| vec![p]),
        _ => None,
    };
    lines.unwrap_or_default()
}

#[cfg(test)]
mod tests;
