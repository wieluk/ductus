//! Handwriting recognition: ink lines found from geometry, read by built-in ONNX models or a vision model.

pub mod backend;
pub mod builtin;
mod formula;
mod image;
pub mod lines;
mod runtime;
mod text;

pub use backend::{Recognizer, from_settings};
pub use lines::{InkLine, find_lines, ink_png, line_png};
pub use runtime::{set_processor_share, threads};
