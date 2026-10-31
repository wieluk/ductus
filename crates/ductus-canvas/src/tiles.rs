//! Tiles are shown scaled while zooming and re-rendered sharp once the view settles.

use kurbo::Rect;
use std::collections::HashMap;
use tiny_skia::Pixmap;

pub const TILE_SIZE: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TileLayer {
    /// The page and the items on it.
    Base,
    /// Items lifted above text boxes (see `stack`): transparent, numbered from 1.
    Band(u8),
    /// What is being drawn, moved or selected.
    Overlay,
}

impl TileLayer {
    /// Redrawing the page redraws the bands too.
    fn covers(self, key: TileLayer) -> bool {
        self == key || (self == TileLayer::Base && matches!(key, TileLayer::Band(_)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey {
    pub layer: TileLayer,
    pub x: i32,
    pub y: i32,
}

pub(crate) struct Tile {
    pub pixmap: Pixmap,
    pub generation: u64,
    pub dirty: bool,
}

pub(crate) struct TileCache {
    pub scale: f64,
    pub tiles: HashMap<TileKey, Tile>,
    next_generation: u64,
}

impl TileCache {
    pub fn new(scale: f64) -> Self {
        Self {
            scale,
            tiles: HashMap::new(),
            next_generation: 1,
        }
    }

    pub fn tile_doc_rect(&self, x: i32, y: i32) -> Rect {
        let size = f64::from(TILE_SIZE) / self.scale;
        Rect::new(
            f64::from(x) * size,
            f64::from(y) * size,
            f64::from(x + 1) * size,
            f64::from(y + 1) * size,
        )
    }

    pub fn range(&self, doc: Rect) -> (i32, i32, i32, i32) {
        let size = f64::from(TILE_SIZE) / self.scale;
        (
            (doc.x0 / size).floor() as i32,
            (doc.y0 / size).floor() as i32,
            ((doc.x1 / size).ceil() as i32 - 1).max((doc.x0 / size).floor() as i32),
            ((doc.y1 / size).ceil() as i32 - 1).max((doc.y0 / size).floor() as i32),
        )
    }

    pub fn next_generation(&mut self) -> u64 {
        self.next_generation += 1;
        self.next_generation
    }

    pub fn invalidate(&mut self, layer: TileLayer, doc: Rect) {
        let size = f64::from(TILE_SIZE) / self.scale;
        for (key, tile) in &mut self.tiles {
            if !layer.covers(key.layer) {
                continue;
            }
            let r = Rect::new(
                f64::from(key.x) * size,
                f64::from(key.y) * size,
                f64::from(key.x + 1) * size,
                f64::from(key.y + 1) * size,
            );
            if ductus_render::overlaps(r, doc) {
                tile.dirty = true;
            }
        }
    }

    pub fn invalidate_layer(&mut self, layer: TileLayer) {
        for (key, tile) in &mut self.tiles {
            if layer.covers(key.layer) {
                tile.dirty = true;
            }
        }
    }

    pub fn drop_layer(&mut self, layer: TileLayer) {
        self.tiles.retain(|key, _| !layer.covers(key.layer));
    }

    pub fn drop_bands_from(&mut self, first: usize) {
        self.tiles
            .retain(|key, _| !matches!(key.layer, TileLayer::Band(b) if usize::from(b) >= first));
    }

    pub fn rescale(&mut self, scale: f64) {
        self.scale = scale;
        self.tiles.clear();
    }

    pub fn evict_outside(&mut self, layer: TileLayer, range: (i32, i32, i32, i32), margin: i32) {
        self.tiles
            .retain(|key, _| key.layer != layer || within(range, margin, key.x, key.y));
    }
}

pub(crate) fn within((x0, y0, x1, y1): (i32, i32, i32, i32), margin: i32, x: i32, y: i32) -> bool {
    x >= x0 - margin && x <= x1 + margin && y >= y0 - margin && y <= y1 + margin
}
