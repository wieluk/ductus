//! Standard /Ink annotations with an appearance stream, so every reader shows the ink; saves append one incremental update and keep the original bytes.

use crate::Error;
use ductus_core::{
    InkPoint, LinePattern, Rgba, Stroke, StrokeId, StrokeStyle, Tool, UNITS_PER_POINT,
};
use kurbo::{BezPath, PathEl, Point, Shape};
use lopdf::{Dictionary, Document, IncrementalDocument, Object, ObjectId, Stream, StringFormat};
use std::fmt::Write as _;

const KEY_INK: &str = "DuctusInk";
const KEY_TILT: &str = "DuctusTilt";
const KEY_TOOL: &str = "DuctusTool";
const KEY_WIDTH: &str = "DuctusWidth";
const KEY_LINE: &str = "DuctusLine";
const KEY_BASE: &str = "DuctusBase";

mod objects;
pub use objects::{PageObject, PlacedObject, read_objects};

#[derive(Debug, Clone)]
pub struct PageInk {
    pub page: usize,
    pub stroke: Stroke,
}

pub(crate) struct PageGeometry {
    pub(crate) llx: f64,
    pub(crate) lly: f64,
    pub(crate) urx: f64,
    pub(crate) ury: f64,
    pub(crate) rotate: i64,
}

fn number(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(f64::from(*r)),
        _ => None,
    }
}

pub(crate) fn inherited<'a>(doc: &'a Document, page: ObjectId, key: &[u8]) -> Option<&'a Object> {
    let mut id = page;
    for _ in 0..32 {
        let dict = doc.get_dictionary(id).ok()?;
        if let Ok(value) = dict.get(key) {
            return Some(match value {
                Object::Reference(r) => doc.get_object(*r).ok()?,
                v => v,
            });
        }
        id = dict.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

impl PageGeometry {
    pub(crate) fn of(doc: &Document, page: ObjectId) -> Self {
        let rect = inherited(doc, page, b"CropBox")
            .or_else(|| inherited(doc, page, b"MediaBox"))
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().filter_map(number).collect::<Vec<_>>())
            .filter(|v| v.len() == 4)
            .unwrap_or_else(|| vec![0.0, 0.0, 612.0, 792.0]);
        let rotate = inherited(doc, page, b"Rotate")
            .and_then(number)
            .map_or(0, |r| (r as i64).rem_euclid(360));
        Self {
            llx: rect[0].min(rect[2]),
            lly: rect[1].min(rect[3]),
            urx: rect[0].max(rect[2]),
            ury: rect[1].max(rect[3]),
            rotate,
        }
    }

    pub(crate) fn size(&self) -> (f64, f64) {
        (self.urx - self.llx, self.ury - self.lly)
    }

    pub(crate) fn to_user(&self, p: Point) -> Point {
        let (dx, dy) = (p.x / UNITS_PER_POINT, p.y / UNITS_PER_POINT);
        match self.rotate {
            90 => Point::new(self.llx + dy, self.lly + dx),
            180 => Point::new(self.urx - dx, self.lly + dy),
            270 => Point::new(self.urx - dy, self.ury - dx),
            _ => Point::new(self.llx + dx, self.ury - dy),
        }
    }
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

fn path_ops(path: &BezPath) -> String {
    let mut ops = String::new();
    let mut current = Point::ZERO;
    let p = |pt: Point| format!("{} {}", fmt(pt.x), fmt(pt.y));
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(a) => {
                writeln!(ops, "{} m", p(a)).unwrap();
                current = a;
            }
            PathEl::LineTo(a) => {
                writeln!(ops, "{} l", p(a)).unwrap();
                current = a;
            }
            PathEl::QuadTo(q, a) => {
                // PDF has cubic curves only.
                let c1 = current + (q - current) * (2.0 / 3.0);
                let c2 = a + (q - a) * (2.0 / 3.0);
                writeln!(ops, "{} {} {} c", p(c1), p(c2), p(a)).unwrap();
                current = a;
            }
            PathEl::CurveTo(c1, c2, a) => {
                writeln!(ops, "{} {} {} c", p(c1), p(c2), p(a)).unwrap();
                current = a;
            }
            PathEl::ClosePath => ops.push_str("h\n"),
        }
    }
    ops
}

fn ink_string(stroke: &Stroke) -> String {
    let t0 = stroke.points.first().map_or(0, |p| p.t);
    stroke
        .points
        .iter()
        .map(|p| {
            format!(
                "{},{},{},{}",
                fmt(p.x),
                fmt(p.y),
                fmt(f64::from(p.pressure)),
                p.t - t0
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn tilt_string(stroke: &Stroke) -> Option<String> {
    stroke
        .points
        .iter()
        .any(|p| p.tilt_x != 0.0 || p.tilt_y != 0.0)
        .then(|| {
            stroke
                .points
                .iter()
                .map(|p| format!("{},{}", fmt(f64::from(p.tilt_x)), fmt(f64::from(p.tilt_y))))
                .collect::<Vec<_>>()
                .join(" ")
        })
}

fn text(o: &Object) -> Option<String> {
    match o {
        Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).into_owned()),
        Object::Name(n) => Some(String::from_utf8_lossy(n).into_owned()),
        _ => None,
    }
}

fn parse_ink(ink: &str, tilt: Option<&str>) -> Vec<InkPoint> {
    let tilts: Vec<(f32, f32)> = tilt
        .map(|t| {
            t.split_whitespace()
                .filter_map(|pair| {
                    let (a, b) = pair.split_once(',')?;
                    Some((a.parse().ok()?, b.parse().ok()?))
                })
                .collect()
        })
        .unwrap_or_default();
    ink.split_whitespace()
        .enumerate()
        .filter_map(|(i, sample)| {
            let v: Vec<&str> = sample.split(',').collect();
            let (tilt_x, tilt_y) = tilts.get(i).copied().unwrap_or((0.0, 0.0));
            Some(InkPoint {
                x: v.first()?.parse().ok()?,
                y: v.get(1)?.parse().ok()?,
                pressure: v.get(2)?.parse().ok()?,
                t: v.get(3)?.parse().ok()?,
                tilt_x,
                tilt_y,
            })
        })
        .collect()
}

fn page_annotations(doc: &Document, page: ObjectId) -> Vec<(Object, Option<&Dictionary>)> {
    let annots = doc
        .get_dictionary(page)
        .ok()
        .and_then(|d| d.get(b"Annots").ok())
        .and_then(|a| match a {
            Object::Reference(r) => doc.get_object(*r).ok(),
            other => Some(other),
        })
        .and_then(|a| a.as_array().ok())
        .cloned()
        .unwrap_or_default();
    annots
        .into_iter()
        .map(|entry| {
            let dict = match &entry {
                Object::Reference(r) => doc.get_dictionary(*r).ok(),
                _ => None,
            };
            (entry, dict)
        })
        .collect()
}

fn is_ours(entry: &(Object, Option<&Dictionary>)) -> bool {
    let ours = |d: &Dictionary| d.has(KEY_INK.as_bytes()) || d.has(objects::KEY_OBJECT.as_bytes());
    match (&entry.0, entry.1) {
        (_, Some(d)) => ours(d),
        (Object::Dictionary(d), None) => ours(d),
        _ => false,
    }
}

pub fn read_ink(bytes: &[u8]) -> Result<Vec<PageInk>, Error> {
    let doc = Document::load_mem(bytes)?;
    let mut out = Vec::new();
    for (index, (_, page)) in doc.get_pages().into_iter().enumerate() {
        for entry in page_annotations(&doc, page) {
            let dict = match (&entry.0, entry.1) {
                (_, Some(d)) => d,
                (Object::Dictionary(d), None) => d,
                _ => continue,
            };
            let Some(ink) = dict.get(KEY_INK.as_bytes()).ok().and_then(text) else {
                continue;
            };
            let tilt = dict.get(KEY_TILT.as_bytes()).ok().and_then(text);
            let points = parse_ink(&ink, tilt.as_deref());
            if points.is_empty() {
                continue;
            }
            let tool = match dict.get(KEY_TOOL.as_bytes()).ok().and_then(text).as_deref() {
                Some("highlighter") => Tool::Highlighter,
                _ => Tool::Pen,
            };
            let rgb: Vec<f64> = dict
                .get(b"C")
                .ok()
                .and_then(|c| c.as_array().ok())
                .map(|a| a.iter().filter_map(number).collect())
                .unwrap_or_default();
            let alpha = dict.get(b"CA").ok().and_then(number).unwrap_or(1.0);
            let color = match rgb.as_slice() {
                [r, g, b] => Rgba {
                    r: *r as f32,
                    g: *g as f32,
                    b: *b as f32,
                    a: alpha as f32,
                },
                _ => Rgba::BLACK,
            };
            let width = dict
                .get(KEY_WIDTH.as_bytes())
                .ok()
                .and_then(number)
                .unwrap_or(3.0);
            out.push(PageInk {
                page: index,
                stroke: Stroke {
                    id: StrokeId(0),
                    style: StrokeStyle {
                        tool,
                        color,
                        width,
                        pattern: dict
                            .get(KEY_LINE.as_bytes())
                            .ok()
                            .and_then(|o| o.as_name().ok())
                            .and_then(|n| LinePattern::from_name(&String::from_utf8_lossy(n)))
                            .unwrap_or_default(),
                    },
                    points,
                },
            });
        }
    }
    Ok(out)
}

/// The recorded base when the tail is exactly one update of ours, else the whole file.
fn base_length(bytes: &[u8]) -> usize {
    let Ok(doc) = Document::load_mem(bytes) else {
        return bytes.len();
    };
    let Some(base) = doc
        .trailer
        .get(KEY_BASE.as_bytes())
        .ok()
        .and_then(number)
        .map(|b| b as usize)
    else {
        return bytes.len();
    };
    if base == 0 || base >= bytes.len() {
        return bytes.len();
    }
    let tail = &bytes[base..];
    let eofs = tail.windows(5).filter(|w| w == b"%%EOF").count();
    let head_ends_well = bytes[..base].trim_ascii_end().ends_with(b"%%EOF");
    if eofs == 1 && head_ends_well {
        base
    } else {
        bytes.len()
    }
}

fn add_annotation(
    doc: &mut Document,
    page: ObjectId,
    geometry: &PageGeometry,
    ink: &PageInk,
) -> Option<ObjectId> {
    let stroke = &ink.stroke;
    let outline = ductus_ink::outline(&stroke.points, &stroke.style);
    if outline.elements().is_empty() {
        return None;
    }
    let mapped: BezPath = outline
        .elements()
        .iter()
        .map(|el| match *el {
            PathEl::MoveTo(a) => PathEl::MoveTo(geometry.to_user(a)),
            PathEl::LineTo(a) => PathEl::LineTo(geometry.to_user(a)),
            PathEl::QuadTo(a, b) => PathEl::QuadTo(geometry.to_user(a), geometry.to_user(b)),
            PathEl::CurveTo(a, b, c) => PathEl::CurveTo(
                geometry.to_user(a),
                geometry.to_user(b),
                geometry.to_user(c),
            ),
            PathEl::ClosePath => PathEl::ClosePath,
        })
        .collect();
    let bounds = mapped.bounding_box().inflate(1.0, 1.0);
    let rect = Object::Array(
        [bounds.x0, bounds.y0, bounds.x1, bounds.y1]
            .iter()
            .map(|v| Object::Real(*v as f32))
            .collect(),
    );
    let c = stroke.style.color;
    let highlighter = stroke.style.tool == Tool::Highlighter;

    let mut gs = Dictionary::new();
    gs.set("Type", Object::Name(b"ExtGState".to_vec()));
    gs.set("ca", Object::Real(c.a));
    gs.set("CA", Object::Real(c.a));
    if highlighter {
        // Text under a highlight stays dark in every viewer.
        gs.set("BM", Object::Name(b"Multiply".to_vec()));
    }
    let mut ext = Dictionary::new();
    ext.set("GS0", Object::Dictionary(gs));
    let mut resources = Dictionary::new();
    resources.set("ExtGState", Object::Dictionary(ext));

    let content = format!(
        "q /GS0 gs {} {} {} rg\n{}f Q\n",
        fmt(f64::from(c.r)),
        fmt(f64::from(c.g)),
        fmt(f64::from(c.b)),
        path_ops(&mapped)
    );
    let mut form = Dictionary::new();
    form.set("Type", Object::Name(b"XObject".to_vec()));
    form.set("Subtype", Object::Name(b"Form".to_vec()));
    form.set("BBox", rect.clone());
    form.set("Resources", Object::Dictionary(resources));
    let appearance = doc.add_object(Stream::new(form, content.into_bytes()));

    // The pen path for readers that rebuild ink from /InkList.
    let ink_list: Vec<Object> = stroke
        .points
        .iter()
        .flat_map(|p| {
            let u = geometry.to_user(Point::new(p.x, p.y));
            [Object::Real(u.x as f32), Object::Real(u.y as f32)]
        })
        .collect();
    let mut border = Dictionary::new();
    border.set(
        "W",
        Object::Real((stroke.style.width / UNITS_PER_POINT) as f32),
    );
    let mut ap = Dictionary::new();
    ap.set("N", Object::Reference(appearance));

    let mut annot = Dictionary::new();
    annot.set("Type", Object::Name(b"Annot".to_vec()));
    annot.set("Subtype", Object::Name(b"Ink".to_vec()));
    annot.set("Rect", rect);
    annot.set("InkList", Object::Array(vec![Object::Array(ink_list)]));
    annot.set(
        "C",
        Object::Array(vec![
            Object::Real(c.r),
            Object::Real(c.g),
            Object::Real(c.b),
        ]),
    );
    annot.set("CA", Object::Real(c.a));
    annot.set("BS", Object::Dictionary(border));
    annot.set("F", Object::Integer(4)); // printable
    annot.set("P", Object::Reference(page));
    annot.set("AP", Object::Dictionary(ap));
    annot.set(
        "T",
        Object::String(b"Ductus".to_vec(), StringFormat::Literal),
    );
    annot.set(
        KEY_INK,
        Object::String(ink_string(stroke).into_bytes(), StringFormat::Literal),
    );
    if let Some(tilt) = tilt_string(stroke) {
        annot.set(
            KEY_TILT,
            Object::String(tilt.into_bytes(), StringFormat::Literal),
        );
    }
    annot.set(
        KEY_TOOL,
        Object::Name(if highlighter {
            b"highlighter".to_vec()
        } else {
            b"pen".to_vec()
        }),
    );
    annot.set(KEY_WIDTH, Object::Real(stroke.style.width as f32));
    if stroke.style.pattern != LinePattern::Solid {
        annot.set(
            KEY_LINE,
            Object::Name(stroke.style.pattern.name().as_bytes().to_vec()),
        );
    }
    Some(doc.add_object(annot))
}

pub fn write_ink(bytes: &[u8], ink: &[PageInk]) -> Result<Vec<u8>, Error> {
    write_annotations(bytes, ink, &[])
}

/// Our ink, text boxes, formulas and pictures replace the ones saved before, in one update.
pub fn write_annotations(
    bytes: &[u8],
    ink: &[PageInk],
    objects: &[PlacedObject],
) -> Result<Vec<u8>, Error> {
    let base_len = base_length(bytes);
    let base = &bytes[..base_len];
    let mut inc = IncrementalDocument::load_from(base)?;
    let prev = inc.get_prev_documents().clone();

    for (index, (_, page)) in prev.get_pages().into_iter().enumerate() {
        let existing = page_annotations(&prev, page);
        let kept: Vec<Object> = existing
            .iter()
            .filter(|e| !is_ours(e))
            .map(|e| e.0.clone())
            .collect();
        let new_ink: Vec<&PageInk> = ink.iter().filter(|i| i.page == index).collect();
        let new_objects: Vec<&PlacedObject> = objects.iter().filter(|o| o.page == index).collect();
        if kept.len() == existing.len() && new_ink.is_empty() && new_objects.is_empty() {
            continue;
        }
        let geometry = PageGeometry::of(&prev, page);
        let mut annots = kept;
        for item in new_ink {
            if let Some(id) = add_annotation(&mut inc.new_document, page, &geometry, item) {
                annots.push(Object::Reference(id));
            }
        }
        for item in new_objects {
            if let Some(id) = objects::add_object(&mut inc.new_document, page, &geometry, item) {
                annots.push(Object::Reference(id));
            }
        }
        inc.opt_clone_object_to_new_document(page)?;
        inc.new_document
            .get_dictionary_mut(page)?
            .set("Annots", Object::Array(annots));
    }
    inc.new_document
        .trailer
        .set(KEY_BASE, Object::Integer(base_len as i64));
    let mut out = Vec::with_capacity(bytes.len() + 64 * 1024);
    inc.save_to(&mut out)?;
    Ok(out)
}

/// The canvas draws our ink itself; pdfium still shows other programs' annotations.
pub fn strip_ductus_annotations(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let mut doc = Document::load_mem(bytes)?;
    let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
    let mut changed = false;
    for page in pages {
        let existing = page_annotations(&doc, page);
        if !existing.iter().any(is_ours) {
            continue;
        }
        let kept: Vec<Object> = existing
            .iter()
            .filter(|e| !is_ours(e))
            .map(|e| e.0.clone())
            .collect();
        doc.get_dictionary_mut(page)?
            .set("Annots", Object::Array(kept));
        changed = true;
    }
    if !changed {
        return Ok(bytes.to_vec());
    }
    let mut out = Vec::new();
    doc.save_to(&mut out)?;
    Ok(out)
}

pub fn flatten(
    bytes: &[u8],
    ink: &[PageInk],
    pictures: &[crate::PagePicture],
) -> Result<Vec<u8>, Error> {
    let with_pictures = crate::pictures::add_to_pdf(bytes, pictures)?;
    let annotated = write_ink(&with_pictures, ink)?;
    let pdfium = crate::engine()?;
    let document = pdfium.load_pdf_from_byte_vec(annotated, None)?;
    for mut page in document.pages().iter() {
        page.flatten()?;
    }
    Ok(document.save_to_bytes()?)
}

#[cfg(test)]
mod tests;
