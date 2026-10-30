use super::*;
use ductus_core::{Orientation, PageKind, PaperSize};
use std::io::Write as _;

fn gzip(xml: &str) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(xml.as_bytes()).unwrap();
    enc.finish().unwrap()
}

fn png() -> String {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbImage::new(4, 2)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    base64::engine::general_purpose::STANDARD.encode(out.into_inner())
}

fn page_of(import: Import) -> crate::ImportedPage {
    match import.entry {
        Entry::Page(page) => page,
        _ => panic!("expected a page"),
    }
}

const A4: &str = r#"width="595.27559" height="841.88976""#;

#[test]
fn a4_notebook_becomes_paper_with_ink_text_and_pictures() {
    let xml = format!(
        r##"<?xml version="1.0" standalone="no"?>
<xournal creator="Xournal++ 1.2.3" fileversion="4">
<title>Xournal++ document - see https://xournalpp.github.io/</title>
<page {A4}>
<background type="solid" color="#ffffffff" style="lined"/>
<layer>
<stroke tool="pen" color="#ff0000ff" width="2 1 2 1">10 20 30 40 50 60</stroke>
<stroke tool="highlighter" color="#ffff007f" width="8.5" style="dash">0 0 100 0</stroke>
<text font="Sans" size="12" x="100" y="200" color="#000000ff">x &lt; y
* item</text>
<image left="10" top="10" right="50" bottom="30">{png}</image>
</layer>
</page>
<page {A4}>
<background type="solid" color="#ffffffff" style="lined"/>
<layer><stroke tool="pen" color="blue" width="1.41">1 2 3 4</stroke></layer>
</page>
</xournal>"##,
        png = png()
    );
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Lecture 1.xopp");
    std::fs::write(&file, gzip(&xml)).unwrap();
    let import = read(&file).unwrap();
    assert!(import.warnings.is_empty(), "{:?}", import.warnings);
    let page = page_of(import);
    assert_eq!(page.name, "Lecture 1");
    assert!(page.pdf.is_none());
    let doc = &page.document;
    assert_eq!(
        doc.settings().kind,
        PageKind::Paper {
            size: PaperSize::A4,
            orientation: Orientation::Portrait
        }
    );
    assert_eq!(doc.settings().background.pattern, PatternStyle::Lined);

    let pen = &doc.strokes()[0];
    assert_eq!(pen.style.tool, Tool::Pen);
    assert_eq!(pen.style.width, 2.0 * K);
    assert_eq!((pen.style.color.r, pen.style.color.g), (1.0, 0.0));
    // Widths 1, 2 and 1 of the widest 2, with points filled in between.
    let at = |x: f64| pen.points.iter().find(|p| p.x == x * K).unwrap();
    assert_eq!(at(10.0).pressure, 1.0 / 3.0);
    assert_eq!((at(30.0).y, at(30.0).pressure), (40.0 * K, 1.0));
    assert_eq!(at(50.0).pressure, 1.0 / 3.0);
    assert!(pen.points.len() > 3);

    let highlighter = &doc.strokes()[1];
    assert_eq!(highlighter.style.tool, Tool::Highlighter);
    assert_eq!(highlighter.style.color.a, 1.0);
    assert_eq!(highlighter.style.pattern, LinePattern::Dashed);

    let second = &doc.strokes()[2];
    let sheet_top = doc.sheets(0)[1].1;
    assert_eq!(second.points[0].y, sheet_top + 2.0 * K);

    assert_eq!(doc.texts()[0].markdown, "x \\< y\\\n\\* item");
    assert_eq!(doc.texts()[0].scale, 12.0 * K / TextBox::FONT_SIZE);
    let image = &doc.images()[0];
    assert_eq!((image.x, image.width), (10.0 * K, 40.0 * K));
    assert_eq!(page.pictures.len(), 1);
    assert_eq!(page.pictures[0].0, image.source);
}

#[test]
fn attached_pdf_backgrounds_stay_under_the_ink() {
    let pdf = ductus_pdf::compose(
        &[],
        &[
            ductus_pdf::SheetSource::Blank {
                width: 300.0,
                height: 200.0,
            },
            ductus_pdf::SheetSource::Blank {
                width: 300.0,
                height: 400.0,
            },
        ],
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("slides.xopp.bg.pdf"), &pdf).unwrap();
    let xml = r##"<xournal creator="Xournal++ 1.2.3" fileversion="4">
<page width="300" height="400"><background type="pdf" domain="attach" filename="bg.pdf" pageno="2ll"/><layer/></page>
<page width="300" height="200"><background type="pdf" pageno="1"/><layer/></page>
<page width="300" height="200"><background type="solid" color="#ffffffff" style="plain"/><layer/></page>
</xournal>"##;
    let file = dir.path().join("slides.xopp");
    std::fs::write(&file, xml).unwrap();
    let import = read(&file).unwrap();
    assert!(import.warnings.is_empty(), "{:?}", import.warnings);
    let page = page_of(import);
    assert_eq!(page.document.settings().kind, PageKind::Pdf);
    assert_eq!(page.document.pdf_sheets().len(), 3);
    let composed = lopdf::Document::load_mem(page.pdf.as_deref().unwrap()).unwrap();
    assert_eq!(composed.get_pages().len(), 3);
}

#[test]
fn a_missing_pdf_leaves_blank_pages_and_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    let xml = r#"<xournal><page width="300" height="200"><background type="pdf" domain="absolute" filename="/nowhere/x.pdf" pageno="1"/></page></xournal>"#;
    let file = dir.path().join("a.xoj");
    std::fs::write(&file, xml).unwrap();
    let import = read(&file).unwrap();
    assert_eq!(import.warnings.len(), 1);
    assert!(page_of(import).pdf.is_some());
}

#[test]
fn latex_becomes_editable_math() {
    let math = formula("$x^2$", (10.0, 10.0, 30.0, 20.0)).unwrap();
    assert_eq!(math.syntax, "latex");
    assert!((math.height - 20.0 * K).abs() < 0.01);
}

#[test]
fn other_files_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.xopp");
    std::fs::write(&file, "<svg/>").unwrap();
    assert!(read(&file).is_err());
}

#[test]
fn a_notebook_only_brings_in_pdfs_and_pictures_it_may_reach() {
    let pdf = ductus_pdf::compose(
        &[],
        &[ductus_pdf::SheetSource::Blank {
            width: 300.0,
            height: 200.0,
        }],
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("outside.pdf"), &pdf).unwrap();
    std::fs::write(root.path().join("secret.txt"), "not for notes").unwrap();
    let dir = root.path().join("notes");
    std::fs::create_dir(&dir).unwrap();
    let secret = root.path().join("secret.txt");
    // Each would be a readable PDF or file, but neither may come in.
    for (name, background) in [
        (
            "climbs",
            r#"domain="attach" filename="../outside.pdf""#.to_owned(),
        ),
        (
            "text",
            format!(r#"domain="absolute" filename="{}""#, secret.display()),
        ),
    ] {
        let xml = format!(
            r#"<xournal><page width="300" height="200"><background type="pdf" {background} pageno="1"/></page></xournal>"#
        );
        let file = dir.join(format!("{name}.xopp"));
        std::fs::write(&file, xml).unwrap();
        let import = read(&file).unwrap();
        assert_eq!(import.warnings.len(), 1, "{name}: {:?}", import.warnings);
    }
    // The same PDF by its absolute path is what Xournal++ itself writes.
    let xml = format!(
        r#"<xournal><page width="300" height="200"><background type="pdf" domain="absolute" filename="{}" pageno="1"/></page></xournal>"#,
        root.path().join("outside.pdf").display()
    );
    let file = dir.join("absolute.xopp");
    std::fs::write(&file, xml).unwrap();
    assert!(read(&file).unwrap().warnings.is_empty());
}
