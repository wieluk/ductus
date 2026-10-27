use super::*;
use ductus_core::{Orientation, PaperSize};
use serde_json::json;
use std::io::Write as _;

fn rnote_file(snapshot: Value) -> tempfile::NamedTempFile {
    let root = json!({ "version": "0.13.1", "data": { "engine_snapshot": snapshot } });
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&serde_json::to_vec(&root).unwrap()).unwrap();
    let mut file = tempfile::Builder::new()
        .suffix(".rnote")
        .tempfile()
        .unwrap();
    file.write_all(&enc.finish().unwrap()).unwrap();
    file
}

fn slot(value: Value) -> Value {
    json!({ "value": value, "version": 1 })
}

fn page_of(import: Import) -> ImportedPage {
    match import.entry {
        Entry::Page(page) => page,
        _ => panic!("expected a page"),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn color(r: f64, g: f64, b: f64, a: f64) -> Value {
    json!({ "r": r, "g": g, "b": b, "a": a })
}

#[test]
fn a4_document_keeps_pages_ink_shapes_and_text() {
    // A4 at 72 dpi: 595 x 842 px, so one Rnote pixel is 4/3 Ductus units.
    let pen = json!({ "brushstroke": {
        "path": {
            "start": { "pos": [10.0, 20.0], "pressure": 1.0 },
            "segments": [{ "lineto": { "end": { "pos": [30.0, 40.0], "pressure": 0.25 } } }]
        },
        "style": { "smooth": {
            "stroke_width": 3.0, "stroke_color": color(1.0, 0.0, 0.0, 1.0), "fill_color": null,
            "pressure_curve": "linear", "line_style": "dashed_wide", "line_cap": "rounded"
        } }
    } });
    let marker = json!({ "brushstroke": {
        "path": {
            "start": { "pos": [10.0, 900.0], "pressure": 0.5 },
            "segments": [{ "lineto": { "end": { "pos": [60.0, 900.0], "pressure": 0.5 } } }]
        },
        "style": { "smooth": { "stroke_width": 12.0, "stroke_color": color(1.0, 1.0, 0.0, 0.5) } }
    } });
    let rect = json!({ "shapestroke": {
        "shape": { "rect": {
            "cuboid": { "half_extents": [10.0, 5.0] },
            "affine": [1.0, 0.0, 0.0, 1.0, 100.0, 100.0]
        } },
        "style": { "rough": { "stroke_width": 2.0, "stroke_color": color(0.0, 0.0, 0.0, 1.0) } }
    } });
    let filled = json!({ "shapestroke": {
        "shape": { "line": { "start": [0.0, 0.0], "end": [1.0, 1.0] } },
        "style": { "smooth": { "stroke_width": 2.0, "stroke_color": null } }
    } });
    let text = json!({ "textstroke": {
        "text": "Hello *world*",
        "affine": [1.0, 0.0, 0.0, 1.0, 50.0, 60.0],
        "text_style": { "font_size": 12.0, "max_width": null }
    } });
    let snapshot = json!({
        "document": {
            "config": {
                "format": { "width": 595.0, "height": 842.0, "dpi": 72.0, "orientation": "portrait" },
                "background": {
                    "color": color(1.0, 1.0, 1.0, 1.0), "pattern": "lines",
                    "pattern_size": [32.0, 24.0], "pattern_color": color(0.5, 0.5, 0.5, 1.0)
                },
                "layout": "fixed_size"
            },
            "x": 0.0, "y": 0.0, "width": 595.0, "height": 1684.0
        },
        "stroke_components": [
            slot(marker), slot(pen), slot(Value::Null), slot(rect), slot(filled), slot(text)
        ],
        "chrono_components": [
            slot(json!({ "t": 5, "layer": "highlighter" })),
            slot(json!({ "t": 1, "layer": "user_layer" }))
        ],
    });
    let file = rnote_file(snapshot);
    let import = read(file.path()).unwrap();
    assert_eq!(import.warnings.len(), 1, "the fill-only shape is left out");
    let page = page_of(import);
    let doc = &page.document;
    assert_eq!(
        doc.settings().kind,
        PageKind::Paper {
            size: PaperSize::A4,
            orientation: Orientation::Portrait
        }
    );
    let bg = doc.settings().background;
    assert_eq!(bg.pattern, PatternStyle::Lined);
    assert_eq!(bg.spacing, 24.0 * 4.0 / 3.0);

    // Ordered by when they were drawn: the pen came first.
    let [pen, rect, marker] = doc.strokes() else {
        panic!("{:?}", doc.strokes().len())
    };
    assert_eq!(pen.style.tool, Tool::Pen);
    assert_eq!(pen.style.width, 4.0);
    assert_eq!(pen.style.pattern, LinePattern::Dashed);
    assert!(close(pen.points[0].x, 10.0 * 4.0 / 3.0));
    assert_eq!(pen.points[0].pressure, 1.0);
    assert_eq!(pen.points.last().unwrap().pressure, 0.0);

    // Corners stay corners: the sides are filled with points.
    assert!(rect.points.len() > 40);
    let corner = |x: f64, y: f64| {
        rect.points
            .iter()
            .any(|p| close(p.x, x * 4.0 / 3.0) && close(p.y, y * 4.0 / 3.0))
    };
    assert!(corner(110.0, 95.0) && corner(110.0, 105.0) && corner(90.0, 105.0));
    assert!(close(rect.points[0].x, 90.0 * 4.0 / 3.0) && close(rect.points[0].y, 95.0 * 4.0 / 3.0));

    // On the second sheet, 58 px below its top.
    assert_eq!(marker.style.tool, Tool::Highlighter);
    assert_eq!(marker.style.color.a, 1.0);
    let second_top = doc.sheets(0)[1].1;
    assert!(close(marker.points[0].y, second_top + 58.0 * 4.0 / 3.0));

    let text = &doc.texts()[0];
    assert_eq!(text.markdown, "Hello \\*world\\*");
    assert!(close(text.x, 50.0 * 4.0 / 3.0) && close(text.y, 60.0 * 4.0 / 3.0));
}

#[test]
fn infinite_canvas_becomes_endless_and_pictures_come_along() {
    let pixels =
        base64::engine::general_purpose::STANDARD.encode([255u8, 0, 0, 255, 0, 0, 128, 128]);
    let bitmap = json!({ "bitmapimage": {
        "image": { "data": pixels, "pixel_width": 2, "pixel_height": 1, "memory_format": "r8g8b8a8_premultiplied" },
        "rectangle": {
            "cuboid": { "half_extents": [20.0, 10.0] },
            "affine": [1.0, 0.0, 0.0, 1.0, -100.0, -50.0]
        }
    } });
    let svg = json!({ "vectorimage": {
        "svg_data": r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="blue"/></svg>"#,
        "intrinsic_size": [10.0, 10.0],
        "rectangle": {
            "cuboid": { "half_extents": [5.0, 5.0] },
            "affine": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
        }
    } });
    let snapshot = json!({
        "document": { "config": { "format": { "dpi": 96.0 }, "layout": "infinite" } },
        "stroke_components": [slot(bitmap), slot(svg)],
        "chrono_components": []
    });
    let file = rnote_file(snapshot);
    let page = page_of(read(file.path()).unwrap());
    assert_eq!(page.document.settings().kind, PageKind::Endless);
    let images = page.document.images();
    assert_eq!(images.len(), 2);
    // Moved right and down so the leftmost picture starts at the margin.
    assert_eq!((images[0].x, images[0].y), (MARGIN, MARGIN));
    assert_eq!(images[0].width, 40.0);
    assert_eq!(page.pictures.len(), 2);
    let decoded = image::load_from_memory(&page.pictures[0].1)
        .unwrap()
        .to_rgba8();
    assert_eq!(decoded.get_pixel(1, 0).0, [0, 0, 255, 128]);
}

#[test]
fn old_files_ask_for_a_resave() {
    let root = json!({ "version": "0.4.0", "data": {} });
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&serde_json::to_vec(&root).unwrap()).unwrap();
    let mut file = tempfile::Builder::new()
        .suffix(".rnote")
        .tempfile()
        .unwrap();
    file.write_all(&enc.finish().unwrap()).unwrap();
    let Err(Error::Invalid(message)) = read(file.path()) else {
        panic!("old files are refused");
    };
    assert!(message.contains("0.4.0"));
}
