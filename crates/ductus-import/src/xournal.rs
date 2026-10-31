//! Xournal++ (.xopp) and Xournal (.xoj): gzipped XML in points, one sheet per page.
//! Xournal++ 1.3 may instead zip that XML with its attachments.

use crate::sheets::{Sheet, assemble};
use crate::{Entry, Import, picture, plain_markdown};
use base64::Engine as _;
use ductus_core::{
    Image, InkPoint, LinePattern, MathBox, PageBackground, PatternStyle, Rgba, Stroke, StrokeId,
    StrokeStyle, TextBox, Tool, UNITS_PER_POINT,
};
use ductus_format::Error;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

const K: f64 = UNITS_PER_POINT;
/// Ductus draws the thinnest part of a pressure stroke at this share of its width.
const MIN_WIDTH_RATIO: f64 = 0.25;

pub(crate) fn read(path: &Path) -> Result<Import, Error> {
    let raw = std::fs::read(path)?;
    let mut attachments = HashMap::new();
    let xml = if raw.starts_with(b"PK\x03\x04") {
        let bad =
            |e: zip::result::ZipError| Error::Invalid(format!("The package can't be read: {e}"));
        let mut zip = zip::ZipArchive::new(Cursor::new(raw)).map_err(bad)?;
        let mut xml = String::new();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(bad)?;
            let name = entry.name().map_err(bad)?.into_owned();
            if name == "content.xml" {
                xml = String::from_utf8(crate::read_unpacked(&mut entry)?)
                    .map_err(|_| Error::Invalid("The file isn't text.".into()))?;
            } else if name.starts_with("attachments/") && entry.is_file() {
                attachments.insert(name, crate::read_unpacked(&mut entry)?);
            }
        }
        xml
    } else if raw.starts_with(&[0x1f, 0x8b]) {
        String::from_utf8(crate::read_unpacked(flate2::read::MultiGzDecoder::new(
            &raw[..],
        ))?)
        .map_err(|_| Error::Invalid("The file isn't text.".into()))?
    } else {
        String::from_utf8(raw).map_err(|_| Error::Invalid("The file isn't text.".into()))?
    };
    let name = path
        .file_stem()
        .map_or_else(|| "Imported".into(), |s| s.to_string_lossy().into_owned());
    parse(&xml, path, attachments, name)
}

struct Parser<'a> {
    file: &'a Path,
    /// Files inside a packaged notebook, by path.
    attachments: HashMap<String, Vec<u8>>,
    sheets: Vec<Sheet>,
    pdfs: Vec<Vec<u8>>,
    pdf_paths: HashMap<String, usize>,
    /// Later PDF backgrounds only give a page number.
    current_pdf: Option<usize>,
    missing_pdf: Option<String>,
    /// Pixmap backgrounds by page, for pages that clone one.
    backdrops: HashMap<usize, Image>,
    pictures: Vec<(String, Vec<u8>)>,
    warnings: Vec<String>,
}

fn parse(
    xml: &str,
    file: &Path,
    attachments: HashMap<String, Vec<u8>>,
    name: String,
) -> Result<Import, Error> {
    let mut reader = Reader::from_str(xml);
    let mut p = Parser {
        file,
        attachments,
        sheets: Vec::new(),
        pdfs: Vec::new(),
        pdf_paths: HashMap::new(),
        current_pdf: None,
        missing_pdf: None,
        backdrops: HashMap::new(),
        pictures: Vec::new(),
        warnings: Vec::new(),
    };
    let mut saw_root = false;
    let mut skipped_maths = 0;
    loop {
        let event = reader.read_event()?;
        let element = match &event {
            Event::Start(e) | Event::Empty(e) => e.clone(),
            Event::Eof => break,
            _ => continue,
        };
        let mut attrs = HashMap::new();
        for attr in element.attributes() {
            let attr = attr.map_err(quick_xml::Error::from)?;
            let value = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)?
                .into_owned();
            attrs.insert(attr.key.into_inner().to_owned(), value);
        }
        let tag = element.name().into_inner().to_owned();
        if !saw_root {
            if tag != "xournal" {
                return Err(Error::Invalid("This isn't a Xournal++ notebook.".into()));
            }
            saw_root = true;
            continue;
        }
        let leaf = matches!(
            tag.as_str(),
            "stroke" | "text" | "link" | "image" | "teximage" | "title" | "preview"
        );
        let content = match event {
            Event::Start(_) if leaf => {
                let raw = reader.read_text(element.name())?;
                quick_xml::escape::unescape(&raw)?.into_owned()
            }
            _ => String::new(),
        };
        let number = |key: &str| attrs.get(key).and_then(|v| v.trim().parse::<f64>().ok());
        if tag == "page" {
            p.sheets.push(Sheet {
                width: number("width").unwrap_or(612.0) * K,
                height: number("height").unwrap_or(792.0) * K,
                ..Sheet::default()
            });
            continue;
        }
        if p.sheets.is_empty() {
            continue;
        }
        match tag.as_str() {
            "background" => p.background(&attrs),
            "stroke" => {
                if let Some(stroke) = stroke(&attrs, &content) {
                    p.sheet().strokes.push(stroke);
                }
            }
            "text" if !content.trim().is_empty() => {
                let text = text(&attrs, &content, plain_markdown(&content));
                p.sheet().texts.push(text);
            }
            "link" if !content.trim().is_empty() => {
                let url = attrs.get("url").map_or("", String::as_str);
                let markdown = format!(
                    "[{}](<{}>)",
                    plain_markdown(&content),
                    url.replace('>', "%3E")
                );
                let text = text(&attrs, &content, markdown);
                p.sheet().texts.push(text);
            }
            "image" => {
                let Some(bytes) = p.contents(&content) else {
                    continue;
                };
                let natural = pixel_size(&bytes);
                if let Some(image) = placement(&attrs, natural).and_then(|r| p.place(&bytes, r)) {
                    p.sheet().images.push(image);
                }
            }
            "teximage" => {
                let bytes = p.contents(&content).unwrap_or_default();
                let natural = pdf_size(&bytes).or_else(|| pixel_size(&bytes));
                let source = attrs.get("text").cloned().unwrap_or_default();
                let placed = placement(&attrs, natural);
                if let Some(math) = placed.and_then(|r| formula(&source, r)) {
                    p.sheet().maths.push(math);
                } else if let Some(image) = placed.and_then(|r| p.place(&bytes, r)) {
                    p.sheet().images.push(image);
                } else {
                    skipped_maths += 1;
                }
            }
            _ => {}
        }
    }
    if !saw_root {
        return Err(Error::Invalid("This isn't a Xournal++ notebook.".into()));
    }
    if let Some(missing) = p.missing_pdf.take() {
        p.warnings.push(format!(
            "The PDF {missing} wasn't found next to the notebook, so its pages are blank."
        ));
    }
    if skipped_maths > 0 {
        p.warnings.push(format!(
            "{skipped_maths} LaTeX formula(s) couldn't be brought over."
        ));
    }
    let page = assemble(name, p.sheets, &p.pdfs, p.pictures)?;
    Ok(Import {
        entry: Entry::Page(Box::new(page)),
        warnings: p.warnings,
    })
}

impl Parser<'_> {
    fn sheet(&mut self) -> &mut Sheet {
        self.sheets.last_mut().expect("inside a page")
    }

    fn background(&mut self, attrs: &HashMap<String, String>) {
        let index = self.sheets.len() - 1;
        match attrs.get("type").map(String::as_str) {
            Some("solid") => {
                let (pattern, spacing) = match attrs.get("style").map(String::as_str) {
                    Some("lined" | "ruled" | "staves") => (PatternStyle::Lined, 24.0),
                    Some("graph" | "isograph") => (PatternStyle::Grid, 14.17),
                    Some("dotted" | "isodotted") => (PatternStyle::Dotted, 14.17),
                    _ => (PatternStyle::Blank, 14.17),
                };
                let paper = attrs
                    .get("color")
                    .and_then(|c| color(c, PAPER_NAMES))
                    .map_or(PageBackground::default().paper, |c| Rgba { a: 1.0, ..c });
                self.sheet().background = Some(PageBackground {
                    pattern,
                    spacing: spacing * K,
                    paper,
                    ..PageBackground::default()
                });
            }
            Some("pdf") => {
                if let Some(filename) = attrs.get("filename") {
                    let domain = attrs.get("domain").map_or("absolute", String::as_str);
                    self.current_pdf = self.open_pdf(domain, filename);
                    if self.current_pdf.is_none() {
                        self.missing_pdf = Some(filename.clone());
                    }
                }
                let page = attrs
                    .get("pageno")
                    .and_then(|n| leading_number(n))
                    .unwrap_or(1)
                    .max(1);
                if let Some(file) = self.current_pdf {
                    self.sheet().pdf = Some((file, page - 1));
                }
            }
            Some("pixmap") => {
                let domain = attrs.get("domain").map_or("absolute", String::as_str);
                let filename = attrs.get("filename").cloned().unwrap_or_default();
                let backdrop = if domain == "clone" {
                    leading_number(&filename).and_then(|page| self.backdrops.get(&page).cloned())
                } else {
                    self.load(domain, &filename)
                        .and_then(|(_, bytes)| self.store(&bytes))
                        .map(|source| {
                            let sheet = &self.sheets[index];
                            Image {
                                id: StrokeId(0),
                                source,
                                x: 0.0,
                                y: 0.0,
                                width: sheet.width,
                                height: sheet.height,
                            }
                        })
                };
                if let Some(image) = backdrop {
                    self.backdrops.insert(index, image.clone());
                    self.sheet().images.insert(0, image);
                }
            }
            _ => {}
        }
    }

    fn open_pdf(&mut self, domain: &str, filename: &str) -> Option<usize> {
        let (key, bytes) = self.load(domain, filename)?;
        if let Some(&index) = self.pdf_paths.get(&key) {
            return Some(index);
        }
        self.pdfs.push(bytes);
        self.pdf_paths.insert(key, self.pdfs.len() - 1);
        Some(self.pdfs.len() - 1)
    }

    /// A file the notebook refers to, with a key naming it.
    /// Attached files sit in the package or next to the notebook; others may have moved along with it.
    fn load(&self, domain: &str, filename: &str) -> Option<(String, Vec<u8>)> {
        if let Some(bytes) = self.attachments.get(filename) {
            return Some((filename.to_owned(), bytes.clone()));
        }
        let dir = self.file.parent().unwrap_or(Path::new("."));
        let filename = filename.strip_prefix("file://").unwrap_or(filename);
        let mut candidates = Vec::new();
        if domain == "attach"
            && let Some(own) = self.file.file_name()
        {
            let mut name = own.to_os_string();
            name.push(".");
            name.push(filename);
            candidates.push(dir.join(name));
        }
        // Xournal++ keeps the PDF or picture a page is drawn over by its absolute path; relative
        // names stay next to the notebook.
        let path = Path::new(filename);
        let climbs = path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
        if path.is_absolute() && domain == "absolute" || path.is_relative() && !climbs {
            candidates.push(dir.join(filename));
        }
        if let Some(base) = path.file_name() {
            candidates.push(dir.join(base));
        }
        let path = candidates.into_iter().find(|p| p.is_file())?;
        let bytes = std::fs::read(&path).ok()?;
        // A notebook may only bring in PDFs and pictures, not any file it names.
        is_pdf_or_picture(&bytes).then(|| (path.to_string_lossy().into_owned(), bytes))
    }

    /// An element's base64 data, or the attachment it points to.
    fn contents(&self, content: &str) -> Option<Vec<u8>> {
        let content = content.trim();
        if content.starts_with("<attachment") {
            let path = content.split("path=\"").nth(1)?.split('"').next()?;
            let path = quick_xml::escape::unescape(path).ok()?;
            return self.load("attach", &path).map(|(_, bytes)| bytes);
        }
        let data: String = content.chars().filter(|c| !c.is_whitespace()).collect();
        base64::engine::general_purpose::STANDARD.decode(data).ok()
    }

    fn store(&mut self, bytes: &[u8]) -> Option<String> {
        let (name, png) = picture(bytes)?;
        if !self.pictures.iter().any(|(n, _)| *n == name) {
            self.pictures.push((name.clone(), png));
        }
        Some(name)
    }

    fn place(&mut self, bytes: &[u8], (x, y, w, h): Rect) -> Option<Image> {
        let source = self.store(bytes)?;
        Some(Image {
            id: StrokeId(0),
            source,
            x: x * K,
            y: y * K,
            width: w * K,
            height: h * K,
        })
    }
}

/// Left, top, width and height in points.
type Rect = (f64, f64, f64, f64);

fn numbers(s: &str) -> Vec<f64> {
    s.split_whitespace()
        .filter_map(|v| v.parse().ok())
        .collect()
}

/// Version 4 gives the corners; version 5 a matrix applied to the natural size.
/// Ductus doesn't turn pictures, so a turned one keeps its size, centred where it was.
fn placement(attrs: &HashMap<String, String>, natural: Option<(f64, f64)>) -> Option<Rect> {
    let number = |key: &str| attrs.get(key).and_then(|v| v.trim().parse::<f64>().ok());
    if let (Some(l), Some(t), Some(r), Some(b)) = (
        number("left"),
        number("top"),
        number("right"),
        number("bottom"),
    ) {
        return Some((l.min(r), t.min(b), (r - l).abs(), (b - t).abs()));
    }
    let m = numbers(attrs.get("matrix")?);
    let [a, b, c, d, tx, ty] = m[..] else {
        return None;
    };
    let (w, h) = natural?;
    let (cx, cy) = (
        a * w / 2.0 + c * h / 2.0 + tx,
        b * w / 2.0 + d * h / 2.0 + ty,
    );
    let (sw, sh) = (a.hypot(b) * w, c.hypot(d) * h);
    Some((cx - sw / 2.0, cy - sh / 2.0, sw, sh))
}

fn pixel_size(bytes: &[u8]) -> Option<(f64, f64)> {
    let (w, h) = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    Some((f64::from(w), f64::from(h)))
}

/// The first page's size, for LaTeX that Xournal++ rendered as PDF.
fn pdf_size(bytes: &[u8]) -> Option<(f64, f64)> {
    if !bytes.starts_with(b"%PDF") {
        return None;
    }
    let doc = lopdf::Document::load_mem(bytes).ok()?;
    let page = *doc.get_pages().values().next()?;
    let mut id = page;
    for _ in 0..16 {
        let dict = doc.get_dictionary(id).ok()?;
        if let Ok(media) = dict.get(b"MediaBox").and_then(|m| doc.dereference(m)) {
            let v: Vec<f64> = media
                .1
                .as_array()
                .ok()?
                .iter()
                .filter_map(|n| n.as_float().ok().map(f64::from))
                .collect();
            return (v.len() == 4).then(|| ((v[2] - v[0]).abs(), (v[3] - v[1]).abs()));
        }
        id = dict.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

fn leading_number(s: &str) -> Option<usize> {
    let digits: String = s.trim().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

const INK_NAMES: &[(&str, u32)] = &[
    ("black", 0x000000),
    ("blue", 0x3333cc),
    ("red", 0xff0000),
    ("green", 0x008000),
    ("gray", 0x808080),
    ("lightblue", 0x00c0ff),
    ("lightgreen", 0x00ff00),
    ("magenta", 0xff00ff),
    ("orange", 0xff8000),
    ("yellow", 0xffff00),
    ("white", 0xffffff),
];

const PAPER_NAMES: &[(&str, u32)] = &[
    ("white", 0xffffff),
    ("yellow", 0xffff80),
    ("pink", 0xffc0d4),
    ("orange", 0xffc080),
    ("blue", 0xa0e8ff),
    ("green", 0x80ffc0),
];

/// `#rrggbbaa`, `#rrggbb` or one of Xournal's color names.
fn color(s: &str, names: &[(&str, u32)]) -> Option<Rgba> {
    let channel = |v: u32, shift: u32| ((v >> shift) & 0xff) as f32 / 255.0;
    let (rgb, a) = match s.trim().strip_prefix('#') {
        Some(hex) if hex.len() == 8 => {
            let v = u32::from_str_radix(hex, 16).ok()?;
            (v >> 8, channel(v, 0))
        }
        Some(hex) if hex.len() == 6 => (u32::from_str_radix(hex, 16).ok()?, 1.0),
        Some(_) => return None,
        None => (names.iter().find(|(n, _)| *n == s.trim())?.1, 1.0),
    };
    Some(Rgba {
        r: channel(rgb, 16),
        g: channel(rgb, 8),
        b: channel(rgb, 0),
        a,
    })
}

fn stroke(attrs: &HashMap<String, String>, coords: &str) -> Option<Stroke> {
    let numbers: Vec<f64> = coords
        .split_whitespace()
        .filter_map(|v| v.parse().ok())
        .collect();
    let xy: Vec<(f64, f64)> = numbers
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| (c[0], c[1]))
        .collect();
    if xy.is_empty() {
        return None;
    }
    let widths: Vec<f64> = attrs
        .get("width")
        .map(|w| {
            w.split_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    let nominal = widths.first().copied().filter(|w| *w > 0.0).unwrap_or(1.41);
    // With pressure, each point but the last has its own width.
    let per_point = &widths[1.min(widths.len())..];
    let tool = match attrs.get("tool").map(String::as_str) {
        Some("highlighter") => Tool::Highlighter,
        _ => Tool::Pen,
    };
    let widest = per_point.iter().copied().fold(nominal, f64::max);
    let width = match tool {
        Tool::Highlighter => nominal,
        Tool::Pen => widest,
    };
    let mut color = attrs
        .get("color")
        .and_then(|c| color(c, INK_NAMES))
        .unwrap_or(Rgba::BLACK);
    if tool == Tool::Highlighter {
        // Ductus makes highlighters translucent itself.
        color.a = 1.0;
    }
    let pattern = match attrs.get("style").map(String::as_str) {
        Some(s) if s.contains("dash") || s.starts_with("cust") => LinePattern::Dashed,
        Some(s) if s.contains("dot") => LinePattern::Dotted,
        _ => LinePattern::Solid,
    };
    let points = xy
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            let pressure = match per_point.get(i).or(per_point.last()) {
                Some(&w) => {
                    ((w / width - MIN_WIDTH_RATIO) / (1.0 - MIN_WIDTH_RATIO)).clamp(0.0, 1.0)
                }
                None => 1.0,
            };
            InkPoint {
                x: x * K,
                y: y * K,
                pressure: pressure as f32,
                tilt_x: 0.0,
                tilt_y: 0.0,
                t: 0,
            }
        })
        .collect();
    Some(Stroke {
        id: StrokeId(0),
        style: StrokeStyle {
            tool,
            color,
            width: width * K,
            pattern,
        },
        points: crate::resample(points),
    })
}

fn text(attrs: &HashMap<String, String>, content: &str, markdown: String) -> TextBox {
    let number = |key: &str| attrs.get(key).and_then(|v| v.trim().parse::<f64>().ok());
    let size = number("size").unwrap_or(12.0);
    // Version 5 places text by a matrix instead.
    let matrix = attrs
        .get("matrix")
        .map(|m| numbers(m))
        .filter(|m| m.len() == 6);
    let (x, y) = match (&matrix, number("x"), number("y")) {
        (_, Some(x), Some(y)) => (x, y),
        (Some(m), ..) => (m[4], m[5]),
        _ => (0.0, 0.0),
    };
    let lines: Vec<&str> = content.lines().collect();
    let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    // Xournal doesn't store the box; this is a little wider than the text.
    let pad = 8.0;
    TextBox {
        id: StrokeId(0),
        markdown,
        x: x * K - pad / 2.0,
        y: y * K - pad / 2.0,
        width: (longest.max(4) as f64 * size * 0.6) * K + pad,
        height: (lines.len().max(1) as f64 * size * 1.35) * K + pad,
        scale: size * K / TextBox::FONT_SIZE,
    }
}

/// The formula as Ductus math, sized to the picture Xournal rendered.
fn formula(source: &str, (x, y, _, h): Rect) -> Option<MathBox> {
    let rendered =
        ductus_math::render(source, ductus_math::Syntax::Latex, ductus_math::BODY_SIZE).ok()?;
    let scale = h / rendered.height.max(0.01);
    Some(MathBox {
        id: StrokeId(0),
        source: source.trim().to_owned(),
        syntax: ductus_math::Syntax::Latex.name().into(),
        x: x * K,
        y: y * K,
        width: rendered.width * scale * K,
        height: rendered.height * scale * K,
        scale,
        color: Rgba::BLACK,
    })
}

fn is_pdf_or_picture(bytes: &[u8]) -> bool {
    let webp = bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP";
    [
        &b"%PDF-"[..],
        b"\x89PNG\r\n\x1a\n",
        b"\xff\xd8\xff",
        b"GIF87a",
        b"GIF89a",
        b"BM",
    ]
    .iter()
    .any(|magic| bytes.starts_with(magic))
        || webp
}

#[cfg(test)]
mod tests;
