use super::*;
use crate::testpdf::make_pdf;
use crate::{PdfFile, engine};
use lopdf::IncrementalDocument;
use tiny_skia::Pixmap;

fn stroke(points: &[(f64, f64)], tool: Tool, tilt: bool) -> Stroke {
    Stroke {
        id: StrokeId(0),
        style: StrokeStyle {
            tool,
            color: if tool == Tool::Pen {
                Rgba::rgb(0.1, 0.2, 0.9)
            } else {
                Rgba {
                    r: 1.0,
                    g: 0.85,
                    b: 0.1,
                    a: 0.4,
                }
            },
            width: if tool == Tool::Pen { 4.0 } else { 18.0 },
            ..StrokeStyle::default()
        },
        points: points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| InkPoint {
                x,
                y,
                pressure: 0.8,
                tilt_x: if tilt { 0.5 } else { 0.0 },
                tilt_y: 0.0,
                t: i as u32 * 10,
            })
            .collect(),
    }
}

fn sample_ink(page: usize) -> Vec<PageInk> {
    let line: Vec<_> = (0..=20)
        .map(|i| (100.0 + f64::from(i) * 10.0, 400.0))
        .collect();
    vec![
        PageInk {
            page,
            stroke: stroke(&line, Tool::Pen, true),
        },
        PageInk {
            page,
            stroke: stroke(&[(100.0, 450.0), (300.0, 450.0)], Tool::Highlighter, false),
        },
    ]
}

fn ductus_annotation_count(bytes: &[u8]) -> (usize, usize) {
    let doc = Document::load_mem(bytes).unwrap();
    let (mut ours, mut foreign) = (0, 0);
    for (_, page) in doc.get_pages() {
        for e in page_annotations(&doc, page) {
            if is_ours(&e) { ours += 1 } else { foreign += 1 }
        }
    }
    (ours, foreign)
}

/// Render tests are skipped without pdfium (set DUCTUS_PDFIUM).
fn pdfium_or_skip() -> bool {
    match engine() {
        Ok(_) => true,
        Err(e) => {
            assert!(
                std::env::var_os("DUCTUS_PDFIUM").is_none(),
                "DUCTUS_PDFIUM set but {e}"
            );
            eprintln!("skipping rendering check: {e}");
            false
        }
    }
}

fn render(bytes: &[u8], page: usize) -> Pixmap {
    let file = PdfFile::open(bytes.to_vec()).unwrap();
    let (w, h) = file.page_sizes()[page];
    let mut pixmap = Pixmap::new(w.ceil() as u32, h.ceil() as u32).unwrap();
    file.render_into(page, &mut pixmap, (0.0, 0.0), 1.0)
        .unwrap();
    if let Ok(dir) = std::env::var("DUCTUS_SHOTS") {
        std::fs::create_dir_all(&dir).unwrap();
        let name = format!("{dir}/pdf-{}-{page}.png", bytes.len());
        pixmap.save_png(name).unwrap();
    }
    pixmap
}

fn rgb(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let c = p.pixel(x, y).unwrap().demultiply();
    (c.red(), c.green(), c.blue())
}

#[test]
fn ink_round_trips_through_the_pdf() {
    let original = make_pdf(&[(595.0, 842.0, 0), (595.0, 842.0, 0)]);
    let written = write_ink(&original, &sample_ink(1)).unwrap();
    let back = read_ink(&written).unwrap();
    assert_eq!(back.len(), 2);
    assert!(back.iter().all(|i| i.page == 1));
    let pen = back
        .iter()
        .find(|i| i.stroke.style.tool == Tool::Pen)
        .unwrap();
    assert_eq!(pen.stroke.points.len(), 21);
    assert!((pen.stroke.points[5].x - 150.0).abs() < 0.01);
    assert_eq!(pen.stroke.points[5].tilt_x, 0.5);
    assert!((pen.stroke.style.color.b - 0.9).abs() < 0.01);
    let marker = back
        .iter()
        .find(|i| i.stroke.style.tool == Tool::Highlighter)
        .unwrap();
    assert!((marker.stroke.style.color.a - 0.4).abs() < 0.01);
}

#[test]
fn the_original_bytes_stay_intact() {
    let original = make_pdf(&[(595.0, 842.0, 0)]);
    let written = write_ink(&original, &sample_ink(0)).unwrap();
    assert!(written.starts_with(&original), "incremental update appends");
}

#[test]
fn repeated_saves_replace_our_update() {
    let original = make_pdf(&[(595.0, 842.0, 0)]);
    let first = write_ink(&original, &sample_ink(0)).unwrap();
    let mut more = sample_ink(0);
    more.extend(sample_ink(0));
    let second = write_ink(&first, &more).unwrap();
    let third = write_ink(&second, &sample_ink(0)).unwrap();
    assert!(second.starts_with(&original) && third.starts_with(&original));
    assert_eq!(
        third.len(),
        first.len(),
        "back to one update of the same size"
    );
    assert_eq!(ductus_annotation_count(&second).0, 4);
    assert_eq!(ductus_annotation_count(&third).0, 2);
    assert!(write_ink(&third, &[]).unwrap().len() < first.len());
}

fn foreign_update(bytes: &[u8]) -> Vec<u8> {
    let mut inc = IncrementalDocument::load_from(bytes).unwrap();
    let prev = inc.get_prev_documents().clone();
    let (_, page) = prev.get_pages().into_iter().next().unwrap();
    let mut note = Dictionary::new();
    note.set("Type", Object::Name(b"Annot".to_vec()));
    note.set("Subtype", Object::Name(b"Text".to_vec()));
    note.set(
        "Rect",
        Object::Array(vec![10.into(), 10.into(), 30.into(), 30.into()]),
    );
    note.set("Contents", Object::string_literal("from another app"));
    let id = inc.new_document.add_object(note);
    let mut annots: Vec<Object> = page_annotations(&prev, page)
        .into_iter()
        .map(|e| e.0)
        .collect();
    annots.push(Object::Reference(id));
    inc.opt_clone_object_to_new_document(page).unwrap();
    inc.new_document
        .get_dictionary_mut(page)
        .unwrap()
        .set("Annots", Object::Array(annots));
    let mut out = Vec::new();
    inc.save_to(&mut out).unwrap();
    out
}

#[test]
fn updates_by_other_programs_are_kept() {
    let original = make_pdf(&[(595.0, 842.0, 0)]);
    let ours = write_ink(&original, &sample_ink(0)).unwrap();
    let theirs = foreign_update(&ours);
    let again = write_ink(&theirs, &sample_ink(0)).unwrap();
    assert!(
        again.starts_with(&theirs),
        "their update is part of the new base"
    );
    assert_eq!(ductus_annotation_count(&again), (2, 1));
}

#[test]
fn stripping_keeps_foreign_annotations() {
    let original = make_pdf(&[(595.0, 842.0, 0)]);
    let theirs = foreign_update(&write_ink(&original, &sample_ink(0)).unwrap());
    let stripped = strip_ductus_annotations(&theirs).unwrap();
    assert_eq!(ductus_annotation_count(&stripped), (0, 1));
}

#[test]
fn rotated_pages_map_ink_to_the_displayed_position() {
    if !pdfium_or_skip() {
        return;
    }
    for rotate in [90, 180, 270] {
        let pdf = make_pdf(&[(595.0, 842.0, rotate)]);
        let written = write_ink(&pdf, &sample_ink(0)).unwrap();
        let page = render(&written, 0);
        let (r, _, b) = rgb(&page, 200, 400);
        assert!(
            b > 150 && r < 100,
            "rotate {rotate}: ink expected at (200, 400)"
        );
    }
}

#[test]
fn flattening_burns_ink_into_the_page() {
    if !pdfium_or_skip() {
        return;
    }
    let pdf = make_pdf(&[(595.0, 842.0, 0)]);
    let flat = flatten(&pdf, &sample_ink(0), &[]).unwrap();
    assert_eq!(ductus_annotation_count(&flat).0, 0, "no annotations left");
    let (r, _, b) = rgb(&render(&flat, 0), 200, 400);
    assert!(b > 150 && r < 100, "ink is part of the page content");
}

#[test]
fn overlay_pages_land_where_they_are_shown_on_rotated_pages() {
    if !pdfium_or_skip() {
        return;
    }
    for rotate in [90, 180] {
        let base = make_pdf(&[(595.0, 842.0, rotate)]);
        let shown = if rotate == 90 {
            (842.0, 595.0)
        } else {
            (595.0, 842.0)
        };
        let overlay = make_pdf(&[(shown.0, shown.1, 0)]);
        let merged = crate::overlay_pages(&base, &overlay).unwrap();
        let page = render(&merged, 0);
        let (r, g, b) = rgb(&page, 80, 80);
        assert!(
            r > 200 && g < 60 && b < 60,
            "rotate {rotate}: overlay square at (80, 80)"
        );
    }
}

fn blank_pdf(w: f64, h: f64) -> Vec<u8> {
    use lopdf::{Dictionary, Object, dictionary};
    let mut doc = lopdf::Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let page = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()],
        "Resources" => Dictionary::new(),
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(
            dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 },
        ),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

#[test]
fn a_second_overlay_keeps_the_first() {
    if !pdfium_or_skip() {
        return;
    }
    // Formulas go on first, then the text boxes: both have to stay.
    let base = blank_pdf(595.0, 842.0);
    let formulas = crate::overlay_pages(&base, &make_pdf(&[(595.0, 842.0, 0)])).unwrap();
    let both = crate::overlay_pages(&formulas, &blank_pdf(595.0, 842.0)).unwrap();
    let (r, g, b) = rgb(&render(&both, 0), 80, 80);
    assert!(
        r > 200 && g < 60 && b < 60,
        "the first overlay is still drawn"
    );
}

#[test]
fn other_readers_show_text_formulas_and_pictures() {
    if !pdfium_or_skip() {
        return;
    }
    use ductus_core::{Image, MathBox, TextBox};
    let mut red = Pixmap::new(4, 4).unwrap();
    red.fill(tiny_skia::Color::from_rgba8(220, 20, 20, 255));
    let mut math = MathBox {
        id: StrokeId(0),
        source: r"\int_0^1 x\,dx".into(),
        syntax: "latex".into(),
        x: 60.0,
        y: 500.0,
        width: 0.0,
        height: 0.0,
        scale: 3.0,
        color: Rgba::rgb(0.1, 0.2, 0.9),
    };
    (math.width, math.height) = ductus_math::size(&math).unwrap();
    let objects = [
        PageObject::Picture(
            Image {
                id: StrokeId(0),
                source: "img.png".into(),
                x: 60.0,
                y: 60.0,
                width: 120.0,
                height: 120.0,
            },
            red.encode_png().unwrap(),
        ),
        PageObject::Text(TextBox {
            id: StrokeId(0),
            markdown: "**Hello** readers".into(),
            x: 60.0,
            y: 300.0,
            width: 400.0,
            height: 50.0,
            scale: 1.0,
        }),
        PageObject::Math(math),
    ]
    .map(|object| PlacedObject { page: 0, object });
    let pdf = make_pdf(&[(595.0, 842.0, 0)]);
    // Rendered at one pixel per document unit.
    let page = render(&write_annotations(&pdf, &[], &objects).unwrap(), 0);
    let count = |x0: u32, y0: u32, w: u32, h: u32, f: &dyn Fn((u8, u8, u8)) -> bool| {
        (x0..x0 + w)
            .flat_map(|x| (y0..y0 + h).map(move |y| (x, y)))
            .filter(|&(x, y)| f(rgb(&page, x, y)))
            .count()
    };
    let (r, g, _) = rgb(&page, 120, 120);
    assert!(r > 200 && g < 60, "the picture");
    let dark = |(r, g, b): (u8, u8, u8)| r < 100 && g < 100 && b < 100;
    assert!(count(60, 300, 300, 50, &dark) > 30, "the text");
    let blue = |(r, _, b): (u8, u8, u8)| b > 150 && r < 100;
    assert!(count(60, 500, 200, 120, &blue) > 30, "the formula");
}
