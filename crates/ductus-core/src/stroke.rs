#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InkPoint {
    pub x: f64,
    pub y: f64,
    pub pressure: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub t: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StrokeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Pen,
    Highlighter,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const BLACK: Rgba = Rgba::rgb(0.1, 0.1, 0.12);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinePattern {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl LinePattern {
    pub fn name(self) -> &'static str {
        match self {
            LinePattern::Solid => "solid",
            LinePattern::Dashed => "dashed",
            LinePattern::Dotted => "dotted",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "solid" => Some(LinePattern::Solid),
            "dashed" => Some(LinePattern::Dashed),
            "dotted" => Some(LinePattern::Dotted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeStyle {
    pub tool: Tool,
    pub color: Rgba,
    pub width: f64,
    pub pattern: LinePattern,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            tool: Tool::Pen,
            color: Rgba::BLACK,
            width: 3.0,
            pattern: LinePattern::Solid,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// Shares the id space with strokes, so one selection can hold both.
    pub id: StrokeId,
    pub source: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextBox {
    pub id: StrokeId,
    pub markdown: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

impl TextBox {
    pub const FONT_SIZE: f64 = 16.0;
}

#[derive(Debug, Clone, PartialEq)]
pub struct MathBox {
    pub id: StrokeId,
    pub source: String,
    pub syntax: String,
    pub x: f64,
    pub y: f64,
    /// Stored so selecting and sizing the page needn't lay the formula out.
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub color: Rgba,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub id: StrokeId,
    pub style: StrokeStyle,
    pub points: Vec<InkPoint>,
}
