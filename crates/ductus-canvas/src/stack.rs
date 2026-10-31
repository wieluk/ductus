//! The frontend draws text boxes above the canvas, so drawn items that lie on a text box go on
//! a sheet of their own above it. Bands number those sheets: band 0 is the page itself.

use crate::Canvas;
use crate::math::math_rect;
use crate::selection::{image_rect, text_rect};
use ductus_core::{Document, ItemRef, Stroke, StrokeId};
use kurbo::Rect;
use std::cell::Ref;
use std::collections::HashMap;

/// More sheets than this share the top one; a page would need text and pictures piled this high.
pub const MAX_BAND: u8 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Stroke,
    Image,
    Text,
    Math,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Entry {
    pub id: StrokeId,
    pub kind: Kind,
    pub rect: Rect,
    /// For a text box: the band it lies above.
    pub band: u8,
}

#[derive(Debug, Default)]
pub(crate) struct Stack {
    version: Option<u64>,
    /// Bottom first.
    pub entries: Vec<Entry>,
    /// Where each band's items lie, by band; band 0 is the whole page.
    pub bounds: Vec<Rect>,
}

impl Stack {
    fn build(doc: &Document, stroke_rect: impl Fn(&Stroke) -> Rect) -> Self {
        let lifts = !doc.texts().is_empty();
        let mut entries = Vec::new();
        // What can lift items stacked on it: text boxes, and items already lifted.
        let mut lifting: Vec<(Rect, u8, bool)> = Vec::new();
        for item in doc.items() {
            let (kind, rect) = match item {
                ItemRef::Stroke(s) => (Kind::Stroke, stroke_rect(s)),
                ItemRef::Image(i) => (Kind::Image, image_rect(i)),
                ItemRef::Text(t) => (Kind::Text, text_rect(t)),
                ItemRef::Math(m) => (Kind::Math, math_rect(m)),
            };
            let text = kind == Kind::Text;
            let band = if lifts {
                lifting
                    .iter()
                    .filter(|(r, _, _)| ductus_render::overlaps(*r, rect))
                    .map(|&(_, band, below_text)| band + u8::from(below_text && !text))
                    .max()
                    .unwrap_or(0)
                    .min(MAX_BAND)
            } else {
                0
            };
            if text || band > 0 {
                lifting.push((rect, band, text));
            }
            entries.push(Entry {
                id: item.id(),
                kind,
                rect,
                band,
            });
        }
        let mut bounds = vec![Rect::ZERO];
        for e in entries
            .iter()
            .filter(|e| e.kind != Kind::Text && e.band > 0)
        {
            let band = usize::from(e.band);
            if bounds.len() <= band {
                bounds.resize(band + 1, Rect::ZERO);
            }
            bounds[band] = if bounds[band] == Rect::ZERO {
                e.rect
            } else {
                bounds[band].union(e.rect)
            };
        }
        Self {
            version: Some(doc.version()),
            entries,
            bounds,
        }
    }

    pub fn bands(&self) -> usize {
        self.bounds.len()
    }

    /// Lifted items with where they lie: what the band sheets show.
    pub fn lifted(&self) -> Vec<(StrokeId, u8, Rect)> {
        self.entries
            .iter()
            .filter(|e| e.kind != Kind::Text && e.band > 0)
            .map(|e| (e.id, e.band, e.rect))
            .collect()
    }

    /// The stacking order of text boxes for the frontend: between band `n` (at 2n) and band n+1.
    pub fn text_z(&self) -> HashMap<StrokeId, f64> {
        let texts: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|e| e.kind == Kind::Text)
            .collect();
        let step = 1.0 / (texts.len() + 1) as f64;
        texts
            .iter()
            .enumerate()
            .map(|(rank, e)| {
                (
                    e.id,
                    2.0 * f64::from(e.band) + 1.0 + (rank + 1) as f64 * step,
                )
            })
            .collect()
    }
}

impl Canvas {
    pub(crate) fn stack(&self) -> Ref<'_, Stack> {
        if self.stack.borrow().version != Some(self.doc.version()) {
            let built = Stack::build(&self.doc, |s| {
                self.paths
                    .get(&s.id)
                    .map_or_else(|| crate::Outline::reach(s), crate::Outline::bounds)
            });
            *self.stack.borrow_mut() = built;
        }
        self.stack.borrow()
    }

    /// Sheets the frontend stacks between text boxes, the page included.
    pub fn bands(&self) -> usize {
        self.stack().bands()
    }

    /// Redraws where an item changed band since the tiles were drawn.
    pub(crate) fn follow_bands(&mut self) {
        let (lifted, bands) = {
            let stack = self.stack();
            (stack.lifted(), stack.bands())
        };
        if lifted == self.drawn_lifted && bands == self.drawn_bands {
            return;
        }
        let before: HashMap<StrokeId, (u8, Rect)> = self
            .drawn_lifted
            .iter()
            .map(|&(id, band, rect)| (id, (band, rect)))
            .collect();
        let after: HashMap<StrokeId, (u8, Rect)> = lifted
            .iter()
            .map(|&(id, band, rect)| (id, (band, rect)))
            .collect();
        for id in before.keys().chain(after.keys()) {
            let (was, is) = (before.get(id), after.get(id));
            if was.map(|w| w.0) != is.map(|i| i.0) {
                for (_, rect) in was.into_iter().chain(is) {
                    self.tiles
                        .invalidate(crate::TileLayer::Base, rect.inflate(2.0, 2.0));
                }
            }
        }
        if bands < self.drawn_bands {
            self.tiles.drop_bands_from(bands);
        }
        self.drawn_lifted = lifted;
        self.drawn_bands = bands;
    }
}
