use super::*;

fn canvas() -> Canvas {
    let mut c = Canvas::new();
    c.set_viewport(400.0, 300.0, 1.0);
    c
}

fn pen_sample(x: f64, y: f64, t: u32, pressure: f64) -> Sample {
    Sample {
        pressure: Some(pressure),
        ..Sample::at(x, y, t)
    }
}

fn draw_wave(c: &mut Canvas, x0: f64, x1: f64, y: f64) {
    let n = 60;
    let point = |i: u32| {
        let f = f64::from(i) / f64::from(n);
        pen_sample(
            x0 + (x1 - x0) * f,
            y + 15.0 * (f * 12.0).sin(),
            i * 5,
            0.3 + 0.7 * f,
        )
    };
    assert!(c.pointer_down(Device::Pen, point(0), None));
    for i in 1..=n {
        c.pointer_move(&[point(i)]);
    }
    c.pointer_up();
}

pub(crate) fn canvas_for_tests() -> Canvas {
    canvas()
}

fn shot(c: &mut Canvas) -> Pixmap {
    c.snapshot().expect("viewport has a size")
}

fn ink_in(p: &Pixmap, x0: u32, y0: u32, x1: u32, y1: u32) -> usize {
    let mut n = 0;
    for y in y0..y1.min(p.height()) {
        for x in x0..x1.min(p.width()) {
            let c = p.pixel(x, y).unwrap();
            if c.red() < 100 && c.green() < 100 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn pen_stroke_is_committed_and_rendered() {
    let mut c = canvas();
    draw_wave(&mut c, 50.0, 350.0, 150.0);
    assert_eq!(c.document().strokes().len(), 1);
    assert!(c.can_undo());
    let p = shot(&mut c);
    assert!(ink_in(&p, 50, 125, 350, 175) > 300, "ink along the wave");
    assert_eq!(ink_in(&p, 0, 200, 400, 300), 0, "no ink below");
}

#[test]
fn only_touched_tiles_are_rerendered() {
    let mut c = canvas();
    c.set_viewport(800.0, 600.0, 1.0);
    let before: HashMap<_, _> = c
        .frame()
        .into_iter()
        .map(|t| (t.key, t.generation))
        .collect();
    draw_wave(&mut c, 60.0, 160.0, 80.0);
    let after = c.frame();
    let changed: Vec<_> = after
        .iter()
        .filter(|t| before.get(&t.key) != Some(&t.generation))
        .map(|t| (t.key.x, t.key.y))
        .collect();
    assert_eq!(changed, [(0, 0)]);
}

#[test]
fn eraser_removes_strokes_in_one_undo_step() {
    let mut c = canvas();
    c.set_eraser_mode(EraserMode::Stroke);
    draw_wave(&mut c, 50.0, 350.0, 100.0);
    draw_wave(&mut c, 50.0, 350.0, 200.0);
    c.pointer_down(Device::PenEraser, pen_sample(200.0, 60.0, 0, 1.0), None);
    for i in 1..=40 {
        c.pointer_move(&[pen_sample(200.0, 60.0 + f64::from(i) * 5.0, i * 5, 1.0)]);
    }
    c.pointer_up();
    assert!(c.document().strokes().is_empty());
    let p = shot(&mut c);
    assert_eq!(ink_in(&p, 0, 0, 400, 300), 0);

    c.undo();
    assert_eq!(c.document().strokes().len(), 2);
    let p = shot(&mut c);
    assert!(ink_in(&p, 50, 75, 350, 125) > 100);
}

#[test]
fn a_stand_in_tool_wins_over_the_chosen_one_and_the_eraser_end() {
    let mut c = canvas();
    c.set_eraser_mode(EraserMode::Stroke);
    draw_wave(&mut c, 50.0, 350.0, 100.0);
    // A pen button set to the eraser, with the pen chosen.
    c.pointer_down(
        Device::Pen,
        pen_sample(200.0, 60.0, 0, 1.0),
        Some(ToolMode::Eraser),
    );
    for i in 1..=20 {
        c.pointer_move(&[pen_sample(200.0, 60.0 + f64::from(i) * 4.0, i * 5, 1.0)]);
    }
    c.pointer_up();
    assert!(c.document().strokes().is_empty());
    // The eraser end set to the pen writes.
    c.pointer_down(
        Device::PenEraser,
        pen_sample(100.0, 100.0, 0, 1.0),
        Some(ToolMode::Pen),
    );
    c.pointer_move(&[pen_sample(200.0, 120.0, 50, 1.0)]);
    c.pointer_up();
    assert_eq!(c.document().strokes().len(), 1);
}

#[test]
fn touch_pans_unless_finger_drawing_is_on() {
    let mut c = canvas();
    assert!(!c.pointer_down(Device::Touch, Sample::at(10.0, 10.0, 0), None));
    assert!(!c.is_drawing());
    c.set_finger_draws(true);
    assert!(c.pointer_down(Device::Touch, Sample::at(10.0, 10.0, 0), None));
    assert!(c.is_drawing());
}

#[test]
fn cancel_discards_live_stroke_and_restores_erased() {
    let mut c = canvas();
    draw_wave(&mut c, 50.0, 350.0, 150.0);
    c.pointer_down(Device::PenEraser, pen_sample(200.0, 100.0, 0, 1.0), None);
    c.pointer_move(&[pen_sample(200.0, 200.0, 50, 1.0)]);
    c.pointer_cancel();
    assert_eq!(c.document().strokes().len(), 1);
    let p = shot(&mut c);
    assert!(ink_in(&p, 50, 125, 350, 175) > 300, "stroke visible again");
}

fn paper(pattern: ductus_core::PatternStyle) -> PageSettings {
    PageSettings {
        kind: ductus_core::PageKind::Paper {
            size: ductus_core::PaperSize::A5,
            orientation: ductus_core::Orientation::Portrait,
        },
        background: ductus_core::PageBackground {
            pattern,
            spacing: pattern.default_spacing(),
            ..ductus_core::PageBackground::default()
        },
    }
}

#[test]
fn writing_on_the_last_sheet_adds_another() {
    let mut c = Canvas::new();
    c.set_viewport(900.0, 700.0, 1.0);
    c.load_document(Document::with_contents(
        paper(ductus_core::PatternStyle::Grid),
        [],
    ));
    let (_, sheet_h) = c.page_settings().kind.sheet_size().unwrap();
    c.pan_by(0.0, -(sheet_h + ductus_core::SHEET_GAP));
    draw_wave(&mut c, 300.0, 600.0, 200.0);
    assert_eq!(c.layout().sheets.len(), 3);
    c.undo();
    assert_eq!(c.layout().sheets.len(), 2);
}

fn press_through(c: &mut Canvas, points: &[(f64, f64)]) {
    let (x, y) = points[0];
    assert!(c.pointer_down(Device::Pen, pen_sample(x, y, 0, 0.6), None));
    for (i, &(x, y)) in points.iter().enumerate().skip(1) {
        c.pointer_move(&[pen_sample(x, y, i as u32 * 8, 0.6)]);
    }
}

fn shaky_line(n: u32, from: (f64, f64), to: (f64, f64)) -> Vec<(f64, f64)> {
    (0..=n)
        .map(|i| {
            let f = f64::from(i) / f64::from(n);
            let wobble = 2.0 * (f * 17.0).sin();
            (
                from.0 + (to.0 - from.0) * f,
                from.1 + (to.1 - from.1) * f + wobble,
            )
        })
        .collect()
}

#[test]
fn holding_turns_a_shaky_line_straight() {
    let mut c = canvas();
    press_through(&mut c, &shaky_line(40, (60.0, 150.0), (340.0, 153.0)));
    assert_eq!(c.hold(), Some("line"));
    c.pointer_move(&[pen_sample(360.0, 152.0, 400, 0.6)]);
    c.pointer_up();
    let stroke = &c.document().strokes()[0];
    let ys: Vec<f64> = stroke.points.iter().map(|p| p.y).collect();
    let spread =
        ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min);
    assert!(spread < 0.5, "line is not straight: spread {spread}");
    let last = stroke.points.last().unwrap();
    assert!((last.x - c.view.to_doc(Point::new(360.0, 0.0)).x).abs() < 1.0);
    shot(&mut c);
    c.undo();
    assert!(
        c.document().strokes().is_empty(),
        "the shape is one undo step"
    );
}

#[test]
fn hold_on_a_scribble_keeps_the_ink() {
    let mut c = canvas();
    let scribble: Vec<(f64, f64)> = (0..60)
        .map(|i| {
            let f = f64::from(i);
            (
                100.0 + f * 3.0,
                150.0 + 40.0 * (f * 0.9).sin() * (f * 0.31).cos(),
            )
        })
        .collect();
    press_through(&mut c, &scribble);
    assert_eq!(c.hold(), None);
    c.pointer_up();
    assert_eq!(c.document().strokes().len(), 1);
}

fn lasso(c: &mut Canvas, rect: (f64, f64, f64, f64)) {
    let (x0, y0, x1, y1) = rect;
    let ring = [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)];
    assert!(c.pointer_down(Device::Pen, pen_sample(x0, y0, 0, 0.5), None));
    for (i, &(x, y)) in ring.iter().enumerate().skip(1) {
        c.pointer_move(&[pen_sample(x, y, i as u32 * 10, 0.5)]);
    }
    c.pointer_up();
}

pub(crate) fn drag(c: &mut Canvas, from: (f64, f64), to: (f64, f64)) {
    assert!(c.pointer_down(Device::Pen, pen_sample(from.0, from.1, 0, 0.5), None));
    for i in 1..=10 {
        let f = f64::from(i) / 10.0;
        let (x, y) = (from.0 + (to.0 - from.0) * f, from.1 + (to.1 - from.1) * f);
        c.pointer_move(&[pen_sample(x, y, i * 10, 0.5)]);
    }
    c.pointer_up();
}

#[test]
fn lasso_selects_moves_and_undoes() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 80.0);
    draw_wave(&mut c, 220.0, 360.0, 220.0);
    c.set_tool(ToolMode::Lasso);
    lasso(&mut c, (20.0, 40.0, 180.0, 130.0));
    assert_eq!(c.selection_count(), 1);
    let rect = c.selection_screen_rect().unwrap();
    assert!(rect.x0 > 20.0 && rect.x1 < 180.0);
    shot(&mut c);

    let start = rect.center();
    drag(&mut c, (start.x, start.y), (start.x, start.y + 100.0));
    let p = shot(&mut c);
    assert_eq!(ink_in(&p, 40, 60, 160, 100), 0, "old place is empty");
    assert!(
        ink_in(&p, 40, 160, 160, 200) > 0,
        "wave is at its new place"
    );
    assert_eq!(c.selection_count(), 1, "selection follows the move");

    c.undo();
    let p = shot(&mut c);
    assert!(ink_in(&p, 40, 60, 160, 100) > 0);
    assert_eq!(ink_in(&p, 40, 160, 160, 200), 0);
}

#[test]
fn a_moved_selection_shifts_its_tiles_without_redrawing_them() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 80.0);
    c.set_tool(ToolMode::Lasso);
    lasso(&mut c, (20.0, 40.0, 180.0, 130.0));
    let overlay = |c: &mut Canvas| -> HashMap<TileKey, u64> {
        c.frame()
            .into_iter()
            .filter(|t| t.key.layer == TileLayer::Overlay)
            .map(|t| (t.key, t.generation))
            .collect()
    };
    assert!(c.pointer_down(Device::Pen, pen_sample(100.0, 80.0, 0, 0.5), None));
    c.pointer_move(&[pen_sample(100.0, 90.0, 10, 0.5)]);
    let before = overlay(&mut c);
    c.pointer_move(&[pen_sample(100.0, 230.0, 20, 0.5)]);
    let after = overlay(&mut c);
    assert!(after.iter().any(|(key, g)| before.get(key) == Some(g)));
    assert!(
        after
            .iter()
            .all(|(key, g)| before.get(key).is_none_or(|b| b == g))
    );
    let p = shot(&mut c);
    assert!(ink_in(&p, 40, 210, 160, 250) > 50);
    assert_eq!(ink_in(&p, 40, 60, 160, 100), 0);
    c.pointer_up();
    let p = shot(&mut c);
    assert!(ink_in(&p, 40, 210, 160, 250) > 50);
    assert_eq!(ink_in(&p, 40, 60, 160, 100), 0);
}

#[test]
fn hand_moves_and_resizes_text_but_pans_over_ink() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 220.0);
    c.add_text(40.0, 40.0, 120.0, 30.0, "Limits").unwrap();
    c.set_tool(ToolMode::Hand);
    // Ink and empty paper are left to panning.
    assert!(!c.pointer_down(Device::Pen, pen_sample(100.0, 220.0, 0, 0.5), None));
    assert!(!c.pointer_down(Device::Touch, pen_sample(300.0, 150.0, 0, 0.5), None));
    assert_eq!(c.selection_count(), 0);

    let grab = c.view.to_screen(Point::new(100.0, 55.0));
    drag(&mut c, (grab.x, grab.y), (grab.x, grab.y + 50.0));
    let text = &c.document().texts()[0];
    assert_eq!((text.x, text.y), (40.0, 90.0));
    assert_eq!(c.selection_count(), 1, "the moved text stays selected");

    let corner = c.selection_screen_rect().unwrap();
    drag(
        &mut c,
        (corner.x1, corner.y1),
        (corner.x1 + 60.0, corner.y1 + 15.0),
    );
    assert!(
        c.document().texts()[0].scale > 1.2,
        "the corner grip resizes"
    );
    c.undo();
    c.undo();
    let text = &c.document().texts()[0];
    assert_eq!((text.x, text.y, text.scale), (40.0, 40.0, 1.0));
}

#[test]
fn copy_paste_and_delete_selection() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 80.0);
    c.set_tool(ToolMode::Lasso);
    lasso(&mut c, (20.0, 40.0, 180.0, 130.0));
    c.copy_selection();
    c.paste();
    assert_eq!(c.document().strokes().len(), 2);
    assert_eq!(c.selection_count(), 1);
    let pasted = *c.selection.iter().next().unwrap();
    assert_ne!(pasted, c.document().strokes()[0].id);
    c.delete_selection();
    assert_eq!(c.document().strokes().len(), 1);
    assert_eq!(c.selection_count(), 0);
    c.undo();
    assert_eq!(c.document().strokes().len(), 2);
    c.undo();
    assert_eq!(c.document().strokes().len(), 1);
}

fn erase_down(c: &mut Canvas, x: f64) {
    c.pointer_down(Device::PenEraser, pen_sample(x, 60.0, 0, 1.0), None);
    for i in 1..=40 {
        c.pointer_move(&[pen_sample(x, 60.0 + f64::from(i) * 5.0, i * 5, 1.0)]);
    }
    c.pointer_up();
}

#[test]
fn partial_eraser_cuts_strokes_in_one_undo_step() {
    let mut c = canvas();
    draw_wave(&mut c, 50.0, 350.0, 100.0);
    draw_wave(&mut c, 50.0, 350.0, 200.0);
    erase_down(&mut c, 200.0);
    assert_eq!(c.document().strokes().len(), 4, "each wave is cut in two");
    let p = shot(&mut c);
    assert_eq!(ink_in(&p, 194, 0, 206, 300), 0, "gap under the eraser");
    assert!(ink_in(&p, 50, 75, 180, 125) > 50);
    assert!(ink_in(&p, 220, 75, 350, 125) > 50);

    c.undo();
    assert_eq!(c.document().strokes().len(), 2);
    let p = shot(&mut c);
    assert!(ink_in(&p, 194, 75, 206, 125) > 0);
    c.redo();
    assert_eq!(c.document().strokes().len(), 4);
}

fn png(width: u32, height: u32, color: tiny_skia::Color) -> Vec<u8> {
    let mut p = Pixmap::new(width, height).unwrap();
    p.fill(color);
    p.encode_png().unwrap()
}

fn red_in(p: &Pixmap, x: u32, y: u32) -> bool {
    let c = p.pixel(x, y).unwrap();
    c.red() > 200 && c.green() < 80
}

#[test]
fn printout_pages_stack_and_undo_together() {
    let mut c = canvas();
    let page = |shade| png(60, 80, tiny_skia::Color::from_rgba8(shade, 30, 30, 255));
    let pages = (0..3)
        .map(|i| (page(100 + i * 40), Some((600.0, 800.0))))
        .collect();
    assert!(c.insert_pictures(pages, None));
    let images = c.document().images().to_vec();
    assert_eq!(images.len(), 3);
    assert_eq!(c.selection_count(), 3);
    // Shrunk alike to fit the view's width, keeping the pages' shape.
    assert!(
        images
            .iter()
            .all(|i| (i.width / i.height - 0.75).abs() < 1e-6 && i.width < 600.0)
    );
    assert!(
        images
            .windows(2)
            .all(|w| w[1].y > w[0].y + w[0].height && w[1].x == w[0].x)
    );
    c.undo();
    assert!(c.document().images().is_empty());
}

#[test]
fn pictures_move_scale_and_undo() {
    let mut c = canvas();
    c.insert_picture(
        png(100, 50, tiny_skia::Color::from_rgba8(220, 30, 30, 255)),
        None,
    );
    let before = c.document().images()[0].clone();
    drag(&mut c, (200.0, 150.0), (150.0, 100.0));
    let moved = c.document().images()[0].clone();
    assert_eq!((moved.x, moved.y), (before.x - 50.0, before.y - 50.0));
    let frame = c.selection_screen_rect().unwrap();
    let grip = (frame.x1, frame.y1);
    let anchor = (frame.x0, frame.y0);
    let target = (
        anchor.0 + 2.0 * (grip.0 - anchor.0),
        anchor.1 + 2.0 * (grip.1 - anchor.1),
    );
    drag(&mut c, grip, target);
    let scaled = c.document().images()[0].clone();
    assert!((scaled.width / scaled.height - 2.0).abs() < 1e-6);
    assert!((scaled.width - 200.0).abs() < 0.01, "{}", scaled.width);
    let p = shot(&mut c);
    let corner = c
        .view
        .to_screen(Point::new(scaled.x + scaled.width - 5.0, scaled.y + 5.0));
    assert!(red_in(&p, corner.x as u32, corner.y as u32));
    c.undo();
    c.undo();
    assert_eq!(c.document().images()[0], before);
    c.set_tool(ToolMode::Lasso);
    lasso(&mut c, (20.0, 20.0, 380.0, 280.0));
    c.copy_selection();
    c.paste();
    assert_eq!(c.document().images().len(), 2);
    c.delete_selection();
    assert_eq!(c.document().images().len(), 1);
}

#[test]
fn recognized_ink_becomes_text_or_a_formula_in_one_undo_step() {
    let mut c = canvas();
    draw_wave(&mut c, 50.0, 250.0, 100.0);
    c.select_all();
    let (id, corner) = c.replace_ink("x^2", true, &[]).unwrap();
    let math = &c.document().maths()[0];
    assert_eq!(math.id, id);
    assert_eq!(
        corner,
        kurbo::Point::new(math.x + math.width, math.y + math.height)
    );
    assert!(c.document().strokes().is_empty());
    assert_eq!(c.document().maths()[0].source, "x^2");
    c.undo();
    assert_eq!(c.document().strokes().len(), 1);
    assert!(c.document().maths().is_empty());
    c.select_all();
    assert!(c.replace_ink("Grenzwert", false, &[]).is_some());
    assert_eq!(c.document().texts()[0].markdown, "Grenzwert");
}

#[test]
fn moves_stay_on_the_sheets_of_paper_pages() {
    let mut c = canvas();
    c.set_page_settings(PageSettings {
        kind: ductus_core::PageKind::Paper {
            size: ductus_core::PaperSize::A5,
            orientation: ductus_core::Orientation::Portrait,
        },
        ..PageSettings::default()
    });
    c.add_text(40.0, 40.0, 120.0, 30.0, "Limits").unwrap();
    c.set_tool(ToolMode::Hand);
    let grab = c.view.to_screen(Point::new(100.0, 55.0));
    drag(&mut c, (grab.x, grab.y), (grab.x - 200.0, grab.y - 200.0));
    let text = &c.document().texts()[0];
    assert_eq!((text.x, text.y), (0.0, 0.0), "the sheet's top left corner");

    let sheet = c.document().sheets(0)[0];
    let grab = c.view.to_screen(Point::new(60.0, 15.0));
    drag(&mut c, (grab.x, grab.y), (grab.x + 2000.0, grab.y));
    let text = &c.document().texts()[0];
    assert!(
        (text.x + text.width - (sheet.0 + sheet.2)).abs() < 1.0,
        "its right edge stops at the sheet's: {} + {}",
        text.x,
        text.width
    );
}

#[test]
fn the_rotation_grip_turns_ink_and_keeps_boxes_upright() {
    let mut c = canvas();
    draw_wave(&mut c, 60.0, 200.0, 80.0);
    c.add_text(60.0, 130.0, 100.0, 30.0, "Note").unwrap();
    c.set_tool(ToolMode::Lasso);
    c.select_all();
    let before = c.document().strokes()[0].clone();
    let frame = c.selection_screen_rect().unwrap();
    let grip = (frame.center().x, frame.y1 + 26.0 - 6.0);
    let center = (frame.center().x, frame.center().y);
    // A quarter turn: from straight below the center to its right.
    let reach = grip.1 - center.1;
    drag(&mut c, grip, (center.0 + reach, center.1 + 1.0));
    let after = &c.document().strokes()[0];
    let (b0, a0) = (before.points[0], after.points[0]);
    assert!(
        (b0.x - a0.x).abs() > 20.0 || (b0.y - a0.y).abs() > 20.0,
        "the ink turned"
    );
    let text = &c.document().texts()[0];
    assert_eq!(
        (text.width, text.height),
        (100.0, 30.0),
        "the box stays upright"
    );
    c.undo();
    assert_eq!(c.document().strokes()[0].points[0].x, b0.x);
}

#[test]
fn reading_the_page_keeps_ink_that_was_not_read() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 60.0);
    draw_wave(&mut c, 40.0, 160.0, 200.0);
    let [drawing, line] = [0, 1].map(|i| c.document().strokes()[i].id);
    c.select_all();
    let (_, corner) = c.replace_ink("hello", false, &[line]).expect("replaced");
    assert_eq!(c.document().strokes().len(), 1);
    assert_eq!(c.document().strokes()[0].id, drawing);
    let kept = ductus_ink::bounds(&c.document().strokes()[0]);
    let text = &c.document().texts()[0];
    assert!(
        text.y > kept.y1 && corner.y > text.y,
        "placed on the line it replaces"
    );
    c.undo();
    assert_eq!(c.document().strokes().len(), 2);
    assert!(c.document().texts().is_empty());
}

#[test]
fn clipboard_is_shared_between_pages() {
    let mut a = canvas();
    draw_wave(&mut a, 40.0, 160.0, 80.0);
    a.set_tool(ToolMode::Lasso);
    lasso(&mut a, (20.0, 40.0, 180.0, 130.0));
    a.copy_selection();

    let mut b = canvas();
    assert!(b.can_paste(), "the clipboard is shared across pages");
    b.paste();
    assert_eq!(b.document().strokes().len(), 1);
    assert_eq!(b.selection_count(), 1);
}

#[test]
fn flip_mirrors_ink_across_its_center() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 80.0);
    let leftmost = c.document().strokes()[0].points[0].x;
    c.set_tool(ToolMode::Lasso);
    lasso(&mut c, (20.0, 40.0, 180.0, 130.0));
    let center = c.selection_bounds().unwrap().center().x;
    c.flip_selection(true);
    let after = c.document().strokes()[0].points[0].x;
    assert!(
        (after - (2.0 * center - leftmost)).abs() < 1.0,
        "the leftmost point is mirrored to the right: {after}"
    );
}

#[test]
fn rotate_turns_the_selection_a_quarter_turn() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 80.0); // wide and short
    c.set_tool(ToolMode::Lasso);
    lasso(&mut c, (20.0, 40.0, 180.0, 130.0));
    let before = c.selection_bounds().unwrap();
    assert!(before.width() > before.height());
    c.rotate_selection(1);
    let after = c.selection_bounds().unwrap();
    assert!(after.height() > after.width(), "rotated to tall and narrow");
    c.undo();
    let restored = ductus_ink::bounds(&c.document().strokes()[0]);
    assert!(
        (restored.width() - before.width()).abs() < 1.0
            && (restored.height() - before.height()).abs() < 1.0,
        "undo restores the orientation"
    );
}

fn stacking(c: &Canvas) -> Vec<StrokeId> {
    c.document().items().iter().map(|i| i.id()).collect()
}

#[test]
fn layering_puts_ink_above_a_picture_and_undoes() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 80.0);
    let red = tiny_skia::Color::from_rgba8(220, 20, 20, 255);
    assert!(c.insert_picture(png(240, 160, red), Some(Point::new(100.0, 80.0))));
    let covered = |c: &mut Canvas| ink_in(&shot(c), 40, 60, 160, 100) == 0;
    assert!(covered(&mut c), "the picture, added last, lies on the ink");
    c.reorder_selection(3); // send to back
    assert!(
        !covered(&mut c),
        "sent back, the picture lies under the ink"
    );
    c.undo();
    assert!(covered(&mut c));
}

#[test]
fn ink_on_a_text_box_lies_on_a_band_above_it() {
    let mut c = canvas();
    let text = c.add_text(20.0, 40.0, 200.0, 60.0, "note").unwrap();
    assert_eq!(c.bands(), 1, "nothing lies on the text box yet");
    draw_wave(&mut c, 40.0, 160.0, 80.0);
    assert_eq!(c.bands(), 2, "ink written on the box lies above it");
    let z = |c: &Canvas| c.text_views().iter().find(|t| t.id == text).unwrap().z;
    assert!(
        z(&c) > 0.0 && z(&c) < 2.0,
        "the box lies between the page and band 1"
    );
    c.set_tool(ToolMode::Lasso);
    lasso(&mut c, (39.0, 79.0, 39.0, 79.0)); // a tap on the wave, which lies on the box
    let wave = c.document().strokes()[0].id;
    assert_eq!(c.selection_count(), 1);
    c.reorder_selection(3);
    assert_eq!(stacking(&c), [wave, text]);
    assert_eq!(c.bands(), 1, "sent back, the wave lies under the box");
    c.undo();
    assert_eq!(c.bands(), 2);
}

#[test]
fn layering_brings_a_stroke_to_the_front_and_undoes() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 80.0);
    draw_wave(&mut c, 220.0, 360.0, 220.0);
    let bottom = c.document().strokes()[0].id;
    let top = c.document().strokes()[1].id;
    c.set_tool(ToolMode::Lasso);
    lasso(&mut c, (20.0, 40.0, 180.0, 130.0)); // the first wave
    assert_eq!(c.selection_count(), 1);
    c.reorder_selection(0); // bring to front
    assert_eq!(
        stacking(&c),
        [top, bottom],
        "the bottom stroke moves on top"
    );
    c.undo();
    assert_eq!(stacking(&c), [bottom, top], "undo restores the order");
}

#[test]
fn layering_steps_past_the_next_overlapping_item() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 80.0);
    draw_wave(&mut c, 220.0, 360.0, 220.0); // elsewhere on the page
    draw_wave(&mut c, 130.0, 300.0, 100.0); // across the first one's corner
    let [a, b, d] = [0, 1, 2].map(|i| c.document().strokes()[i].id);
    c.set_tool(ToolMode::Lasso);
    lasso(&mut c, (20.0, 40.0, 180.0, 130.0)); // the first wave
    assert_eq!(c.selection_count(), 1);
    let order = stacking;
    c.reorder_selection(1); // bring forward: past the one it overlaps, not just the next one
    assert_eq!(order(&c), [b, d, a]);
    c.reorder_selection(2); // send backward: under the one it overlaps
    assert_eq!(order(&c), [b, a, d]);
    let revision = c.revision();
    c.reorder_selection(2); // nothing else overlaps below: no change, no undo step
    assert_eq!(order(&c), [b, a, d]);
    assert_eq!(c.revision(), revision);
}

#[test]
fn paste_lands_in_view_when_the_copy_is_out_of_it() {
    let mut c = canvas();
    draw_wave(&mut c, 40.0, 160.0, 80.0);
    c.set_tool(ToolMode::Lasso);
    lasso(&mut c, (20.0, 40.0, 180.0, 130.0));
    c.copy_selection();
    c.pan_by(0.0, -2000.0);
    c.paste();
    let pasted = ductus_ink::bounds(&c.document().strokes()[1]);
    let visible = c.view.visible_doc_rect();
    assert!(
        (pasted.center() - visible.center()).hypot() < 1.0,
        "{pasted:?} is centered in {visible:?}"
    );
}

#[test]
fn a_lone_picture_does_not_flip_or_turn() {
    let mut c = canvas();
    assert!(c.insert_picture(png(60, 40, tiny_skia::Color::BLACK), None));
    let revision = c.revision();
    assert!(!c.selection_turns());
    c.flip_selection(true);
    c.rotate_selection(1);
    assert_eq!(c.revision(), revision, "no empty undo steps");
}
