//! Handwriting recognition: ink lines found from geometry, read by a vision model.

pub mod backend;
pub mod builtin;
pub mod lines;
mod model;

pub use backend::{Recognizer, from_settings};
pub use lines::{InkLine, find_lines, ink_png, line_png};
