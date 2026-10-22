//! Handwriting recognition: ink lines found from geometry, read by a vision model.

pub mod backend;
pub mod lines;

pub use backend::{Recognizer, from_settings};
pub use lines::{InkLine, find_lines, line_png};
