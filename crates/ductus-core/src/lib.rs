//! Coordinates are document units: CSS pixels (1/96 inch).

mod document;
mod history;
mod memory;
mod page;
mod stroke;

pub use document::Document;
pub use history::{Command, History};
pub use memory::release_freed_memory;
pub use page::{
    MARGIN_LINE, Orientation, PageBackground, PageKind, PageSettings, PaperSize, PatternStyle,
    SHEET_GAP, SheetRect, UNITS_PER_MM, UNITS_PER_POINT, stack_sheets,
};
pub use stroke::{
    Image, InkPoint, LinePattern, MathBox, Rgba, Stroke, StrokeId, StrokeStyle, TextBox, Tool,
};
