use crate::math::{math_paths, math_rect};
use crate::{Canvas, Gesture, PASTE_OFFSET, SELECTION_COLOR, TileLayer, ToolMode};
use ductus_core::{Command, Image, MathBox, PageKind, Rgba, Stroke, StrokeId, TextBox, Tool};
use ductus_render::{Layer, Picture, StrokePath, Viewport};
use kurbo::{Affine, Point, Rect, Vec2};
use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use tiny_skia::Pixmap;

const FRAME_PAD: f64 = 6.0;
const GRIP_REACH: f64 = 16.0;
/// How far below the frame the rotation grip sits, in screen pixels.
const ROTATE_GRIP: f64 = 26.0;
const TAP_LENGTH: f64 = 8.0;
const TAP_REACH: f64 = 6.0;
const INSERT_SHARE: f64 = 0.7;
/// PNG bytes and, if not as large as its pixels, a size in document units.
pub type NewPicture = (Vec<u8>, Option<(f64, f64)>);
/// Between the pages of a printout, in document units.
const PICTURE_GAP: f64 = 16.0;
/// Must match ductus_format::library::IMAGE_PREFIX.
const IMAGE_PREFIX: &str = "img-";

pub(crate) struct PictureData {
    bytes: Rc<[u8]>,
    /// Decoded when first drawn, so a page opens without decoding pictures out of sight.
    picture: OnceCell<Option<Picture>>,
}

impl PictureData {
    fn picture(&self) -> Option<&Picture> {
        self.picture
            .get_or_init(|| Picture::decode_png(&self.bytes))
            .as_ref()
    }
}

#[derive(Default, Clone)]
pub(crate) struct Clipboard {
    strokes: Vec<Stroke>,
    images: Vec<Image>,
    texts: Vec<TextBox>,
    maths: Vec<MathBox>,
    /// PNG bytes of the copied pictures, by source name, so they paste onto other pages too.
    pictures: HashMap<String, Rc<[u8]>>,
    /// Where the copied items lay, so pasting stacks them alike.
    stacking: HashMap<StrokeId, (i64, StrokeId)>,
}

impl Clipboard {
    fn bounds(&self) -> Option<Rect> {
        self.strokes
            .iter()
            .map(ductus_ink::bounds)
            .chain(self.images.iter().map(image_rect))
            .chain(self.texts.iter().map(text_rect))
            .chain(self.maths.iter().map(math_rect))
            .reduce(|a, b| a.union(b))
    }
}

fn inside(polygon: &[Point], p: Point) -> bool {
    let mut result = false;
    let n = polygon.len();
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            result = !result;
        }
    }
    result
}

pub(crate) fn image_rect(image: &Image) -> Rect {
    Rect::new(
        image.x,
        image.y,
        image.x + image.width,
        image.y + image.height,
    )
}

pub(crate) fn text_rect(text: &TextBox) -> Rect {
    Rect::new(text.x, text.y, text.x + text.width, text.y + text.height)
}

pub(crate) fn transform_text(text: &TextBox, t: Affine) -> TextBox {
    let r = t.transform_rect_bbox(text_rect(text));
    TextBox {
        x: r.x0,
        y: r.y0,
        width: r.width(),
        height: r.height(),
        scale: text.scale * scale_of(t),
        ..text.clone()
    }
}

fn transform_math(math: &MathBox, t: Affine) -> MathBox {
    let r = t.transform_rect_bbox(math_rect(math));
    MathBox {
        x: r.x0,
        y: r.y0,
        width: r.width(),
        height: r.height(),
        scale: math.scale * scale_of(t),
        ..math.clone()
    }
}

/// Boxes can't be stored turned: under a rotation they move along but stay upright.
fn upright(t: Affine, rect: Rect) -> Affine {
    let [a, b, c, d, ..] = t.as_coeffs();
    if b.abs() < 1e-9 && c.abs() < 1e-9 && a > 0.0 && d > 0.0 {
        return t;
    }
    let center = rect.center();
    Affine::translate((t * center).to_vec2())
        * Affine::scale(scale_of(t))
        * Affine::translate(-center.to_vec2())
}

fn scale_of(t: Affine) -> f64 {
    t.determinant().abs().sqrt()
}

fn transform_stroke(stroke: &Stroke, t: Affine) -> Stroke {
    let mut out = stroke.clone();
    for p in &mut out.points {
        let q = t * Point::new(p.x, p.y);
        p.x = q.x;
        p.y = q.y;
    }
    out.style.width *= scale_of(t);
    out
}

fn transform_image(image: &Image, t: Affine) -> Image {
    let r = t.transform_rect_bbox(image_rect(image));
    Image {
        x: r.x0,
        y: r.y0,
        width: r.width(),
        height: r.height(),
        ..image.clone()
    }
}

/// Named by content (FNV-1a), so the same picture is stored once.
fn picture_name(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{IMAGE_PREFIX}{h:016x}.png")
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1
}

/// New stacking order for layering: 0 bring to front, 1 bring forward,
/// 2 send backward, 3 send to back. Later items lie on top.
fn reorder_order(
    items: &[(StrokeId, Rect)],
    selected: &HashSet<StrokeId>,
    mode: u8,
) -> Vec<StrokeId> {
    let (chosen, rest): (Vec<StrokeId>, Vec<StrokeId>) = items
        .iter()
        .map(|(id, _)| *id)
        .partition(|id| selected.contains(id));
    match mode {
        0 => rest.into_iter().chain(chosen).collect(),
        3 => chosen.into_iter().chain(rest).collect(),
        _ => {
            // A step goes past the next item that overlaps: passing one elsewhere on the page
            // changes nothing to see. The item nearest that end moves first, keeping their order.
            let forward = mode == 1;
            let mut out = items.to_vec();
            let picks: Vec<StrokeId> = if forward {
                chosen.into_iter().rev().collect()
            } else {
                chosen
            };
            for id in picks {
                let Some(i) = out.iter().position(|(o, _)| *o == id) else {
                    continue;
                };
                let rect = out[i].1;
                let past = |&(o, r): &(StrokeId, Rect)| !selected.contains(&o) && overlaps(r, rect);
                let target = if forward {
                    out[i + 1..].iter().position(past).map(|k| i + 1 + k)
                } else {
                    out[..i].iter().rposition(past)
                };
                if let Some(j) = target {
                    let item = out.remove(i);
                    out.insert(j, item);
                }
            }
            out.into_iter().map(|(id, _)| id).collect()
        }
    }
}

impl Canvas {
    pub fn transforming(&self) -> bool {
        matches!(
            self.gesture,
            Some(
                Gesture::MovingSelection { .. }
                    | Gesture::ScalingSelection { .. }
                    | Gesture::RotatingSelection { .. }
            )
        )
    }

    /// Dragged and erased items leave the base layer; the overlay shows them meanwhile.
    pub(crate) fn is_hidden(&self, id: StrokeId) -> bool {
        self.erased.contains(&id)
            || (self.transforming() && self.selection.contains(&id))
            || self.math_draft_target() == Some(id)
    }

    fn selected_strokes(&self) -> impl Iterator<Item = &Stroke> {
        self.doc
            .strokes()
            .iter()
            .filter(|s| self.selection.contains(&s.id))
    }

    pub(crate) fn selected_texts(&self) -> impl Iterator<Item = &TextBox> {
        self.doc
            .texts()
            .iter()
            .filter(|t| self.selection.contains(&t.id))
    }

    fn selected_maths(&self) -> impl Iterator<Item = &MathBox> {
        self.doc
            .maths()
            .iter()
            .filter(|m| self.selection.contains(&m.id))
    }

    fn selected_images(&self) -> impl Iterator<Item = &Image> {
        self.doc
            .images()
            .iter()
            .filter(|i| self.selection.contains(&i.id))
    }

    pub(crate) fn selection_bounds(&self) -> Option<Rect> {
        self.selected_strokes()
            .map(ductus_ink::bounds)
            .chain(self.selected_images().map(image_rect))
            .chain(self.selected_texts().map(text_rect))
            .chain(self.selected_maths().map(math_rect))
            .reduce(|a, b| a.union(b))
    }

    /// Keeps a move on the sheets, which exports cut off at; paper grows downwards, PDFs don't.
    pub(crate) fn clamp_move(&self, delta: Vec2) -> Vec2 {
        let Some(bounds) = self.selection_bounds() else {
            return delta;
        };
        let sheets = self.doc.sheets(1);
        if sheets.is_empty() {
            return delta;
        }
        let left = sheets.iter().map(|s| s.0).fold(f64::INFINITY, f64::min);
        let right = sheets
            .iter()
            .map(|s| s.0 + s.2)
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = sheets.last().map_or(f64::INFINITY, |s| s.1 + s.3);
        let fit = |delta: f64, low: f64, high: f64, min: f64, max: f64| {
            // Wider than the sheets: not clamped.
            if high - low > max - min {
                delta
            } else {
                delta.clamp(min - low, max - high)
            }
        };
        let x = fit(delta.x, bounds.x0, bounds.x1, left, right);
        let y = if matches!(self.doc.settings().kind, PageKind::Pdf) {
            fit(delta.y, bounds.y0, bounds.y1, 0.0, bottom)
        } else {
            delta.y.max(-bounds.y0.max(0.0))
        };
        Vec2::new(x, y)
    }

    fn selection_frame(&self) -> Option<Rect> {
        self.frame_moved(self.selection_transform)
    }

    fn frame_moved(&self, t: Affine) -> Option<Rect> {
        let pad = FRAME_PAD / self.view.zoom;
        let b = t.transform_rect_bbox(self.selection_bounds()?);
        Some(b.inflate(pad, pad))
    }

    /// A plain move shifts the overlay tiles drawn when it began instead of drawing new ones.
    pub(crate) fn overlay_shift(&self) -> Option<Vec2> {
        match self.gesture {
            Some(Gesture::MovingSelection { .. })
                if self.mark.is_none() && self.math_draft.is_none() =>
            {
                Some(self.selection_transform.translation())
            }
            _ => None,
        }
    }

    /// What the overlay is drawn with; a shifted overlay is drawn unmoved.
    fn overlay_transform(&self) -> Affine {
        match self.overlay_shift() {
            Some(_) => Affine::IDENTITY,
            None => self.selection_transform,
        }
    }

    pub fn selection_count(&self) -> usize {
        self.selection.len()
    }

    pub fn selection_has_ink(&self) -> bool {
        self.selected_strokes().any(|s| s.style.tool == Tool::Pen)
    }

    pub fn selection_screen_rect(&self) -> Option<Rect> {
        Some(self.view.to_screen_rect(self.selection_frame()?))
    }

    pub fn clear_selection(&mut self) {
        if !self.selection.is_empty() {
            self.selection.clear();
            self.selection_transform = Affine::IDENTITY;
            self.redraw_overlay();
        }
    }

    pub fn select_all(&mut self) {
        self.selection = self
            .doc
            .strokes()
            .iter()
            .map(|s| s.id)
            .chain(self.doc.images().iter().map(|i| i.id))
            .chain(self.doc.texts().iter().map(|t| t.id))
            .chain(self.doc.maths().iter().map(|m| m.id))
            .collect();
        self.redraw_overlay();
    }

    pub(crate) fn begin_selection_gesture(&mut self, at: Point) -> Gesture {
        if let Some(gesture) = self.grab_selection(at) {
            return gesture;
        }
        self.clear_selection();
        Gesture::Lasso { points: vec![at] }
    }

    /// The hand takes the selection, else selects the box under it.
    pub(crate) fn begin_hand_gesture(&mut self, at: Point) -> Option<Gesture> {
        if let Some(gesture) = self.grab_selection(at) {
            return Some(gesture);
        }
        self.clear_selection();
        let id = self.object_at(at)?;
        self.selection = HashSet::from([id]);
        if let Some(bounds) = self.selection_bounds() {
            self.tiles.invalidate(TileLayer::Base, bounds);
        }
        Some(Gesture::MovingSelection { start: at })
    }

    /// A drag on a corner grip scales the selection; inside its frame, it moves.
    fn grab_selection(&mut self, at: Point) -> Option<Gesture> {
        if let (Some(frame), Some(bounds)) = (self.selection_frame(), self.selection_bounds()) {
            let reach = GRIP_REACH / self.view.zoom;
            let corners = [
                (
                    Point::new(frame.x0, frame.y0),
                    Point::new(frame.x1, frame.y1),
                ),
                (
                    Point::new(frame.x1, frame.y0),
                    Point::new(frame.x0, frame.y1),
                ),
                (
                    Point::new(frame.x1, frame.y1),
                    Point::new(frame.x0, frame.y0),
                ),
                (
                    Point::new(frame.x0, frame.y1),
                    Point::new(frame.x1, frame.y0),
                ),
            ];
            let turn = Point::new(frame.center().x, frame.y1 + ROTATE_GRIP / self.view.zoom);
            if (at - turn).hypot() <= reach {
                self.tiles.invalidate(TileLayer::Base, bounds);
                return Some(Gesture::RotatingSelection {
                    center: bounds.center(),
                    start: at,
                });
            }
            let grip = corners
                .into_iter()
                .find(|(corner, _)| (at - *corner).hypot() <= reach);
            if grip.is_some() || frame.contains(at) {
                // The selection moves from the base layer to the overlay.
                self.tiles.invalidate(TileLayer::Base, bounds);
                return Some(match grip {
                    Some((start, anchor)) => Gesture::ScalingSelection { anchor, start },
                    None => Gesture::MovingSelection { start: at },
                });
            }
        }
        None
    }

    pub(crate) fn finish_lasso(&mut self, points: &[Point]) {
        let length: f64 = points.windows(2).map(|w| (w[1] - w[0]).hypot()).sum();
        if length * self.view.zoom < TAP_LENGTH {
            if let Some(&at) = points.first() {
                self.selection = self.item_at(at).into_iter().collect();
            }
            self.redraw_overlay();
        } else {
            self.select_inside(points);
        }
    }

    /// The topmost item there.
    fn item_at(&self, at: Point) -> Option<StrokeId> {
        let reach = TAP_REACH / self.view.zoom;
        let ink = self
            .doc
            .strokes()
            .iter()
            .filter(|s| ductus_ink::hits_circle(s, at, reach))
            .map(|s| s.id);
        self.objects_at(at)
            .chain(ink)
            .max_by_key(|id| self.doc.stack_key(*id))
    }

    /// What the hand picks up: everything but ink, which it would otherwise grab when panning.
    fn object_at(&self, at: Point) -> Option<StrokeId> {
        self.objects_at(at).max_by_key(|id| self.doc.stack_key(*id))
    }

    fn objects_at(&self, at: Point) -> impl Iterator<Item = StrokeId> {
        self.text_at(at)
            .into_iter()
            .chain(self.math_at(at))
            .chain(self.image_at(at))
    }

    fn image_at(&self, at: Point) -> Option<StrokeId> {
        self.doc
            .images()
            .iter()
            .filter(|i| image_rect(i).contains(at))
            .map(|i| i.id)
            .max_by_key(|id| self.doc.stack_key(*id))
    }

    pub(crate) fn select_inside(&mut self, lasso: &[Point]) {
        self.selection = if lasso.len() < 3 {
            HashSet::new()
        } else {
            let strokes = self.doc.strokes().iter().filter(|s| {
                let hits = s
                    .points
                    .iter()
                    .filter(|p| inside(lasso, Point::new(p.x, p.y)))
                    .count();
                hits * 2 > s.points.len()
            });
            let images = self
                .doc
                .images()
                .iter()
                .filter(|i| inside(lasso, image_rect(i).center()));
            let texts = self
                .doc
                .texts()
                .iter()
                .filter(|t| inside(lasso, text_rect(t).center()));
            let maths = self
                .doc
                .maths()
                .iter()
                .filter(|m| inside(lasso, math_rect(m).center()));
            strokes
                .map(|s| s.id)
                .chain(images.map(|i| i.id))
                .chain(texts.map(|t| t.id))
                .chain(maths.map(|m| m.id))
                .collect()
        };
        self.redraw_overlay();
    }

    pub(crate) fn finish_transform(&mut self) {
        let t = std::mem::replace(&mut self.selection_transform, Affine::IDENTITY);
        self.apply_transform(t);
    }

    fn apply_transform(&mut self, t: Affine) {
        let Some(bounds) = self.selection_bounds() else {
            return;
        };
        self.tiles.invalidate(TileLayer::Base, bounds);
        if t == Affine::IDENTITY {
            self.redraw_overlay();
            return;
        }
        self.tiles.invalidate(
            TileLayer::Base,
            t.transform_rect_bbox(bounds).inflate(4.0, 4.0),
        );
        let strokes: Vec<Stroke> = self.selected_strokes().cloned().collect();
        let images: Vec<Image> = self.selected_images().cloned().collect();
        let texts: Vec<TextBox> = self.selected_texts().cloned().collect();
        let moved_texts = texts
            .iter()
            .map(|x| transform_text(x, upright(t, text_rect(x))))
            .collect();
        let maths: Vec<MathBox> = self.selected_maths().cloned().collect();
        let moved_maths = maths
            .iter()
            .map(|m| transform_math(m, upright(t, math_rect(m))))
            .collect();
        let moved: Vec<Stroke> = strokes.iter().map(|s| transform_stroke(s, t)).collect();
        for s in &moved {
            self.paths.remove(&s.id);
            self.outlined = None;
        }
        let moved_images = images
            .iter()
            .map(|i| transform_image(i, upright(t, image_rect(i))))
            .collect();
        let cmd = Command::batch([
            (!strokes.is_empty()).then_some(Command::Replace {
                before: strokes,
                after: moved,
            }),
            (!images.is_empty()).then_some(Command::ReplaceImages {
                before: images,
                after: moved_images,
            }),
            (!texts.is_empty()).then_some(Command::ReplaceTexts {
                before: texts,
                after: moved_texts,
            }),
            (!maths.is_empty()).then_some(Command::ReplaceMaths {
                before: maths,
                after: moved_maths,
            }),
        ]);
        if let Some(cmd) = cmd {
            self.history.apply(&mut self.doc, cmd);
            self.sync_layout();
        }
        self.redraw_overlay();
    }

    /// Flipping or turning changes something: ink turns, or several items trade places.
    /// Pictures and boxes stay upright, so one alone stays as it is.
    pub fn selection_turns(&self) -> bool {
        self.selected_strokes().next().is_some() || self.selection.len() > 1
    }

    pub fn flip_selection(&mut self, horizontal: bool) {
        if !self.selection_turns() {
            return;
        }
        let Some(bounds) = self.selection_bounds() else {
            return;
        };
        let center = bounds.center();
        let t = if horizontal {
            Affine::translate(Vec2::new(2.0 * center.x, 0.0)) * Affine::scale_non_uniform(-1.0, 1.0)
        } else {
            Affine::translate(Vec2::new(0.0, 2.0 * center.y)) * Affine::scale_non_uniform(1.0, -1.0)
        };
        self.apply_transform(t);
    }

    pub fn rotate_selection(&mut self, quarter_turns: i32) {
        if !self.selection_turns() {
            return;
        }
        let Some(bounds) = self.selection_bounds() else {
            return;
        };
        let center = bounds.center().to_vec2();
        let angle = std::f64::consts::FRAC_PI_2 * f64::from(quarter_turns);
        let t = Affine::translate(center) * Affine::rotate(angle) * Affine::translate(-center);
        self.apply_transform(t);
    }

    /// Moves the selection up or down the one stack every kind of item shares.
    pub fn reorder_selection(&mut self, mode: u8) {
        if mode > 3 {
            return;
        }
        let items: Vec<(StrokeId, Rect)> = self
            .stack()
            .entries
            .iter()
            .map(|e| (e.id, e.rect))
            .collect();
        let order = reorder_order(&items, &self.selection, mode);
        if order.iter().eq(items.iter().map(|(id, _)| id)) {
            return;
        }
        let Some(cmd) = Command::restack(&self.doc, &order) else {
            return;
        };
        if let Some(b) = self.selection_bounds() {
            self.tiles.invalidate(TileLayer::Base, b.inflate(4.0, 4.0));
        }
        self.history.apply(&mut self.doc, cmd);
        self.redraw_overlay();
    }

    pub fn delete_selection(&mut self) {
        if let Some(b) = self.selection_bounds() {
            self.tiles.invalidate(TileLayer::Base, b);
        }
        let ids: Vec<StrokeId> = self.selection.iter().copied().collect();
        let cmd = Command::batch([
            Command::remove(&self.doc, &ids),
            Command::remove_images(&self.doc, &ids),
            Command::remove_texts(&self.doc, &ids),
            Command::remove_maths(&self.doc, &ids),
        ]);
        if let Some(cmd) = cmd {
            self.history.apply(&mut self.doc, cmd);
            self.sync_layout();
        }
        self.clear_selection();
    }

    pub fn copy_selection(&mut self) {
        let strokes = self.selected_strokes().cloned().collect();
        let images: Vec<Image> = self.selected_images().cloned().collect();
        let pictures = images
            .iter()
            .filter_map(|i| {
                self.pictures
                    .get(&i.source)
                    .map(|p| (i.source.clone(), p.bytes.clone()))
            })
            .collect();
        let texts = self.selected_texts().cloned().collect();
        let maths = self.selected_maths().cloned().collect();
        let stacking = self
            .selection
            .iter()
            .map(|id| (*id, self.doc.stack_key(*id)))
            .collect();
        *self.clipboard.borrow_mut() = Clipboard {
            strokes,
            images,
            texts,
            maths,
            pictures,
            stacking,
        };
    }

    pub fn cut_selection(&mut self) {
        self.copy_selection();
        self.delete_selection();
    }

    pub fn can_paste(&self) -> bool {
        let clipboard = self.clipboard.borrow();
        !clipboard.strokes.is_empty()
            || !clipboard.images.is_empty()
            || !clipboard.texts.is_empty()
            || !clipboard.maths.is_empty()
    }

    pub fn paste(&mut self) {
        if !self.can_paste() {
            return;
        }
        let clipboard = std::mem::take(&mut *self.clipboard.borrow_mut());
        let visible = self.view.visible_doc_rect();
        // Copied out of view (further down, or on another page), the paste lands in view.
        let t = Affine::translate(match clipboard.bounds() {
            Some(b) if !overlaps(b, visible) => visible.center() - b.center(),
            _ => Vec2::new(PASTE_OFFSET, PASTE_OFFSET),
        });
        let strokes: Vec<Stroke> = clipboard
            .strokes
            .iter()
            .map(|s| Stroke {
                id: self.doc.alloc_id(),
                ..transform_stroke(s, t)
            })
            .collect();
        let images: Vec<Image> = clipboard
            .images
            .iter()
            .map(|i| Image {
                id: self.doc.alloc_id(),
                ..transform_image(i, t)
            })
            .collect();
        let texts: Vec<TextBox> = clipboard
            .texts
            .iter()
            .map(|x| TextBox {
                id: self.doc.alloc_id(),
                ..transform_text(x, t)
            })
            .collect();
        let maths: Vec<MathBox> = clipboard
            .maths
            .iter()
            .map(|m| MathBox {
                id: self.doc.alloc_id(),
                ..transform_math(m, t)
            })
            .collect();
        // The pasted pictures render here, on whatever page receives the paste.
        for (name, bytes) in &clipboard.pictures {
            if !self.pictures.contains_key(name) {
                self.add_picture(name, bytes.to_vec());
            }
        }
        let copied = clipboard
            .strokes
            .iter()
            .map(|s| s.id)
            .chain(clipboard.images.iter().map(|i| i.id))
            .chain(clipboard.texts.iter().map(|x| x.id))
            .chain(clipboard.maths.iter().map(|m| m.id));
        let pasted = strokes
            .iter()
            .map(|s| s.id)
            .chain(images.iter().map(|i| i.id))
            .chain(texts.iter().map(|x| x.id))
            .chain(maths.iter().map(|m| m.id));
        let mut stacked: Vec<((i64, StrokeId), StrokeId)> = copied
            .zip(pasted)
            .map(|(old, new)| {
                (
                    clipboard.stacking.get(&old).copied().unwrap_or((0, old)),
                    new,
                )
            })
            .collect();
        stacked.sort_by_key(|(key, _)| *key);
        let order: Vec<StrokeId> = stacked.iter().map(|(_, id)| *id).collect();
        // The next paste lands a step further along.
        *self.clipboard.borrow_mut() = Clipboard {
            strokes: strokes.clone(),
            images: images.clone(),
            texts: texts.clone(),
            maths: maths.clone(),
            pictures: clipboard.pictures,
            stacking: order
                .iter()
                .enumerate()
                .map(|(z, id)| (*id, (z as i64, *id)))
                .collect(),
        };
        self.doc.stack_new_on_top(&order);
        self.selection = order.into_iter().collect();
        for r in strokes
            .iter()
            .map(ductus_ink::bounds)
            .chain(images.iter().map(image_rect))
            .chain(maths.iter().map(math_rect))
        {
            self.tiles.invalidate(TileLayer::Base, r);
        }
        let cmd = Command::batch([
            (!strokes.is_empty()).then_some(Command::AddMany(strokes)),
            (!images.is_empty()).then_some(Command::AddImages(images)),
            (!texts.is_empty()).then_some(Command::AddTexts(texts)),
            (!maths.is_empty()).then_some(Command::AddMaths(maths)),
        ]);
        if let Some(cmd) = cmd {
            self.history.apply(&mut self.doc, cmd);
            self.sync_layout();
        }
        self.redraw_overlay();
    }

    pub fn recolor_selection(&mut self, color: Rgba) {
        let before: Vec<Stroke> = self
            .selected_strokes()
            .filter(|s| s.style.tool == Tool::Pen && s.style.color != color)
            .cloned()
            .collect();
        if before.is_empty() {
            return;
        }
        let after: Vec<Stroke> = before
            .iter()
            .map(|s| {
                let mut c = s.clone();
                c.style.color = color;
                c
            })
            .collect();
        for s in &after {
            self.paths.remove(&s.id);
            self.outlined = None;
            self.tiles
                .invalidate(TileLayer::Base, ductus_ink::bounds(s));
        }
        self.history
            .apply(&mut self.doc, Command::Replace { before, after });
    }

    /// PNG bytes the page's images show; ones that don't decode show nothing.
    pub fn add_picture(&mut self, name: &str, bytes: Vec<u8>) {
        self.pictures.insert(
            name.to_owned(),
            PictureData {
                bytes: bytes.into(),
                picture: OnceCell::new(),
            },
        );
        for image in self.doc.images().iter().filter(|i| i.source == name) {
            self.tiles.invalidate(TileLayer::Base, image_rect(image));
        }
    }

    pub fn picture_bytes(&self, name: &str) -> Option<&[u8]> {
        self.pictures.get(name).map(|p| &*p.bytes)
    }

    pub fn insert_picture(&mut self, bytes: Vec<u8>, at: Option<Point>) -> bool {
        self.insert_pictures(vec![(bytes, None)], at)
    }

    /// Pictures one below the other (the pages of a PDF printout), as one step to undo.
    pub fn insert_pictures(&mut self, pictures: Vec<NewPicture>, at: Option<Point>) -> bool {
        let mut placed = Vec::new();
        for (bytes, size) in pictures {
            let name = picture_name(&bytes);
            self.add_picture(&name, bytes);
            let Some((w, h)) = self.pictures[&name].picture().map(Picture::size) else {
                self.pictures.remove(&name);
                continue;
            };
            // Screenshots of high-density screens have more pixels than points.
            let size = size.unwrap_or((f64::from(w) / self.view.dpr, f64::from(h) / self.view.dpr));
            placed.push((name, size));
        }
        if placed.is_empty() {
            return false;
        }
        let visible = self.view.visible_doc_rect();
        let mut room = (visible.width(), visible.height());
        if let PageKind::Paper { .. } | PageKind::Pdf = self.doc.settings().kind
            && let Some(sheet) = self.layout.sheet_rects().first()
        {
            room.0 = room.0.min(sheet.width());
        }
        let widest = placed.iter().map(|(_, (w, _))| *w).fold(0.0, f64::max);
        let total: f64 = placed.iter().map(|(_, (_, h))| *h).sum::<f64>()
            + PICTURE_GAP * (placed.len() - 1) as f64;
        // A single picture fits the view; pages run on downwards.
        let tall = if placed.len() == 1 {
            room.1 * INSERT_SHARE / total
        } else {
            f64::INFINITY
        };
        let fit = (room.0 * INSERT_SHARE / widest).min(tall).min(1.0);
        let center = at.map_or(visible.center(), |p| self.view.to_doc(p));
        let mut y = if placed.len() == 1 {
            center.y - total * fit / 2.0
        } else {
            (center.y - total * fit / 2.0).max(visible.y0 + PICTURE_GAP)
        };
        let mut images = Vec::new();
        for (name, (w, h)) in placed {
            let (width, height) = (w * fit, h * fit);
            images.push(Image {
                id: self.doc.alloc_id(),
                source: name,
                x: (center.x - width / 2.0).round(),
                y: y.round(),
                width,
                height,
            });
            y += height + PICTURE_GAP * fit;
        }
        for image in &images {
            self.tiles.invalidate(TileLayer::Base, image_rect(image));
        }
        self.selection = images.iter().map(|i| i.id).collect();
        self.history
            .apply(&mut self.doc, Command::AddImages(images));
        self.sync_layout();
        self.tool = ToolMode::Lasso;
        self.redraw_overlay();
        true
    }

    pub(crate) fn render_picture(
        &self,
        id: StrokeId,
        pixmap: &mut Pixmap,
        vp: &Viewport,
        clip: Option<&tiny_skia::Mask>,
    ) {
        let Some(image) = self.doc.image_index_of(id).map(|i| &self.doc.images()[i]) else {
            return;
        };
        if let Some(picture) = self
            .pictures
            .get(&image.source)
            .and_then(PictureData::picture)
        {
            ductus_render::render_picture(pixmap, vp, picture, image_rect(image), clip);
        }
    }

    pub(crate) fn overlay_bounds(&self) -> Option<Rect> {
        let pad = 4.0 / self.view.zoom;
        let mut bounds = self.active.as_ref().map(|a| a.bounds);
        if let Some(Gesture::Lasso { points }) = &self.gesture {
            let lasso = points.iter().fold(None::<Rect>, |r, p| {
                Some(r.map_or(Rect::from_points(*p, *p), |r| r.union_pt(*p)))
            });
            bounds = match (bounds, lasso) {
                (Some(a), Some(b)) => Some(a.union(b)),
                (a, b) => a.or(b),
            };
        }
        if let Some(mark) = self.mark_bounds() {
            bounds = Some(bounds.map_or(mark, |b| b.union(mark)));
        }
        if let Some(draft) = self.math_draft_bounds() {
            bounds = Some(bounds.map_or(draft, |b| b.union(draft)));
        }
        if let Some(frame) = self.frame_moved(self.overlay_transform()) {
            let grips = frame.inflate(10.0 / self.view.zoom, 10.0 / self.view.zoom);
            bounds = Some(bounds.map_or(grips, |b| b.union(grips)));
        }
        bounds.map(|b| b.inflate(pad, pad))
    }

    pub(crate) fn render_overlay(&self, pixmap: &mut Pixmap, vp: &Viewport) {
        pixmap.fill(tiny_skia::Color::TRANSPARENT);
        self.render_mark(pixmap, vp);
        let clip = ductus_render::ink_clip(pixmap, vp, &self.layout);
        let t = self.overlay_transform();
        if self.transforming() {
            for image in self.selected_images() {
                if let Some(picture) = self
                    .pictures
                    .get(&image.source)
                    .and_then(PictureData::picture)
                {
                    let r = transform_image(image, t);
                    ductus_render::render_picture(
                        pixmap,
                        vp,
                        picture,
                        image_rect(&r),
                        clip.as_ref(),
                    );
                }
            }
            for math in self.selected_maths() {
                let paths = math_paths(&transform_math(math, t));
                ductus_render::render_strokes(pixmap, vp, paths.iter(), clip.as_ref());
            }
            // The cached outlines, moved: outlining every selected stroke per tile was too slow.
            let paths: Vec<&StrokePath> = self
                .selected_strokes()
                .filter_map(|s| self.paths.get(&s.id)?.built())
                .collect();
            for layer in [Layer::Highlight, Layer::Ink] {
                ductus_render::render_strokes_moved(
                    pixmap,
                    vp,
                    paths.iter().copied().filter(|p| p.layer == layer),
                    t,
                    clip.as_ref(),
                );
            }
        }
        ductus_render::render_strokes(pixmap, vp, self.active.iter(), clip.as_ref());
        self.render_math_draft(pixmap, vp);
        if let Some(Gesture::Lasso { points }) = &self.gesture {
            ductus_render::render_guide(pixmap, vp, points, false, SELECTION_COLOR);
        }
        if let Some(b) = self.frame_moved(t) {
            let corners = [
                Point::new(b.x0, b.y0),
                Point::new(b.x1, b.y0),
                Point::new(b.x1, b.y1),
                Point::new(b.x0, b.y1),
            ];
            ductus_render::render_guide(pixmap, vp, &corners, true, SELECTION_COLOR);
            for corner in corners {
                ductus_render::render_handle(pixmap, vp, corner, SELECTION_COLOR);
            }
            // The rotation grip, on a short stem below the frame.
            let bottom = Point::new(b.center().x, b.y1);
            let turn = Point::new(b.center().x, b.y1 + ROTATE_GRIP / self.view.zoom);
            ductus_render::render_guide(pixmap, vp, &[bottom, turn], false, SELECTION_COLOR);
            ductus_render::render_handle(pixmap, vp, turn, SELECTION_COLOR);
        }
    }
}
