use super::*;
use ductus_core::{Orientation, PageBackground, PaperSize, StrokeId};

const DESK: Rgba = Rgba::rgb(0.85, 0.85, 0.88);

fn line(x0: f64, x1: f64, y: f64, style: StrokeStyle) -> StrokePath {
    let points: Vec<_> = (0..=20)
        .map(|i| InkPoint {
            x: x0 + (x1 - x0) * f64::from(i) / 20.0,
            y,
            pressure: 1.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
            t: 0,
        })
        .collect();
    StrokePath::new(&Stroke {
        id: StrokeId(1),
        style,
        points,
    })
    .unwrap()
}

fn pen(width: f64) -> StrokeStyle {
    StrokeStyle {
        width,
        ..StrokeStyle::default()
    }
}

fn layout(kind: PageKind, pattern: PatternStyle, spare: usize) -> PageLayout {
    let settings = PageSettings {
        kind,
        background: PageBackground {
            pattern,
            spacing: 20.0,
            ..PageBackground::default()
        },
    };
    PageLayout::for_document(&Document::with_contents(settings, []), spare)
}

fn endless(pattern: PatternStyle) -> PageLayout {
    layout(PageKind::Endless, pattern, 0)
}

fn paper(pattern: PatternStyle) -> PageLayout {
    let a5 = PageKind::Paper {
        size: PaperSize::A5,
        orientation: Orientation::Portrait,
    };
    layout(a5, pattern, 1)
}

fn vp(x: f64, y: f64, scale: f64) -> Viewport {
    Viewport {
        origin: kurbo::Point::new(x, y),
        scale,
    }
}

fn rgb(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let c = p.pixel(x, y).unwrap().demultiply();
    (c.red(), c.green(), c.blue())
}

#[test]
fn ink_lands_where_the_viewport_maps_it() {
    let mut p = Pixmap::new(100, 100).unwrap();
    render_view(
        &mut p,
        &vp(50.0, 50.0, 2.0),
        &endless(PatternStyle::Blank),
        DESK,
        &[line(50.0, 100.0, 70.0, pen(4.0))],
    );
    assert!(rgb(&p, 50, 40).0 < 60, "ink expected at row 40");
    assert!(rgb(&p, 50, 60).0 > 240, "paper expected at row 60");
}

#[test]
fn highlight_is_drawn_beneath_ink() {
    let mut p = Pixmap::new(60, 40).unwrap();
    let marker = StrokeStyle {
        tool: Tool::Highlighter,
        color: Rgba {
            r: 1.0,
            g: 0.85,
            b: 0.1,
            a: 0.4,
        },
        width: 18.0,
        ..StrokeStyle::default()
    };
    let strokes = [
        line(5.0, 55.0, 20.0, pen(4.0)),
        line(5.0, 55.0, 20.0, marker),
    ];
    render_view(
        &mut p,
        &vp(0.0, 0.0, 1.0),
        &endless(PatternStyle::Blank),
        DESK,
        &strokes,
    );
    assert!(rgb(&p, 30, 20).0 < 60);
    let (r, _, b) = rgb(&p, 30, 26);
    assert!(r > 230 && b < 190, "yellow expected");
}

#[test]
fn ink_is_clipped_to_paper() {
    let layout = paper(PatternStyle::Blank);
    let mut p = Pixmap::new(100, 60).unwrap();
    let view = vp(-50.0, 0.0, 1.0);
    render_view(
        &mut p,
        &view,
        &layout,
        DESK,
        &[line(-40.0, 40.0, 30.0, pen(6.0))],
    );
    assert!(rgb(&p, 20, 30).0 > 150, "no ink on the desk");
    assert!(rgb(&p, 80, 30).0 < 60, "ink on the sheet");
}

fn render_view(
    pixmap: &mut Pixmap,
    vp: &Viewport,
    layout: &PageLayout,
    desk: Rgba,
    strokes: &[StrokePath],
) {
    render_page_background(pixmap, vp, layout, desk, None);
    let clip = ink_clip(pixmap, vp, layout);
    for layer in [Layer::Highlight, Layer::Ink] {
        render_strokes(
            pixmap,
            vp,
            strokes.iter().filter(|s| s.layer == layer),
            clip.as_ref(),
        );
    }
}
