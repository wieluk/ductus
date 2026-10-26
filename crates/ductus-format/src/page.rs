//! Pages are plain SVG: the ink is ordinary paths; the raw pen samples live in the Ductus namespace.

use crate::Error;
use ductus_core::{
    Document, Image, InkPoint, LinePattern, MathBox, Orientation, PageBackground, PageKind,
    PageSettings, PaperSize, PatternStyle, Rgba, Stroke, StrokeId, StrokeStyle, TextBox, Tool,
    UNITS_PER_MM,
};
use kurbo::{BezPath, PathEl, Rect};
use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::{Namespace, ResolveResult};
use std::fmt::Write as _;

pub const SVG_NS: &str = "http://www.w3.org/2000/svg";
pub const DX_NS: &str = "https://ductus.app/ns/1";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct PageFile {
    pub id: String,
    pub document: Document,
    pub pdf: Option<String>,
    /// A cache: it can always be recognized again from the ink.
    pub recognized: Vec<RecognizedLine>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecognizedLine {
    /// Changes with the ink, so edited lines are recognized again.
    pub key: String,
    pub text: String,
    pub rect: (f64, f64, f64, f64),
    pub lang: Option<String>,
}

fn sheet_rects(doc: &Document) -> Vec<Rect> {
    doc.sheets(0)
        .into_iter()
        .map(|(x, y, w, h)| Rect::new(x, y, x + w, y + h))
        .collect()
}

fn num(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        &s
    };
    if s == "-0" { "0".into() } else { s.into() }
}

pub(crate) fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn hex(c: Rgba) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b))
}

fn path_data(path: &BezPath) -> String {
    let mut d = String::new();
    let p = |pt: kurbo::Point| format!("{},{}", num(pt.x, 2), num(pt.y, 2));
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(a) => write!(d, "M{}", p(a)),
            PathEl::LineTo(a) => write!(d, "L{}", p(a)),
            PathEl::QuadTo(a, b) => write!(d, "Q{} {}", p(a), p(b)),
            PathEl::CurveTo(a, b, c) => write!(d, "C{} {} {}", p(a), p(b), p(c)),
            PathEl::ClosePath => write!(d, "Z"),
        }
        .expect("writing to a String cannot fail");
    }
    d
}

fn canvas_rect(doc: &Document) -> Rect {
    if let Some(sheets) = sheet_rects(doc).into_iter().reduce(|a, b| a.union(b)) {
        return sheets;
    }
    let bounds = doc
        .strokes()
        .iter()
        .map(ductus_ink::bounds)
        .chain(
            doc.images()
                .iter()
                .map(|i| Rect::new(i.x, i.y, i.x + i.width, i.y + i.height)),
        )
        .chain(
            doc.texts()
                .iter()
                .map(|t| Rect::new(t.x, t.y, t.x + t.width, t.y + t.height)),
        )
        .chain(
            doc.maths()
                .iter()
                .map(|m| Rect::new(m.x, m.y, m.x + m.width, m.y + m.height)),
        )
        .reduce(|a, b| a.union(b));
    match bounds {
        Some(b) => b.inflate(24.0, 24.0),
        None => Rect::new(0.0, 0.0, 800.0, 600.0),
    }
}

fn pattern_def(bg: &PageBackground, origin: kurbo::Point) -> Option<String> {
    let s = num(bg.spacing, 3);
    let ink = hex(bg.ink);
    let (x, y) = (num(origin.x, 2), num(origin.y, 2));
    let content = match bg.pattern {
        PatternStyle::Blank => return None,
        PatternStyle::Lined => {
            format!(r#"<path d="M0,{s}H{s}" stroke="{ink}" stroke-width="0.6"/>"#)
        }
        PatternStyle::Grid => {
            format!(r#"<path d="M0,{s}H{s}M{s},0V{s}" stroke="{ink}" stroke-width="0.5"/>"#)
        }
        PatternStyle::Dotted => format!(
            r#"<circle cx="0" cy="0" r="0.8" fill="{ink}"/><circle cx="{s}" cy="0" r="0.8" fill="{ink}"/><circle cx="0" cy="{s}" r="0.8" fill="{ink}"/><circle cx="{s}" cy="{s}" r="0.8" fill="{ink}"/>"#
        ),
    };
    Some(format!(
        r#"<pattern id="dx-pattern" patternUnits="userSpaceOnUse" x="{x}" y="{y}" width="{s}" height="{s}">{content}</pattern>"#
    ))
}

fn stroke_element(stroke: &Stroke) -> String {
    let outline = ductus_ink::outline(&stroke.points, &stroke.style);
    let mut el = format!(
        r#"<path d="{}" fill="{}""#,
        path_data(&outline),
        hex(stroke.style.color)
    );
    if stroke.style.color.a < 1.0 {
        write!(
            el,
            r#" fill-opacity="{}""#,
            num(f64::from(stroke.style.color.a), 3)
        )
        .unwrap();
    }
    let tool = match stroke.style.tool {
        Tool::Pen => "pen",
        Tool::Highlighter => "highlighter",
    };
    write!(
        el,
        r#" dx:tool="{tool}" dx:width="{}""#,
        num(stroke.style.width, 3)
    )
    .unwrap();
    if stroke.style.pattern != LinePattern::Solid {
        write!(el, r#" dx:line="{}""#, stroke.style.pattern.name()).unwrap();
    }
    let t0 = stroke.points.first().map_or(0, |p| p.t);
    let ink: Vec<String> = stroke
        .points
        .iter()
        .map(|p| {
            format!(
                "{},{},{},{}",
                num(p.x, 2),
                num(p.y, 2),
                num(f64::from(p.pressure), 3),
                p.t - t0
            )
        })
        .collect();
    write!(el, r#" dx:ink="{}""#, ink.join(" ")).unwrap();
    if stroke
        .points
        .iter()
        .any(|p| p.tilt_x != 0.0 || p.tilt_y != 0.0)
    {
        let tilt: Vec<String> = stroke
            .points
            .iter()
            .map(|p| {
                format!(
                    "{},{}",
                    num(f64::from(p.tilt_x), 2),
                    num(f64::from(p.tilt_y), 2)
                )
            })
            .collect();
        write!(el, r#" dx:tilt="{}""#, tilt.join(" ")).unwrap();
    }
    el.push_str("/>");
    el
}

/// The inline HTML text boxes use (`<u>`, spans with color and size) as XHTML; sizes in em.
fn text_tag(tag: &str) -> Option<String> {
    if matches!(tag, "<u>" | "</u>" | "</span>") {
        return Some(tag.to_owned());
    }
    let style = tag.strip_prefix("<span style=\"")?.strip_suffix("\">")?;
    let mut out = Vec::new();
    for part in style.split(';') {
        let (name, value) = part.split_once(':')?;
        match name.trim() {
            "color" => {
                let hex = value.trim().strip_prefix('#')?;
                if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    return None;
                }
                out.push(format!("color:#{hex}"));
            }
            "font-size" => {
                let px: f64 = value.trim().strip_suffix("px")?.parse().ok()?;
                out.push(format!("font-size:{}em", num(px / TextBox::FONT_SIZE, 3)));
            }
            _ => return None,
        }
    }
    Some(format!(r#"<span style="{}">"#, out.join(";")))
}

/// Other raw HTML stays text, keeping the XML well formed; open tags close with their paragraph.
fn markdown_html(markdown: &str) -> String {
    use pulldown_cmark::{CowStr, Event, Options, Parser, TagEnd, html};
    let mut open: Vec<&'static str> = Vec::new();
    let mut events = Vec::new();
    for event in Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH) {
        match event {
            Event::InlineHtml(t) => match text_tag(&t) {
                Some(tag) if tag.starts_with("</") => {
                    let name = if tag == "</u>" { "u" } else { "span" };
                    if open.last() == Some(&name) {
                        open.pop();
                        events.push(Event::InlineHtml(CowStr::from(tag)));
                    } else {
                        events.push(Event::Text(t));
                    }
                }
                Some(tag) => {
                    open.push(if tag == "<u>" { "u" } else { "span" });
                    events.push(Event::InlineHtml(CowStr::from(tag)));
                }
                None => events.push(Event::Text(t)),
            },
            Event::Html(t) => events.push(Event::Text(t)),
            e @ Event::End(
                TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item | TagEnd::TableCell,
            ) => {
                while let Some(name) = open.pop() {
                    events.push(Event::InlineHtml(CowStr::from(format!("</{name}>"))));
                }
                events.push(e);
            }
            e => events.push(e),
        }
    }
    let parser = events.into_iter();
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

fn math_element(math: &MathBox) -> String {
    let mut el = format!(
        r#"<g dx:math="{}" dx:src="{}" dx:x="{}" dx:y="{}" dx:width="{}" dx:height="{}" dx:scale="{}" fill="{}">"#,
        escape_attr(&math.syntax),
        escape_attr(&math.source).replace('\n', "&#10;"),
        num(math.x, 2),
        num(math.y, 2),
        num(math.width, 2),
        num(math.height, 2),
        num(math.scale, 3),
        hex(math.color)
    );
    for (path, _) in ductus_math::outlines(math) {
        write!(el, r#"<path d="{}"/>"#, path_data(&path)).unwrap();
    }
    el.push_str("</g>");
    el
}

fn text_element(text: &TextBox) -> String {
    format!(
        r#"<foreignObject x="{}" y="{}" width="{}" height="{}" dx:text="{}" dx:scale="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="font-family:sans-serif;font-size:{}px;line-height:1.3">{}</div></foreignObject>"#,
        num(text.x, 2),
        num(text.y, 2),
        num(text.width, 2),
        num(text.height, 2),
        escape_attr(&text.markdown).replace('\n', "&#10;"),
        num(text.scale, 3),
        num(TextBox::FONT_SIZE * text.scale, 2),
        markdown_html(&text.markdown).trim_end()
    )
}

pub fn write_page(page: &PageFile) -> String {
    write_page_in(page, "")
}

pub fn write_page_in(page: &PageFile, assets_dir: &str) -> String {
    let doc = &page.document;
    let settings = doc.settings();
    let area = canvas_rect(doc);
    let bg = &settings.background;

    let mut svg = String::with_capacity(4096 + doc.strokes().len() * 1024);
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write!(
        svg,
        r#"<svg xmlns="{SVG_NS}" xmlns:dx="{DX_NS}" dx:version="{FORMAT_VERSION}" dx:id="{}""#,
        page.id
    )
    .unwrap();
    match settings.kind {
        PageKind::Endless => svg.push_str(r#" dx:kind="endless""#),
        PageKind::Paper { size, orientation } => write!(
            svg,
            r#" dx:kind="paper" dx:paper="{}" dx:orientation="{}""#,
            size.name(),
            match orientation {
                Orientation::Portrait => "portrait",
                Orientation::Landscape => "landscape",
            }
        )
        .unwrap(),
        PageKind::Pdf => {
            let sizes: Vec<String> = doc
                .pdf_sheets()
                .iter()
                .map(|(w, h)| format!("{},{}", num(*w, 2), num(*h, 2)))
                .collect();
            write!(
                svg,
                r#" dx:kind="pdf" dx:pdf="{}" dx:sheets="{}""#,
                escape_attr(page.pdf.as_deref().unwrap_or("source.pdf")),
                sizes.join(" ")
            )
            .unwrap();
        }
    }
    write!(
        svg,
        r#" dx:pattern="{}" dx:spacing="{}" dx:paper-color="{}" dx:pattern-color="{}""#,
        bg.pattern.name(),
        num(bg.spacing, 3),
        hex(bg.paper),
        hex(bg.ink)
    )
    .unwrap();
    // Physical size for paper pages, so viewers and printers use real dimensions.
    let (w, h) = (area.width(), area.height());
    if !sheet_rects(doc).is_empty() {
        write!(
            svg,
            r#" width="{}mm" height="{}mm""#,
            num(w / UNITS_PER_MM, 2),
            num(h / UNITS_PER_MM, 2)
        )
        .unwrap();
    } else {
        write!(svg, r#" width="{}" height="{}""#, num(w, 2), num(h, 2)).unwrap();
    }
    writeln!(
        svg,
        r#" viewBox="{} {} {} {}">"#,
        num(area.x0, 2),
        num(area.y0, 2),
        num(w, 2),
        num(h, 2)
    )
    .unwrap();

    let mut sheets = sheet_rects(doc);
    if sheets.is_empty() {
        sheets.push(area);
    }
    // PDF pages show the PDF instead of a pattern.
    let patterned = bg.pattern != PatternStyle::Blank && settings.kind != PageKind::Pdf;
    let pattern_origin = kurbo::Point::ZERO;
    if let Some(def) = pattern_def(bg, pattern_origin).filter(|_| patterned) {
        writeln!(svg, "<defs>{def}</defs>").unwrap();
    }
    svg.push_str(r#"<g dx:layer="paper">"#);
    for r in &sheets {
        let geom = format!(
            r#"x="{}" y="{}" width="{}" height="{}""#,
            num(r.x0, 2),
            num(r.y0, 2),
            num(r.width(), 2),
            num(r.height(), 2)
        );
        write!(svg, r#"<rect {geom} fill="{}"/>"#, hex(bg.paper)).unwrap();
        match settings.ruled_margins().filter(|_| patterned) {
            Some((top, left)) => {
                // Lines below the heading room, and the margin line.
                write!(
                    svg,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="url(#dx-pattern)"/>"#,
                    num(r.x0, 2),
                    num(r.y0 + top, 2),
                    num(r.width(), 2),
                    num((r.height() - top).max(0.0), 2)
                )
                .unwrap();
                write!(
                    svg,
                    r#"<path d="M{},{}V{}" stroke="{}" stroke-width="0.8"/>"#,
                    num(r.x0 + left, 2),
                    num(r.y0, 2),
                    num(r.y1, 2),
                    hex(ductus_core::MARGIN_LINE)
                )
                .unwrap();
            }
            None if patterned => {
                write!(svg, r#"<rect {geom} fill="url(#dx-pattern)"/>"#).unwrap();
            }
            None => {}
        }
    }
    svg.push_str("</g>\n");

    if !doc.images().is_empty() {
        svg.push_str(r#"<g dx:layer="images">"#);
        for image in doc.images() {
            let href = if assets_dir.is_empty() {
                image.source.clone()
            } else {
                format!("{assets_dir}/{}", image.source)
            };
            write!(
                svg,
                r#"<image href="{}" dx:src="{}" x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none"/>"#,
                escape_attr(&href),
                escape_attr(&image.source),
                num(image.x, 2),
                num(image.y, 2),
                num(image.width, 2),
                num(image.height, 2)
            )
            .unwrap();
        }
        svg.push_str("</g>\n");
    }

    if !doc.maths().is_empty() {
        svg.push_str(r#"<g dx:layer="math">"#);
        for math in doc.maths() {
            svg.push_str(&math_element(math));
        }
        svg.push_str("</g>\n");
    }

    // Highlights beneath ink, as the app draws them.
    for (layer, tool) in [("highlight", Tool::Highlighter), ("ink", Tool::Pen)] {
        writeln!(svg, r#"<g dx:layer="{layer}">"#).unwrap();
        for stroke in doc.strokes().iter().filter(|s| s.style.tool == tool) {
            svg.push_str(&stroke_element(stroke));
            svg.push('\n');
        }
        svg.push_str("</g>\n");
    }
    // Recognized handwriting as invisible text over the ink, like OCR text in scans.
    if !page.recognized.is_empty() {
        svg.push_str(r#"<g dx:layer="recognized" fill-opacity="0" font-family="sans-serif">"#);
        for line in &page.recognized {
            let (x, y, w, h) = line.rect;
            write!(
                svg,
                r#"<text x="{}" y="{}" font-size="{}" textLength="{}" lengthAdjust="spacingAndGlyphs" dx:key="{}" dx:x="{}" dx:y="{}" dx:w="{}" dx:h="{}""#,
                num(x, 2),
                num(y + h * 0.8, 2),
                num((h * 0.8).max(1.0), 2),
                num(w.max(1.0), 2),
                escape_attr(&line.key),
                num(x, 2),
                num(y, 2),
                num(w, 2),
                num(h, 2)
            )
            .unwrap();
            if let Some(lang) = &line.lang {
                write!(svg, r#" dx:lang="{}""#, escape_attr(lang)).unwrap();
            }
            write!(
                svg,
                ">{}</text>",
                escape_attr(&line.text).replace('>', "&gt;")
            )
            .unwrap();
        }
        svg.push_str("</g>\n");
    }
    if !doc.texts().is_empty() {
        svg.push_str(r#"<g dx:layer="text">"#);
        for text in doc.texts() {
            svg.push_str(&text_element(text));
        }
        svg.push_str("</g>\n");
    }
    svg.push_str("</svg>\n");
    svg
}

fn parse_color(s: &str) -> Option<Rgba> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let c = |i: usize| {
        u8::from_str_radix(&s[i..i + 2], 16)
            .ok()
            .map(|v| f32::from(v) / 255.0)
    };
    Some(Rgba::rgb(c(0)?, c(2)?, c(4)?))
}

fn parse_ink(ink: &str, tilt: Option<&str>) -> Result<Vec<InkPoint>, Error> {
    let tilts: Vec<(f32, f32)> = match tilt {
        Some(t) => t
            .split_whitespace()
            .map(|pair| {
                let mut it = pair.split(',').map(str::parse::<f32>);
                match (it.next(), it.next()) {
                    (Some(Ok(a)), Some(Ok(b))) => Ok((a, b)),
                    _ => Err(Error::Invalid(format!("bad tilt sample {pair:?}"))),
                }
            })
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    ink.split_whitespace()
        .enumerate()
        .map(|(i, sample)| {
            let v: Vec<&str> = sample.split(',').collect();
            let bad = || Error::Invalid(format!("bad ink sample {sample:?}"));
            if v.len() != 4 {
                return Err(bad());
            }
            let (tilt_x, tilt_y) = tilts.get(i).copied().unwrap_or((0.0, 0.0));
            Ok(InkPoint {
                x: v[0].parse().map_err(|_| bad())?,
                y: v[1].parse().map_err(|_| bad())?,
                pressure: v[2].parse().map_err(|_| bad())?,
                t: v[3].parse().map_err(|_| bad())?,
                tilt_x,
                tilt_y,
            })
        })
        .collect()
}

/// Elements without Ductus data are ignored (and dropped on the next save).
pub fn read_page(svg: &str) -> Result<PageFile, Error> {
    let mut reader = NsReader::from_str(svg);
    let mut id = None;
    let mut kind_name = String::from("endless");
    let (mut paper, mut orientation) = (PaperSize::A4, Orientation::Portrait);
    let mut background = PageBackground::default();
    let mut strokes = Vec::new();
    let mut images = Vec::new();
    let mut texts = Vec::new();
    let mut maths = Vec::new();
    let mut recognized = Vec::new();
    let mut saw_root = false;
    let mut pdf = None;
    let mut pdf_sheets = Vec::new();

    loop {
        let event = reader.read_event()?;
        let element = match &event {
            Event::Start(e) | Event::Empty(e) => e.clone(),
            Event::Eof => break,
            _ => continue,
        };
        let (ns, local) = reader.resolver().resolve_element(element.name());
        if !matches!(ns, ResolveResult::Bound(Namespace(n)) if n == SVG_NS) {
            continue;
        }
        let mut dx = std::collections::HashMap::new();
        for attr in element.attributes() {
            let attr = attr.map_err(quick_xml::Error::from)?;
            let (ans, alocal) = reader.resolver().resolve_attribute(attr.key);
            let value = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)?
                .into_owned();
            let key = alocal.as_ref().to_owned();
            match ans {
                ResolveResult::Bound(Namespace(n)) if n == DX_NS => {
                    dx.insert(key, value);
                }
                ResolveResult::Unbound => {
                    dx.entry(format!("svg:{key}")).or_insert(value);
                }
                _ => {}
            }
        }

        match local.as_ref() {
            "svg" if !saw_root => {
                saw_root = true;
                let version: u32 = dx.get("version").and_then(|v| v.parse().ok()).unwrap_or(0);
                if version == 0 {
                    return Err(Error::Invalid("not a Ductus page (no dx:version)".into()));
                }
                if version > FORMAT_VERSION {
                    return Err(Error::Invalid(format!(
                        "page format {version} is newer than this app ({FORMAT_VERSION})"
                    )));
                }
                id = dx.get("id").cloned();
                if let Some(k) = dx.get("kind") {
                    kind_name.clone_from(k);
                }
                if let Some(size) = dx.get("paper").and_then(|p| PaperSize::from_name(p)) {
                    paper = size;
                }
                if dx.get("orientation").map(String::as_str) == Some("landscape") {
                    orientation = Orientation::Landscape;
                }
                if let Some(p) = dx.get("pattern").and_then(|p| PatternStyle::from_name(p)) {
                    background.pattern = p;
                }
                if let Some(s) = dx.get("spacing").and_then(|s| s.parse().ok()) {
                    background.spacing = s;
                }
                if let Some(c) = dx.get("paper-color").and_then(|c| parse_color(c)) {
                    background.paper = c;
                }
                if let Some(c) = dx.get("pattern-color").and_then(|c| parse_color(c)) {
                    background.ink = c;
                }
                pdf = dx.get("pdf").cloned();
                pdf_sheets = dx
                    .get("sheets")
                    .map(|s| {
                        s.split_whitespace()
                            .filter_map(|pair| {
                                let (w, h) = pair.split_once(',')?;
                                Some((w.parse().ok()?, h.parse().ok()?))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            "path" => {
                let Some(ink) = dx.get("ink") else { continue };
                let tool = match dx.get("tool").map(String::as_str) {
                    Some("highlighter") => Tool::Highlighter,
                    _ => Tool::Pen,
                };
                let mut color = dx
                    .get("svg:fill")
                    .and_then(|c| parse_color(c))
                    .unwrap_or(Rgba::BLACK);
                if let Some(a) = dx.get("svg:fill-opacity").and_then(|a| a.parse().ok()) {
                    color.a = a;
                }
                let width = dx.get("width").and_then(|w| w.parse().ok()).unwrap_or(3.0);
                let points = parse_ink(ink, dx.get("tilt").map(String::as_str))?;
                if points.is_empty() {
                    continue;
                }
                strokes.push(Stroke {
                    id: StrokeId(0),
                    style: StrokeStyle {
                        tool,
                        color,
                        width,
                        pattern: dx
                            .get("line")
                            .and_then(|l| LinePattern::from_name(l))
                            .unwrap_or_default(),
                    },
                    points,
                });
            }
            "text" => {
                let Some(key) = dx.get("key").cloned() else {
                    continue;
                };
                if !matches!(event, Event::Start(_)) {
                    continue;
                }
                let raw = reader.read_text(element.name())?;
                let text = quick_xml::escape::unescape(&raw)?.into_owned();
                let number = |k: &str| dx.get(k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                recognized.push(RecognizedLine {
                    key,
                    text,
                    rect: (number("x"), number("y"), number("w"), number("h")),
                    lang: dx.get("lang").cloned(),
                });
            }
            "g" => {
                let Some(source) = dx.get("src") else {
                    continue;
                };
                let Some(syntax) = dx.get("math") else {
                    continue;
                };
                let number = |key: &str| dx.get(key).and_then(|v| v.parse::<f64>().ok());
                maths.push(MathBox {
                    id: StrokeId(0),
                    source: source.clone(),
                    syntax: syntax.clone(),
                    x: number("x").unwrap_or(0.0),
                    y: number("y").unwrap_or(0.0),
                    width: number("width").unwrap_or(0.0),
                    height: number("height").unwrap_or(0.0),
                    scale: number("scale").filter(|s| *s > 0.0).unwrap_or(1.0),
                    color: dx
                        .get("svg:fill")
                        .and_then(|c| parse_color(c))
                        .unwrap_or(Rgba::BLACK),
                });
            }
            "foreignObject" => {
                let Some(markdown) = dx.get("text") else {
                    continue;
                };
                let number = |key: &str| dx.get(key).and_then(|v| v.parse::<f64>().ok());
                if let (Some(x), Some(y), Some(width), Some(height)) = (
                    number("svg:x"),
                    number("svg:y"),
                    number("svg:width"),
                    number("svg:height"),
                ) {
                    texts.push(TextBox {
                        id: StrokeId(0),
                        markdown: markdown.clone(),
                        x,
                        y,
                        width,
                        height,
                        scale: number("scale").filter(|s| *s > 0.0).unwrap_or(1.0),
                    });
                }
            }
            "image" => {
                // Only the file name: the folder part goes stale when another program renames the page.
                let source = dx.get("src").cloned().or_else(|| {
                    dx.get("svg:href")
                        .map(|h| h.rsplit('/').next().unwrap_or(h).to_owned())
                });
                let number = |key: &str| dx.get(key).and_then(|v| v.parse::<f64>().ok());
                if let (Some(source), Some(x), Some(y), Some(width), Some(height)) = (
                    source,
                    number("svg:x"),
                    number("svg:y"),
                    number("svg:width"),
                    number("svg:height"),
                ) {
                    images.push(Image {
                        id: StrokeId(0),
                        source,
                        x,
                        y,
                        width,
                        height,
                    });
                }
            }
            _ => {}
        }
    }
    if !saw_root {
        return Err(Error::Invalid("no <svg> root element".into()));
    }
    let kind = match kind_name.as_str() {
        "paper" => PageKind::Paper {
            size: paper,
            orientation,
        },
        "pdf" => PageKind::Pdf,
        _ => PageKind::Endless,
    };
    let mut document = Document::with_contents(PageSettings { kind, background }, strokes)
        .with_images(images)
        .with_texts(texts)
        .with_maths(maths);
    document.set_pdf_sheets(pdf_sheets);
    Ok(PageFile {
        id: id.unwrap_or_else(crate::new_id),
        document,
        pdf: pdf.filter(|_| kind == PageKind::Pdf),
        recognized,
    })
}

#[cfg(test)]
mod tests;
