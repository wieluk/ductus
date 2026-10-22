use crate::Error;
use ductus_core::{
    Document, PageKind, PatternStyle, Rgba, SHEET_GAP, Tool, UNITS_PER_MM, UNITS_PER_POINT,
};
use kurbo::{BezPath, PathEl, Point, Rect, Shape};
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Object, Stream, dictionary};

const A4: (f64, f64) = (210.0 * UNITS_PER_MM, 297.0 * UNITS_PER_MM);

pub fn page_areas(doc: &Document) -> Vec<Rect> {
    let sheets = doc.sheets(0);
    if !sheets.is_empty() && doc.settings().kind != PageKind::Endless {
        return sheets
            .into_iter()
            .map(|(x, y, w, h)| Rect::new(x, y, x + w, y + h))
            .collect();
    }
    let Some(content) = doc
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
        .reduce(|a, b| a.union(b))
    else {
        return vec![Rect::new(0.0, 0.0, A4.0, A4.1)];
    };
    let content = content.inflate(24.0, 24.0);
    let width = content.width().max(A4.0 / 2.0);
    let height = width * A4.1 / A4.0;
    let count = (content.height() / height).ceil().max(1.0) as usize;
    (0..count)
        .map(|i| {
            let y = content.y0 + i as f64 * height;
            Rect::new(content.x0, y, content.x0 + width, y + height)
        })
        .collect()
}

fn n(v: f64) -> Object {
    Object::Real(v as f32)
}

fn color(c: Rgba) -> Vec<Object> {
    vec![n(f64::from(c.r)), n(f64::from(c.g)), n(f64::from(c.b))]
}

fn path_ops(path: &BezPath, area: Rect, scale: f64, ops: &mut Vec<Operation>) {
    let map = |p: Point| {
        let x = (p.x - area.x0) * scale;
        let y = (area.y1 - p.y) * scale;
        (n(x), n(y))
    };
    let mut current = Point::ZERO;
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(a) => {
                let (x, y) = map(a);
                ops.push(Operation::new("m", vec![x, y]));
                current = a;
            }
            PathEl::LineTo(a) => {
                let (x, y) = map(a);
                ops.push(Operation::new("l", vec![x, y]));
                current = a;
            }
            PathEl::QuadTo(q, a) => {
                let c1 = current + (q - current) * (2.0 / 3.0);
                let c2 = a + (q - a) * (2.0 / 3.0);
                let ((x1, y1), (x2, y2), (x, y)) = (map(c1), map(c2), map(a));
                ops.push(Operation::new("c", vec![x1, y1, x2, y2, x, y]));
                current = a;
            }
            PathEl::CurveTo(c1, c2, a) => {
                let ((x1, y1), (x2, y2), (x, y)) = (map(c1), map(c2), map(a));
                ops.push(Operation::new("c", vec![x1, y1, x2, y2, x, y]));
                current = a;
            }
            PathEl::ClosePath => ops.push(Operation::new("h", vec![])),
        }
    }
}

fn pattern_ops(doc: &Document, area: Rect, scale: f64, ops: &mut Vec<Operation>) {
    let bg = &doc.settings().background;
    if bg.pattern == PatternStyle::Blank || bg.spacing <= 0.0 {
        return;
    }
    ops.push(Operation::new("rg", color(bg.ink)));
    ops.push(Operation::new("RG", color(bg.ink)));
    ops.push(Operation::new("w", vec![n(0.4)]));
    // Paper patterns start at the sheet edge; endless ones at the origin.
    let origin = if doc.settings().kind == PageKind::Endless {
        Point::ZERO
    } else {
        area.origin()
    };
    let first = |lo: f64, o: f64| ((lo - o) / bg.spacing).floor() as i64 + 1;
    let last = |hi: f64, o: f64| ((hi - o) / bg.spacing).ceil() as i64 - 1;
    let ys: Vec<f64> = (first(area.y0, origin.y)..=last(area.y1, origin.y))
        .map(|k| origin.y + k as f64 * bg.spacing)
        .collect();
    let xs: Vec<f64> = (first(area.x0, origin.x)..=last(area.x1, origin.x))
        .map(|k| origin.x + k as f64 * bg.spacing)
        .collect();
    let px = |x: f64| n((x - area.x0) * scale);
    let py = |y: f64| n((area.y1 - y) * scale);
    match bg.pattern {
        PatternStyle::Lined | PatternStyle::Grid => {
            for &y in &ys {
                ops.push(Operation::new("m", vec![px(area.x0), py(y)]));
                ops.push(Operation::new("l", vec![px(area.x1), py(y)]));
            }
            if bg.pattern == PatternStyle::Grid {
                for &x in &xs {
                    ops.push(Operation::new("m", vec![px(x), py(area.y0)]));
                    ops.push(Operation::new("l", vec![px(x), py(area.y1)]));
                }
            }
            ops.push(Operation::new("S", vec![]));
        }
        PatternStyle::Dotted => {
            let r = 0.6;
            for &y in &ys {
                for &x in &xs {
                    let (cx, cy) = ((x - area.x0) * scale, (area.y1 - y) * scale);
                    ops.push(Operation::new(
                        "re",
                        vec![n(cx - r), n(cy - r), n(2.0 * r), n(2.0 * r)],
                    ));
                }
            }
            ops.push(Operation::new("f", vec![]));
        }
        PatternStyle::Blank => {}
    }
}

fn math_ops(doc: &Document, area: Rect, scale: f64, ops: &mut Vec<Operation>) {
    for math in doc.maths() {
        let r = Rect::new(math.x, math.y, math.x + math.width, math.y + math.height);
        if !r.overlaps(area) {
            continue;
        }
        for (path, fill) in ductus_math::outlines(math) {
            ops.push(Operation::new("rg", color(fill)));
            path_ops(&path, area, scale, ops);
            ops.push(Operation::new("f", vec![]));
        }
    }
}

pub fn math_overlay(doc: &Document, areas: &[Rect]) -> Result<Option<Vec<u8>>, Error> {
    if doc.maths().is_empty() || areas.is_empty() {
        return Ok(None);
    }
    let mut pdf = lopdf::Document::with_version("1.7");
    let pages_id = pdf.new_object_id();
    let scale = 1.0 / UNITS_PER_POINT;
    let mut kids = Vec::new();
    for &area in areas {
        let mut ops = Vec::new();
        math_ops(doc, area, scale, &mut ops);
        let content = Content { operations: ops }.encode()?;
        let content_id = pdf.add_object(Stream::new(Dictionary::new(), content));
        let page = pdf.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![n(0.0), n(0.0), n(area.width() * scale), n(area.height() * scale)],
            "Contents" => content_id,
            "Resources" => Dictionary::new(),
        });
        kids.push(Object::Reference(page));
    }
    let count = kids.len() as i64;
    pdf.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    pdf.trailer.set("Root", catalog);
    let mut out = Vec::new();
    pdf.save_to(&mut out)?;
    Ok(Some(out))
}

pub fn export_notes(
    doc: &Document,
    image: &dyn Fn(&str) -> Option<crate::PdfImage>,
) -> Result<Vec<u8>, Error> {
    let mut pdf = lopdf::Document::with_version("1.7");
    let pages_id = pdf.new_object_id();
    let scale = 1.0 / UNITS_PER_POINT;
    let alpha_states = dictionary! {
        "GSink" => dictionary! { "Type" => "ExtGState", "ca" => 1.0, "CA" => 1.0 },
        "GSmark" => dictionary! { "Type" => "ExtGState", "ca" => 0.4, "CA" => 0.4, "BM" => "Multiply" },
    };
    let alpha_states = pdf.add_object(alpha_states);

    let mut kids = Vec::new();
    for area in page_areas(doc) {
        let mut ops = vec![
            Operation::new("rg", color(doc.settings().background.paper)),
            Operation::new(
                "re",
                vec![
                    n(0.0),
                    n(0.0),
                    n(area.width() * scale),
                    n(area.height() * scale),
                ],
            ),
            Operation::new("f", vec![]),
        ];
        pattern_ops(doc, area, scale, &mut ops);
        let xobjects =
            crate::pictures::notes_page_pictures(&mut pdf, doc, area, scale, image, &mut ops);
        math_ops(doc, area, scale, &mut ops);
        // Highlights beneath ink, as on screen.
        for tool in [Tool::Highlighter, Tool::Pen] {
            for stroke in doc.strokes().iter().filter(|s| s.style.tool == tool) {
                let outline = ductus_ink::outline(&stroke.points, &stroke.style);
                if !outline.bounding_box().overlaps(area) {
                    continue;
                }
                let state = if tool == Tool::Pen { "GSink" } else { "GSmark" };
                ops.push(Operation::new("q", vec![]));
                ops.push(Operation::new(
                    "gs",
                    vec![Object::Name(state.as_bytes().to_vec())],
                ));
                ops.push(Operation::new("rg", color(stroke.style.color)));
                path_ops(&outline, area, scale, &mut ops);
                ops.push(Operation::new("f", vec![]));
                ops.push(Operation::new("Q", vec![]));
            }
        }
        let content = Content { operations: ops }.encode()?;
        let content_id = pdf.add_object(Stream::new(Dictionary::new(), content));
        let page = pdf.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![n(0.0), n(0.0), n(area.width() * scale), n(area.height() * scale)],
            "Contents" => content_id,
            "Resources" => dictionary! { "ExtGState" => alpha_states, "XObject" => xobjects },
        });
        kids.push(Object::Reference(page));
    }
    let count = kids.len() as i64;
    pdf.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    pdf.trailer.set("Root", catalog);
    let mut out = Vec::new();
    pdf.save_to(&mut out)?;
    Ok(out)
}

/// Each stroke belongs to the sheet holding its first point.
pub fn ink_by_page(doc: &Document) -> Vec<crate::PageInk> {
    let sheets = ductus_core::stack_sheets(doc.pdf_sheets());
    doc.strokes()
        .iter()
        .filter_map(|stroke| {
            let first = stroke.points.first()?;
            let index = sheets
                .iter()
                .position(|&(_, y, _, h)| first.y < y + h + SHEET_GAP / 2.0)
                .unwrap_or(sheets.len().checked_sub(1)?);
            let (x0, y0, _, _) = sheets[index];
            let mut local = stroke.clone();
            for p in &mut local.points {
                p.x -= x0;
                p.y -= y0;
            }
            Some(crate::PageInk {
                page: index,
                stroke: local,
            })
        })
        .collect()
}

pub fn document_strokes(
    sizes: &[(f64, f64)],
    ink: Vec<crate::PageInk>,
) -> Vec<ductus_core::Stroke> {
    let sheets = ductus_core::stack_sheets(sizes);
    ink.into_iter()
        .filter_map(|mut item| {
            let &(x0, y0, _, _) = sheets.get(item.page)?;
            for p in &mut item.stroke.points {
                p.x += x0;
                p.y += y0;
            }
            Some(item.stroke)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::{
        InkPoint, Orientation, PageSettings, PaperSize, Stroke, StrokeId, StrokeStyle,
    };

    fn stroke(y: f64) -> Stroke {
        Stroke {
            id: StrokeId(0),
            style: StrokeStyle::default(),
            points: (0..10)
                .map(|i| InkPoint {
                    x: 100.0 + f64::from(i) * 10.0,
                    y,
                    pressure: 0.7,
                    tilt_x: 0.0,
                    tilt_y: 0.0,
                    t: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn paper_pages_export_one_pdf_page_per_sheet() {
        let a4 = PageSettings {
            kind: PageKind::Paper {
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            },
            ..PageSettings::default()
        };
        let sheet = 297.0 * UNITS_PER_MM + SHEET_GAP;
        let doc = Document::with_contents(a4, [stroke(100.0), stroke(sheet + 100.0)]);
        let pdf = export_notes(&doc, &|_| None).unwrap();
        let back = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(back.get_pages().len(), 2);
    }

    #[test]
    fn ink_moves_between_document_and_page_coordinates() {
        let sizes = vec![(800.0, 600.0), (800.0, 600.0)];
        let mut doc = Document::with_contents(
            PageSettings {
                kind: PageKind::Pdf,
                ..PageSettings::default()
            },
            [stroke(50.0), stroke(600.0 + SHEET_GAP + 50.0)],
        );
        doc.set_pdf_sheets(sizes.clone());
        let ink = ink_by_page(&doc);
        assert_eq!(ink.iter().map(|i| i.page).collect::<Vec<_>>(), [0, 1]);
        assert!((ink[1].stroke.points[0].y - 50.0).abs() < 1e-9);
        let back = document_strokes(&sizes, ink);
        assert!((back[1].points[0].y - (600.0 + SHEET_GAP + 50.0)).abs() < 1e-9);
    }

    #[test]
    fn exported_ink_renders_where_it_was_written() {
        if crate::engine().is_err() {
            assert!(
                std::env::var_os("DUCTUS_PDFIUM").is_none(),
                "pdfium expected"
            );
            return;
        }
        let a5 = PageSettings {
            kind: PageKind::Paper {
                size: PaperSize::A5,
                orientation: Orientation::Portrait,
            },
            background: ductus_core::PageBackground {
                pattern: PatternStyle::Lined,
                spacing: 30.0,
                ..ductus_core::PageBackground::default()
            },
        };
        let doc = Document::with_contents(a5, [stroke(100.0)]);
        let file = crate::PdfFile::open(export_notes(&doc, &|_| None).unwrap()).unwrap();
        let (w, h) = file.page_sizes()[0];
        let mut pixmap = tiny_skia::Pixmap::new(w.ceil() as u32, h.ceil() as u32).unwrap();
        file.render_into(0, &mut pixmap, (0.0, 0.0), 1.0).unwrap();
        if let Ok(dir) = std::env::var("DUCTUS_SHOTS") {
            pixmap
                .save_png(format!("{dir}/pdf-export-notes.png"))
                .unwrap();
        }
        let ink = pixmap.pixel(150, 100).unwrap().demultiply();
        assert!(ink.red() < 80, "ink at (150, 100)");
        let line = pixmap.pixel(50, 90).unwrap().demultiply();
        assert!(line.red() < 235, "ruled line at y = 90");
    }
}
