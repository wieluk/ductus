use crate::math::{math_paths, math_rect};
use crate::{Canvas, Gesture, PASTE_OFFSET, SELECTION_COLOR, TileLayer, ToolMode};
use ductus_core::{Command, Image, MathBox, PageKind, Rgba, Stroke, StrokeId, TextBox, Tool};
use ductus_render::{Layer, Picture, StrokePath, Viewport};
use kurbo::{Affine, Point, Rect, Vec2};
use std::rc::Rc;
use tiny_skia::Pixmap;

const FRAME_PAD: f64 = 6.0;
const GRIP_REACH: f64 = 16.0;
const TAP_LENGTH: f64 = 8.0;
const TAP_REACH: f64 = 6.0;
const INSERT_SHARE: f64 = 0.7;
/// Must match ductus_format::library::IMAGE_PREFIX.
const IMAGE_PREFIX: &str = "img-";

pub(crate) struct PictureData {
    bytes: Rc<[u8]>,
    picture: Rc<Picture>,
}

#[derive(Default)]
pub(crate) struct Clipboard {
    strokes: Vec<Stroke>,
    images: Vec<Image>,
    texts: Vec<TextBox>,
    maths: Vec<MathBox>,
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

fn image_rect(image: &Image) -> Rect {
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

impl Canvas {
    pub fn transforming(&self) -> bool {
        matches!(
            self.gesture,
            Some(Gesture::MovingSelection { .. } | Gesture::ScalingSelection { .. })
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

    fn selection_frame(&self) -> Option<Rect> {
        let pad = FRAME_PAD / self.view.zoom;
        let b = self
            .selection_transform
            .transform_rect_bbox(self.selection_bounds()?);
        Some(b.inflate(pad, pad))
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

    /// The hand takes the selection, else the text, formula or picture under it, which it selects.
    pub(crate) fn begin_hand_gesture(&mut self, at: Point) -> Option<Gesture> {
        if let Some(gesture) = self.grab_selection(at) {
            return Some(gesture);
        }
        self.clear_selection();
        let id = self.object_at(at)?;
        self.selection = vec![id];
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

    /// Text lies on top, then ink, then pictures.
    fn item_at(&self, at: Point) -> Option<StrokeId> {
        let reach = TAP_REACH / self.view.zoom;
        self.text_at(at)
            .or_else(|| {
                self.doc
                    .strokes()
                    .iter()
                    .rev()
                    .find(|s| ductus_ink::hits_circle(s, at, reach))
                    .map(|s| s.id)
            })
            .or_else(|| self.math_at(at))
            .or_else(|| self.image_at(at))
    }

    /// What the hand picks up: everything but ink, which it would otherwise grab when panning.
    fn object_at(&self, at: Point) -> Option<StrokeId> {
        self.text_at(at)
            .or_else(|| self.math_at(at))
            .or_else(|| self.image_at(at))
    }

    fn image_at(&self, at: Point) -> Option<StrokeId> {
        self.doc
            .images()
            .iter()
            .rev()
            .find(|i| image_rect(i).contains(at))
            .map(|i| i.id)
    }

    pub(crate) fn select_inside(&mut self, lasso: &[Point]) {
        self.selection = if lasso.len() < 3 {
            Vec::new()
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
        let moved_texts = texts.iter().map(|x| transform_text(x, t)).collect();
        let maths: Vec<MathBox> = self.selected_maths().cloned().collect();
        let moved_maths = maths.iter().map(|m| transform_math(m, t)).collect();
        let moved: Vec<Stroke> = strokes.iter().map(|s| transform_stroke(s, t)).collect();
        for s in &moved {
            self.paths.remove(&s.id);
        }
        let moved_images = images.iter().map(|i| transform_image(i, t)).collect();
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

    pub fn delete_selection(&mut self) {
        if let Some(b) = self.selection_bounds() {
            self.tiles.invalidate(TileLayer::Base, b);
        }
        let cmd = Command::batch([
            Command::remove(&self.doc, &self.selection),
            Command::remove_images(&self.doc, &self.selection),
            Command::remove_texts(&self.doc, &self.selection),
            Command::remove_maths(&self.doc, &self.selection),
        ]);
        if let Some(cmd) = cmd {
            self.history.apply(&mut self.doc, cmd);
            self.sync_layout();
        }
        self.clear_selection();
    }

    pub fn copy_selection(&mut self) {
        self.clipboard = Clipboard {
            strokes: self.selected_strokes().cloned().collect(),
            images: self.selected_images().cloned().collect(),
            texts: self.selected_texts().cloned().collect(),
            maths: self.selected_maths().cloned().collect(),
        };
    }

    pub fn cut_selection(&mut self) {
        self.copy_selection();
        self.delete_selection();
    }

    pub fn can_paste(&self) -> bool {
        !self.clipboard.strokes.is_empty()
            || !self.clipboard.images.is_empty()
            || !self.clipboard.texts.is_empty()
            || !self.clipboard.maths.is_empty()
    }

    pub fn paste(&mut self) {
        if !self.can_paste() {
            return;
        }
        let t = Affine::translate(Vec2::new(PASTE_OFFSET, PASTE_OFFSET));
        let strokes: Vec<Stroke> = std::mem::take(&mut self.clipboard.strokes)
            .iter()
            .map(|s| Stroke {
                id: self.doc.alloc_id(),
                ..transform_stroke(s, t)
            })
            .collect();
        let images: Vec<Image> = std::mem::take(&mut self.clipboard.images)
            .iter()
            .map(|i| Image {
                id: self.doc.alloc_id(),
                ..transform_image(i, t)
            })
            .collect();
        let texts: Vec<TextBox> = std::mem::take(&mut self.clipboard.texts)
            .iter()
            .map(|x| TextBox {
                id: self.doc.alloc_id(),
                ..transform_text(x, t)
            })
            .collect();
        let maths: Vec<MathBox> = std::mem::take(&mut self.clipboard.maths)
            .iter()
            .map(|m| MathBox {
                id: self.doc.alloc_id(),
                ..transform_math(m, t)
            })
            .collect();
        // The next paste lands a step further along.
        self.clipboard = Clipboard {
            strokes: strokes.clone(),
            images: images.clone(),
            texts: texts.clone(),
            maths: maths.clone(),
        };
        self.selection = strokes
            .iter()
            .map(|s| s.id)
            .chain(images.iter().map(|i| i.id))
            .chain(texts.iter().map(|x| x.id))
            .chain(maths.iter().map(|m| m.id))
            .collect();
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
            self.tiles
                .invalidate(TileLayer::Base, ductus_ink::bounds(s));
        }
        self.history
            .apply(&mut self.doc, Command::Replace { before, after });
    }

    pub fn add_picture(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        let Some(picture) = Picture::decode_png(&bytes) else {
            return false;
        };
        self.pictures.insert(
            name.to_owned(),
            PictureData {
                bytes: bytes.into(),
                picture: Rc::new(picture),
            },
        );
        for image in self.doc.images().iter().filter(|i| i.source == name) {
            self.tiles.invalidate(TileLayer::Base, image_rect(image));
        }
        true
    }

    pub fn picture_bytes(&self, name: &str) -> Option<&[u8]> {
        self.pictures.get(name).map(|p| &*p.bytes)
    }

    pub fn insert_picture(&mut self, bytes: Vec<u8>, at: Option<Point>) -> bool {
        let name = picture_name(&bytes);
        if !self.add_picture(&name, bytes) {
            return false;
        }
        let (w, h) = self.pictures[&name].picture.size();
        // Screenshots of high-density screens have more pixels than points.
        let (mut width, mut height) = (f64::from(w) / self.view.dpr, f64::from(h) / self.view.dpr);
        let visible = self.view.visible_doc_rect();
        let mut room = (visible.width(), visible.height());
        if let PageKind::Paper { .. } | PageKind::Pdf = self.doc.settings().kind {
            if let Some(sheet) = self.layout.sheet_rects().first() {
                room.0 = room.0.min(sheet.width());
            }
        }
        let fit = (room.0 * INSERT_SHARE / width)
            .min(room.1 * INSERT_SHARE / height)
            .min(1.0);
        width *= fit;
        height *= fit;
        let center = at.map_or(visible.center(), |p| self.view.to_doc(p));
        let image = Image {
            id: self.doc.alloc_id(),
            source: name,
            x: (center.x - width / 2.0).round(),
            y: (center.y - height / 2.0).round(),
            width,
            height,
        };
        self.tiles.invalidate(TileLayer::Base, image_rect(&image));
        self.selection = vec![image.id];
        self.history
            .apply(&mut self.doc, Command::AddImages(vec![image]));
        self.sync_layout();
        self.tool = ToolMode::Lasso;
        self.redraw_overlay();
        true
    }

    pub(crate) fn render_pictures_into(&self, pixmap: &mut Pixmap, vp: &Viewport) {
        if self.doc.images().is_empty() {
            return;
        }
        let clip = ductus_render::ink_clip(pixmap, vp, &self.layout);
        for image in self.doc.images().iter().filter(|i| !self.is_hidden(i.id)) {
            if let Some(data) = self.pictures.get(&image.source) {
                ductus_render::render_picture(
                    pixmap,
                    vp,
                    &data.picture,
                    image_rect(image),
                    clip.as_ref(),
                );
            }
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
        if let Some(frame) = self.selection_frame() {
            let grips = frame.inflate(10.0 / self.view.zoom, 10.0 / self.view.zoom);
            bounds = Some(bounds.map_or(grips, |b| b.union(grips)));
        }
        bounds.map(|b| b.inflate(pad, pad))
    }

    pub(crate) fn render_overlay(&self, pixmap: &mut Pixmap, vp: &Viewport) {
        pixmap.fill(tiny_skia::Color::TRANSPARENT);
        self.render_mark(pixmap, vp);
        let clip = ductus_render::ink_clip(pixmap, vp, &self.layout);
        if self.transforming() {
            let t = self.selection_transform;
            for image in self.selected_images() {
                if let Some(data) = self.pictures.get(&image.source) {
                    let r = transform_image(image, t);
                    ductus_render::render_picture(
                        pixmap,
                        vp,
                        &data.picture,
                        image_rect(&r),
                        clip.as_ref(),
                    );
                }
            }
            for math in self.selected_maths() {
                let paths = math_paths(&transform_math(math, t));
                ductus_render::render_strokes(pixmap, vp, paths.iter(), clip.as_ref());
            }
            let paths: Vec<StrokePath> = self
                .selected_strokes()
                .filter_map(|s| StrokePath::new(&transform_stroke(s, t)))
                .collect();
            for layer in [Layer::Highlight, Layer::Ink] {
                ductus_render::render_strokes(
                    pixmap,
                    vp,
                    paths.iter().filter(|p| p.layer == layer),
                    clip.as_ref(),
                );
            }
        }
        ductus_render::render_strokes(pixmap, vp, self.active.iter(), clip.as_ref());
        self.render_math_draft(pixmap, vp);
        if let Some(Gesture::Lasso { points }) = &self.gesture {
            ductus_render::render_guide(pixmap, vp, points, false, SELECTION_COLOR);
        }
        if let Some(b) = self.selection_frame() {
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
        }
    }
}
