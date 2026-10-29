use super::*;
use ductus_core::SHEET_GAP;

fn stroke(tool: Tool, color: Rgba, points: &[(f64, f64, f32)], tilt: bool) -> Stroke {
    Stroke {
        id: StrokeId(0),
        style: StrokeStyle {
            tool,
            color,
            width: if tool == Tool::Pen { 3.0 } else { 18.0 },
            ..StrokeStyle::default()
        },
        points: points
            .iter()
            .enumerate()
            .map(|(i, &(x, y, pressure))| InkPoint {
                x,
                y,
                pressure,
                tilt_x: if tilt { 0.25 } else { 0.0 },
                tilt_y: if tilt { -0.5 } else { 0.0 },
                t: 1000 + i as u32 * 8,
            })
            .collect(),
    }
}

fn sample_page(settings: PageSettings) -> PageFile {
    let line: Vec<_> = (0..=20)
        .map(|i| {
            (
                40.0 + f64::from(i) * 15.0,
                120.0,
                0.2 + f32::from(i as u8) * 0.04,
            )
        })
        .collect();
    let marker = [(30.0, 120.0, 1.0), (360.0, 120.0, 1.0)];
    let doc = Document::with_contents(
        settings,
        [
            stroke(Tool::Pen, Rgba::rgb(0.1, 0.2, 0.8), &line, true),
            stroke(
                Tool::Highlighter,
                Rgba {
                    r: 1.0,
                    g: 0.85,
                    b: 0.1,
                    a: 0.4,
                },
                &marker,
                false,
            ),
        ],
    );
    PageFile {
        id: "abc123".into(),
        document: doc,
        pdf: None,
        recognized: Vec::new(),
        pictures: BTreeMap::new(),
    }
}

fn paper_settings() -> PageSettings {
    PageSettings {
        kind: PageKind::Paper {
            size: PaperSize::A5,
            orientation: Orientation::Landscape,
        },
        background: PageBackground {
            pattern: PatternStyle::Grid,
            spacing: 20.0,
            ..PageBackground::default()
        },
    }
}

#[test]
fn round_trip_keeps_ink_data() {
    let page = sample_page(PageSettings::default());
    let svg = write_page(&page);
    let back = read_page(&svg).unwrap();
    assert_eq!(back.id, "abc123");

    let (a, b) = (page.document.strokes(), back.document.strokes());
    assert_eq!(b.len(), 2);
    for original in a {
        let loaded = b
            .iter()
            .find(|s| s.style.tool == original.style.tool)
            .unwrap();
        assert_eq!(loaded.style.width, original.style.width);
        assert!((loaded.style.color.a - original.style.color.a).abs() < 0.001);
        assert!((loaded.style.color.b - original.style.color.b).abs() < 0.01);
        assert_eq!(loaded.points.len(), original.points.len());
        for (p, q) in original.points.iter().zip(&loaded.points) {
            assert!((p.x - q.x).abs() < 0.01 && (p.y - q.y).abs() < 0.01);
            assert!((p.pressure - q.pressure).abs() < 0.001);
            assert_eq!(q.t, p.t - original.points[0].t, "times are stored relative");
            assert_eq!((q.tilt_x, q.tilt_y), (p.tilt_x, p.tilt_y));
        }
    }
}

#[test]
fn round_trip_keeps_page_settings() {
    let page = sample_page(paper_settings());
    let back = read_page(&write_page(&page)).unwrap();
    assert_eq!(
        *back.document.settings(),
        paper_settings_after_hex(paper_settings())
    );
}

fn paper_settings_after_hex(mut s: PageSettings) -> PageSettings {
    let q = |c: Rgba| {
        let r = |v: f32| (v * 255.0).round() / 255.0;
        Rgba::rgb(r(c.r), r(c.g), r(c.b))
    };
    s.background.paper = q(s.background.paper);
    s.background.ink = q(s.background.ink);
    s
}

#[test]
fn rejects_foreign_and_future_files() {
    let plain = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0,0L1,1"/></svg>"#;
    assert!(matches!(read_page(plain), Err(Error::Invalid(_))));
    let future = format!(r#"<svg xmlns="{SVG_NS}" xmlns:dx="{DX_NS}" dx:version="99"/>"#);
    assert!(matches!(read_page(&future), Err(Error::Invalid(m)) if m.contains("newer")));
    assert!(read_page("not xml <<<").is_err());
}

#[test]
fn reads_other_namespace_prefixes() {
    let svg = write_page(&sample_page(PageSettings::default()))
        .replace("xmlns:dx=", "xmlns:ductus=")
        .replace("dx:", "ductus:");
    assert_eq!(read_page(&svg).unwrap().document.strokes().len(), 2);
}

#[test]
fn other_svg_renderers_show_the_ink() {
    let svg = write_page(&sample_page(PageSettings::default()));
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );

    let bounds = page_area(&svg);
    let px = |x: f64, y: f64| {
        let c = pixmap
            .pixel((x - bounds.0) as u32, (y - bounds.1) as u32)
            .unwrap()
            .demultiply();
        (c.red(), c.green(), c.blue())
    };
    let (r, g, b) = px(200.0, 120.0);
    assert!(b > 120 && r < 80, "blue pen ink expected, got {r} {g} {b}");
    let (r, g, b) = px(200.0, 127.0);
    assert!(
        r > 230 && b < 190,
        "yellow highlight expected, got {r} {g} {b}"
    );
    let (r, g, b) = px(200.0, 150.0);
    assert!(
        r > 240 && g > 240 && b > 230,
        "paper expected, got {r} {g} {b}"
    );
}

fn page_area(svg: &str) -> (f64, f64) {
    let start = svg.find("viewBox=\"").unwrap() + 9;
    let mut it = svg[start..]
        .split([' ', '"'])
        .map(|v| v.parse::<f64>().unwrap_or(0.0));
    (it.next().unwrap(), it.next().unwrap())
}

#[test]
fn pdf_backed_pages_keep_their_sheets_and_source() {
    let mut page = sample_page(PageSettings {
        kind: PageKind::Pdf,
        ..PageSettings::default()
    });
    page.document
        .set_pdf_sheets(vec![(793.7, 1122.5), (1122.5, 793.7)]);
    page.pdf = Some("source.pdf".into());
    let svg = write_page(&page);
    assert!(!svg.contains("dx-pattern"), "no pattern over a PDF");
    let back = read_page(&svg).unwrap();
    assert_eq!(back.document.settings().kind, PageKind::Pdf);
    assert_eq!(
        back.document.pdf_sheets(),
        &[(793.7, 1122.5), (1122.5, 793.7)]
    );
    assert_eq!(back.pdf.as_deref(), Some("source.pdf"));
    // Pages come from other devices and apps: their PDF is a name next to them, never a path.
    for path in ["../../secret.pdf", "/etc/passwd", "C:x.pdf", ".."] {
        let crafted = svg.replace(r#"dx:pdf="source.pdf""#, &format!(r#"dx:pdf="{path}""#));
        assert_eq!(read_page(&crafted).unwrap().pdf, None, "{path}");
    }
    let mut crafted = page.clone();
    crafted.id = r#"x" onload="alert(1)"#.into();
    assert_eq!(
        read_page(&write_page(&crafted)).unwrap().id,
        crafted.id,
        "the id stays one attribute"
    );
    let height = 1122.5 + SHEET_GAP + 793.7;
    assert!(
        svg.contains(&format!("{} {}", num(1122.5, 2), num(height, 2))),
        "viewBox covers both sheets"
    );
}

#[test]
fn pictures_are_inside_the_page_and_other_renderers_show_them() {
    let mut red = resvg::tiny_skia::Pixmap::new(4, 4).unwrap();
    red.fill(resvg::tiny_skia::Color::from_rgba8(220, 20, 20, 255));
    let png = red.encode_png().unwrap();
    let mut page = sample_page(PageSettings::default());
    page.document = page.document.clone().with_images([Image {
        id: StrokeId(0),
        source: "img-0123.png".into(),
        x: 50.0,
        y: 400.0,
        width: 320.5,
        height: 180.0,
    }]);
    page.pictures.insert("img-0123.png".into(), png.clone());
    let svg = write_page(&page);
    assert!(svg.contains(r#"href="data:image/png;base64,"#));
    assert!(svg.find("<image").unwrap() < svg.find(r#"dx:layer="highlight""#).unwrap());
    let back = read_page(&svg).unwrap();
    let image = &back.document.images()[0];
    assert_eq!(image.source, "img-0123.png");
    assert_eq!(back.pictures["img-0123.png"], png);
    assert_eq!(
        (image.x, image.y, image.width, image.height),
        (50.0, 400.0, 320.5, 180.0)
    );
    assert_eq!(back.document.strokes().len(), 2);

    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let bounds = page_area(&svg);
    let c = pixmap
        .pixel((200.0 - bounds.0) as u32, (500.0 - bounds.1) as u32)
        .unwrap()
        .demultiply();
    assert!(
        c.red() > 200 && c.green() < 60,
        "the picture expected, got {} {} {}",
        c.red(),
        c.green(),
        c.blue()
    );
}

#[test]
fn text_boxes_round_trip_and_show_as_html() {
    let mut page = sample_page(PageSettings::default());
    let markdown = "# Limits\n\nA **sequence** converges & <b>stays</b>.\n\n- one\n- two\n";
    page.document = page.document.clone().with_texts([TextBox {
        id: StrokeId(0),
        markdown: markdown.into(),
        x: 40.0,
        y: 300.0,
        width: 320.0,
        height: 120.5,
        scale: 1.5,
    }]);
    let svg = write_page(&page);
    assert!(svg.contains("<h1>Limits</h1>"));
    assert!(svg.contains("<strong>sequence</strong>"));
    assert!(svg.contains("&lt;b&gt;stays&lt;/b&gt;"));
    assert!(svg.contains("<li>two</li>"));
    let back = read_page(&svg).unwrap();
    let text = &back.document.texts()[0];
    assert_eq!(text.markdown, markdown);
    assert_eq!(
        (text.x, text.y, text.width, text.height, text.scale),
        (40.0, 300.0, 320.0, 120.5, 1.5)
    );
    resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
}

#[test]
fn formulas_round_trip_as_vector_paths() {
    let mut page = sample_page(PageSettings::default());
    let source = "\\frac{a + b}{2} \\geq \\sqrt{ab}";
    page.document = page.document.clone().with_maths([MathBox {
        id: StrokeId(0),
        source: source.into(),
        syntax: "latex".into(),
        x: 50.0,
        y: 500.0,
        width: 120.0,
        height: 40.0,
        scale: 1.5,
        color: Rgba::rgb(0.12, 0.31, 0.82),
    }]);
    let svg = write_page(&page);
    let group = &svg[svg.find("dx:math=").unwrap()..];
    assert!(
        group[..group.find("</g>").unwrap()]
            .matches("<path")
            .count()
            > 5
    );
    let back = read_page(&svg).unwrap();
    let math = &back.document.maths()[0];
    assert_eq!(math.source, source);
    assert_eq!(math.syntax, "latex");
    assert_eq!((math.x, math.y, math.scale), (50.0, 500.0, 1.5));
    assert!((math.color.b - 0.82).abs() < 0.01);
    resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
}

#[test]
fn dotted_and_dashed_lines_round_trip() {
    let mut page = sample_page(PageSettings::default());
    let mut strokes = page.document.strokes().to_vec();
    strokes[0].style.pattern = LinePattern::Dotted;
    strokes[1].style.pattern = LinePattern::Dashed;
    page.document = Document::with_contents(*page.document.settings(), strokes);
    let svg = write_page(&page);
    assert!(svg.contains(r#"dx:line="dotted""#));
    let back = read_page(&svg).unwrap();
    let patterns: Vec<_> = back
        .document
        .strokes()
        .iter()
        .map(|s| s.style.pattern)
        .collect();
    assert!(patterns.contains(&LinePattern::Dotted) && patterns.contains(&LinePattern::Dashed));
}

#[test]
fn recognized_handwriting_round_trips_as_invisible_text() {
    let mut page = sample_page(PageSettings::default());
    page.recognized = vec![RecognizedLine {
        key: "k1".into(),
        text: "Grenzwert <x> & Folgen".into(),
        rect: (40.0, 100.0, 300.0, 30.0),
        lang: Some("de".into()),
    }];
    let svg = write_page(&page);
    assert!(svg.contains(r#"fill-opacity="0""#));
    assert!(svg.contains("Grenzwert &lt;x&gt; &amp; Folgen</text>"));
    let back = read_page(&svg).unwrap();
    assert_eq!(back.recognized, page.recognized);
    assert_eq!(back.document.strokes().len(), 2);
    resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
}

#[test]
fn text_boxes_keep_underline_color_and_size_as_xhtml() {
    let html = markdown_html(
        "Plain <u>under **bold**</u> and <span style=\"color:#d12f2f;font-size:22px\">big red</span>\n\n<u>left open\n\n<script>no</script> <span style=\"position:fixed\">x</span>",
    );
    assert!(
        html.contains("<u>under <strong>bold</strong></u>"),
        "{html}"
    );
    assert!(
        html.contains(r#"<span style="color:#d12f2f;font-size:1.375em">big red</span>"#),
        "{html}"
    );
    assert!(
        html.contains("<u>left open</u></p>"),
        "closed with its paragraph: {html}"
    );
    assert!(!html.contains("<script>"), "{html}");
    assert!(
        html.contains("&lt;span style="),
        "unknown styles stay text: {html}"
    );
}

#[test]
fn text_links_lead_only_to_the_web_or_mail() {
    let html = super::markdown_html(
        "[run](javascript:alert(1)) [site](https://example.org) [mail](mailto:a@b.c) ![x](http://tracker/p.png)",
    );
    assert!(!html.contains("javascript"), "{html}");
    assert!(!html.contains("<img"), "{html}");
    assert!(html.contains(r#"href="https://example.org""#), "{html}");
    assert!(html.contains(r#"href="mailto:a@b.c""#), "{html}");
    assert!(
        html.contains("run") && html.contains('x'),
        "the text stays: {html}"
    );
}
