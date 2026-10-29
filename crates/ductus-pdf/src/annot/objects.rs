//! Text boxes, formulas and pictures on a PDF written in place. Other readers show them as
//! standard annotations; Ductus keeps its own data in them, to edit them again.

use super::{PageGeometry, fmt, number, path_ops, text};
use crate::{Error, PdfImage};
use ductus_core::{Image, MathBox, Rgba, StrokeId, TextBox, UNITS_PER_POINT};
use kurbo::{BezPath, PathEl, Point, Rect};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat, dictionary};

pub(super) const KEY_OBJECT: &str = "DuctusObject";
const KEY_SOURCE: &str = "DuctusSource";
const KEY_SYNTAX: &str = "DuctusSyntax";
const KEY_SCALE: &str = "DuctusScale";
/// Where it sits on its page, in document units: "x y width height".
const KEY_BOX: &str = "DuctusBox";

#[derive(Debug, Clone, PartialEq)]
pub enum PageObject {
    Text(TextBox),
    Math(MathBox),
    /// A picture and its PNG.
    Picture(Image, Vec<u8>),
}

/// An object on a page of the PDF, in that page's document units.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedObject {
    pub page: usize,
    pub object: PageObject,
}

impl PageObject {
    pub fn rect(&self) -> Rect {
        let (x, y, w, h) = match self {
            PageObject::Text(t) => (t.x, t.y, t.width, t.height),
            PageObject::Math(m) => (m.x, m.y, m.width, m.height),
            PageObject::Picture(i, _) => (i.x, i.y, i.width, i.height),
        };
        Rect::new(x, y, x + w, y + h)
    }

    pub fn moved(mut self, dx: f64, dy: f64) -> Self {
        match &mut self {
            PageObject::Text(t) => (t.x, t.y) = (t.x + dx, t.y + dy),
            PageObject::Math(m) => (m.x, m.y) = (m.x + dx, m.y + dy),
            PageObject::Picture(i, _) => (i.x, i.y) = (i.x + dx, i.y + dy),
        }
        self
    }
}

fn literal(s: &str) -> Object {
    Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
}

/// A PDF text string: UTF-16 with a byte order mark reads right in every reader.
fn text_string(s: &str) -> Object {
    let mut bytes = vec![0xfe, 0xff];
    bytes.extend(s.encode_utf16().flat_map(u16::to_be_bytes));
    Object::String(bytes, StringFormat::Hexadecimal)
}

fn string_value(dict: &Dictionary, key: &str) -> Option<String> {
    dict.get(key.as_bytes()).ok().and_then(text)
}

/// The text without Markdown marks and tags, for readers that only show plain text.
fn plain_text(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let line = line.trim_start_matches('#').trim_start();
            let line = match line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
                Some(rest) => format!("\u{2022} {rest}"),
                None => line.to_owned(),
            };
            let mut out = String::new();
            let mut in_tag = false;
            let mut chars = line.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    '<' => in_tag = true,
                    '>' if in_tag => in_tag = false,
                    _ if in_tag => {}
                    '\\' => out.extend(chars.next()),
                    '*' | '_' if chars.peek() == Some(&c) => {
                        chars.next();
                    }
                    _ => out.push(c),
                }
            }
            out
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Helvetica's built-in encoding; anything else shows as "?" in other readers.
fn win_ansi(c: char) -> u8 {
    match c {
        ' '..='~' => c as u8,
        '\u{a0}'..='\u{ff}' => c as u32 as u8,
        '\u{2022}' => 0x95,
        '\u{2013}' => 0x96,
        '\u{2014}' => 0x97,
        '\u{2018}' => 0x91,
        '\u{2019}' => 0x92,
        '\u{201c}' => 0x93,
        '\u{201d}' => 0x94,
        '\u{20ac}' => 0x80,
        _ => b'?',
    }
}

/// Lines that fit `chars` characters, broken at spaces where possible.
fn wrap(text: &str, chars: usize) -> Vec<String> {
    let chars = chars.max(4);
    let mut out = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let fits = line.chars().count() + word.chars().count() < chars;
            if !line.is_empty() && !fits {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push(line);
    }
    out
}

fn user_rect(geometry: &PageGeometry, rect: Rect) -> Rect {
    let corners = [
        Point::new(rect.x0, rect.y0),
        Point::new(rect.x1, rect.y0),
        Point::new(rect.x0, rect.y1),
        Point::new(rect.x1, rect.y1),
    ]
    .map(|p| geometry.to_user(p));
    corners
        .iter()
        .skip(1)
        .fold(Rect::from_points(corners[0], corners[0]), |r, p| {
            r.union_pt(*p)
        })
}

fn rect_object(r: Rect) -> Object {
    Object::Array(
        [r.x0, r.y0, r.x1, r.y1]
            .iter()
            .map(|v| Object::Real(*v as f32))
            .collect(),
    )
}

fn form(doc: &mut Document, bbox: Rect, resources: Dictionary, content: Vec<u8>) -> ObjectId {
    let dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => rect_object(bbox),
        "Resources" => resources,
    };
    doc.add_object(Stream::new(dict, content))
}

fn text_appearance(doc: &mut Document, rect: Rect, t: &TextBox) -> ObjectId {
    let size = TextBox::FONT_SIZE * t.scale / UNITS_PER_POINT;
    let leading = size * 1.3;
    // Helvetica averages about half an em per character.
    let lines = wrap(
        &plain_text(&t.markdown),
        (rect.width() / (size * 0.5)) as usize,
    );
    let mut content = format!(
        "BT /F1 {} Tf 0.102 0.102 0.122 rg {} TL {} {} Td\n",
        fmt(size),
        fmt(leading),
        fmt(rect.x0),
        fmt(rect.y1 - size)
    )
    .into_bytes();
    for line in lines {
        content.push(b'(');
        for c in line.chars() {
            let b = win_ansi(c);
            if matches!(b, b'(' | b')' | b'\\') {
                content.push(b'\\');
            }
            content.push(b);
        }
        content.extend_from_slice(b") Tj T*\n");
    }
    content.extend_from_slice(b"ET\n");
    let font = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    };
    let resources = dictionary! { "Font" => dictionary! { "F1" => font } };
    form(doc, rect, resources, content)
}

fn math_appearance(
    doc: &mut Document,
    geometry: &PageGeometry,
    rect: Rect,
    m: &MathBox,
) -> ObjectId {
    let mut content = String::new();
    for (path, color) in ductus_math::outlines(m) {
        let mapped: BezPath = path
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
        content.push_str(&format!(
            "{} {} {} rg\n{}f\n",
            fmt(f64::from(color.r)),
            fmt(f64::from(color.g)),
            fmt(f64::from(color.b)),
            path_ops(&mapped)
        ));
    }
    form(doc, rect, Dictionary::new(), content.into_bytes())
}

fn picture_appearance(doc: &mut Document, rect: Rect, png: &[u8]) -> Option<ObjectId> {
    let image = PdfImage::from_png(png)?.add_to(doc);
    let content = format!(
        "q {} 0 0 {} {} {} cm /Im0 Do Q\n",
        fmt(rect.width()),
        fmt(rect.height()),
        fmt(rect.x0),
        fmt(rect.y0)
    );
    let resources = dictionary! { "XObject" => dictionary! { "Im0" => image } };
    Some(form(doc, rect, resources, content.into_bytes()))
}

pub(super) fn add_object(
    doc: &mut Document,
    page: ObjectId,
    geometry: &PageGeometry,
    placed: &PlacedObject,
) -> Option<ObjectId> {
    let local = placed.object.rect();
    if local.width() <= 0.0 || local.height() <= 0.0 {
        return None;
    }
    let rect = user_rect(geometry, local);
    let mut annot = dictionary! {
        "Type" => "Annot",
        "Rect" => rect_object(rect),
        "F" => 4, // printable
        "P" => page,
        "T" => literal("Ductus"),
        KEY_BOX => literal(&format!(
            "{} {} {} {}",
            fmt(local.x0),
            fmt(local.y0),
            fmt(local.width()),
            fmt(local.height())
        )),
    };
    let appearance = match &placed.object {
        PageObject::Text(t) => {
            let size = TextBox::FONT_SIZE * t.scale / UNITS_PER_POINT;
            annot.set("Subtype", "FreeText");
            annot.set("Contents", text_string(&plain_text(&t.markdown)));
            annot.set("DA", literal(&format!("/Helv {} Tf 0 g", fmt(size))));
            annot.set(KEY_OBJECT, "text");
            annot.set(KEY_SOURCE, literal(&t.markdown));
            annot.set(KEY_SCALE, Object::Real(t.scale as f32));
            text_appearance(doc, rect, t)
        }
        PageObject::Math(m) => {
            annot.set("Subtype", "Stamp");
            annot.set("Name", "Formula");
            annot.set("Contents", text_string(&m.source));
            annot.set(
                "C",
                Object::Array(vec![
                    Object::Real(m.color.r),
                    Object::Real(m.color.g),
                    Object::Real(m.color.b),
                ]),
            );
            annot.set(KEY_OBJECT, "math");
            annot.set(KEY_SOURCE, literal(&m.source));
            annot.set(KEY_SYNTAX, Object::Name(m.syntax.as_bytes().to_vec()));
            annot.set(KEY_SCALE, Object::Real(m.scale as f32));
            math_appearance(doc, geometry, rect, m)
        }
        PageObject::Picture(image, png) => {
            annot.set("Subtype", "Stamp");
            annot.set("Name", "Picture");
            annot.set(KEY_OBJECT, "picture");
            annot.set(KEY_SOURCE, literal(&image.source));
            picture_appearance(doc, rect, png)?
        }
    };
    annot.set("AP", dictionary! { "N" => appearance });
    Some(doc.add_object(annot))
}

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a Object> {
    match o {
        Object::Reference(r) => doc.get_object(*r).ok(),
        other => Some(other),
    }
}

/// The PNG drawn by a picture annotation's appearance.
fn picture_png(doc: &Document, annot: &Dictionary) -> Option<Vec<u8>> {
    let ap = resolve(doc, annot.get(b"AP").ok()?)?.as_dict().ok()?;
    let form = resolve(doc, ap.get(b"N").ok()?)?.as_stream().ok()?;
    let resources = resolve(doc, form.dict.get(b"Resources").ok()?)?
        .as_dict()
        .ok()?;
    let xobjects = resolve(doc, resources.get(b"XObject").ok()?)?
        .as_dict()
        .ok()?;
    let image = resolve(doc, xobjects.get(b"Im0").ok()?)?.as_stream().ok()?;
    let width = u32::try_from(image.dict.get(b"Width").ok()?.as_i64().ok()?).ok()?;
    let height = u32::try_from(image.dict.get(b"Height").ok()?.as_i64().ok()?).ok()?;
    let data = |s: &Stream| {
        s.decompressed_content()
            .unwrap_or_else(|_| s.content.clone())
    };
    let rgb = data(image);
    let pixels = width as usize * height as usize;
    let alpha = image
        .dict
        .get(b"SMask")
        .ok()
        .and_then(|m| resolve(doc, m))
        .and_then(|m| m.as_stream().ok())
        .map(data)
        .filter(|a| a.len() >= pixels);
    if rgb.len() < pixels * 3 {
        return None;
    }
    let mut premultiplied = Vec::with_capacity(pixels * 4);
    for i in 0..pixels {
        let a = alpha.as_ref().map_or(255, |a| a[i]);
        let scale = |c: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
        premultiplied.extend([
            scale(rgb[i * 3]),
            scale(rgb[i * 3 + 1]),
            scale(rgb[i * 3 + 2]),
            a,
        ]);
    }
    let size = tiny_skia::IntSize::from_wh(width, height)?;
    tiny_skia::Pixmap::from_vec(premultiplied, size)?
        .encode_png()
        .ok()
}

/// Ductus's text boxes, formulas and pictures in a PDF written in place.
pub fn read_objects(bytes: &[u8]) -> Result<Vec<PlacedObject>, Error> {
    let doc = Document::load_mem(bytes)?;
    let mut out = Vec::new();
    for (index, (_, page)) in doc.get_pages().into_iter().enumerate() {
        for entry in super::page_annotations(&doc, page) {
            let dict = match (&entry.0, entry.1) {
                (_, Some(d)) => d,
                (Object::Dictionary(d), None) => d,
                _ => continue,
            };
            let Some(kind) = string_value(dict, KEY_OBJECT) else {
                continue;
            };
            let numbers: Vec<f64> = string_value(dict, KEY_BOX)
                .unwrap_or_default()
                .split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect();
            let [x, y, width, height] = numbers[..] else {
                continue;
            };
            let source = string_value(dict, KEY_SOURCE).unwrap_or_default();
            let scale = dict
                .get(KEY_SCALE.as_bytes())
                .ok()
                .and_then(number)
                .unwrap_or(1.0);
            let object = match kind.as_str() {
                "text" => PageObject::Text(TextBox {
                    id: StrokeId(0),
                    markdown: source,
                    x,
                    y,
                    width,
                    height,
                    scale,
                }),
                "math" => {
                    let rgb: Vec<f64> = dict
                        .get(b"C")
                        .ok()
                        .and_then(|c| c.as_array().ok())
                        .map(|a| a.iter().filter_map(number).collect())
                        .unwrap_or_default();
                    let color = match rgb[..] {
                        [r, g, b] => Rgba {
                            r: r as f32,
                            g: g as f32,
                            b: b as f32,
                            a: 1.0,
                        },
                        _ => Rgba::BLACK,
                    };
                    PageObject::Math(MathBox {
                        id: StrokeId(0),
                        source,
                        syntax: string_value(dict, KEY_SYNTAX).unwrap_or_else(|| "latex".into()),
                        x,
                        y,
                        width,
                        height,
                        scale,
                        color,
                    })
                }
                "picture" => {
                    let Some(png) = picture_png(&doc, dict) else {
                        continue;
                    };
                    let image = Image {
                        id: StrokeId(0),
                        source,
                        x,
                        y,
                        width,
                        height,
                    };
                    PageObject::Picture(image, png)
                }
                _ => continue,
            };
            out.push(PlacedObject {
                page: index,
                object,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_drops_markdown_marks() {
        assert_eq!(
            plain_text("# Title\n- **bold** item\n<u>under</u> 2\\*3"),
            "Title\n\u{2022} bold item\nunder 2*3"
        );
    }

    #[test]
    fn text_formulas_and_pictures_come_back_from_the_pdf() {
        let pdf = crate::testpdf::make_pdf(&[(595.0, 842.0, 0), (595.0, 842.0, 90)]);
        let mut pixmap = tiny_skia::Pixmap::new(3, 2).unwrap();
        pixmap.fill(tiny_skia::Color::from_rgba8(200, 30, 30, 255));
        let png = pixmap.encode_png().unwrap();
        let text = TextBox {
            id: StrokeId(0),
            markdown: "# Übung\n- **eins** (1)".into(),
            x: 40.0,
            y: 60.0,
            width: 300.0,
            height: 50.0,
            scale: 1.0,
        };
        let math = MathBox {
            id: StrokeId(0),
            source: r"\frac{1}{2}".into(),
            syntax: "latex".into(),
            x: 100.0,
            y: 400.0,
            width: 30.0,
            height: 40.0,
            scale: 1.5,
            color: Rgba::rgb(0.1, 0.2, 0.9),
        };
        let image = Image {
            id: StrokeId(0),
            source: "img-1.png".into(),
            x: 20.0,
            y: 30.0,
            width: 90.0,
            height: 60.0,
        };
        let placed = vec![
            PlacedObject {
                page: 0,
                object: PageObject::Text(text),
            },
            PlacedObject {
                page: 0,
                object: PageObject::Math(math),
            },
            PlacedObject {
                page: 1,
                object: PageObject::Picture(image, png.clone()),
            },
        ];
        let saved = super::super::write_annotations(&pdf, &[], &placed).unwrap();
        assert_eq!(read_objects(&saved).unwrap(), placed);
        // Saving again replaces them; ink-only saves (export) drop them.
        let again = super::super::write_annotations(&saved, &[], &placed[..1]).unwrap();
        assert_eq!(read_objects(&again).unwrap(), placed[..1]);
        assert!(
            read_objects(&super::super::write_ink(&again, &[]).unwrap())
                .unwrap()
                .is_empty()
        );
        // Other readers get the text as plain text.
        let doc = Document::load_mem(&saved).unwrap();
        let page = *doc.get_pages().get(&1).unwrap();
        let annots = super::super::page_annotations(&doc, page);
        let free_text = annots
            .iter()
            .filter_map(|a| a.1)
            .find(|d| d.get(b"Subtype").ok() == Some(&Object::Name(b"FreeText".to_vec())))
            .unwrap();
        let Object::String(contents, _) = free_text.get(b"Contents").unwrap() else {
            panic!()
        };
        let utf16: Vec<u16> = contents[2..]
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(
            String::from_utf16(&utf16).unwrap(),
            "Übung\n\u{2022} eins (1)"
        );
    }

    #[test]
    fn long_lines_wrap_at_spaces() {
        assert_eq!(wrap("one two three four", 9), ["one two", "three", "four"]);
    }
}
