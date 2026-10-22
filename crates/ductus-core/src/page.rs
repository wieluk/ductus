use crate::Rgba;

pub const UNITS_PER_MM: f64 = 96.0 / 25.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaperSize {
    #[default]
    A4,
    A5,
    Letter,
}

impl PaperSize {
    pub const ALL: [PaperSize; 3] = [PaperSize::A4, PaperSize::A5, PaperSize::Letter];

    pub fn mm(self) -> (f64, f64) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A5 => (148.0, 210.0),
            PaperSize::Letter => (215.9, 279.4),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PaperSize::A4 => "a4",
            PaperSize::A5 => "a5",
            PaperSize::Letter => "letter",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PageKind {
    #[default]
    Endless,
    Paper {
        size: PaperSize,
        orientation: Orientation,
    },
    Pdf,
}

impl PageKind {
    pub fn sheet_size(&self) -> Option<(f64, f64)> {
        match *self {
            PageKind::Endless | PageKind::Pdf => None,
            PageKind::Paper { size, orientation } => {
                let (w, h) = size.mm();
                let (w, h) = match orientation {
                    Orientation::Portrait => (w, h),
                    Orientation::Landscape => (h, w),
                };
                Some((w * UNITS_PER_MM, h * UNITS_PER_MM))
            }
        }
    }
}

pub const SHEET_GAP: f64 = 24.0;

pub const UNITS_PER_POINT: f64 = 96.0 / 72.0;

pub type SheetRect = (f64, f64, f64, f64);

pub fn stack_sheets(sizes: &[(f64, f64)]) -> Vec<SheetRect> {
    let widest = sizes.iter().map(|s| s.0).fold(0.0, f64::max);
    let mut y = 0.0;
    sizes
        .iter()
        .map(|&(w, h)| {
            let rect = (((widest - w) / 2.0).round(), y, w, h);
            y += h + SHEET_GAP;
            rect
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PatternStyle {
    Blank,
    Lined,
    Grid,
    #[default]
    Dotted,
}

impl PatternStyle {
    pub const ALL: [PatternStyle; 4] = [
        PatternStyle::Blank,
        PatternStyle::Lined,
        PatternStyle::Grid,
        PatternStyle::Dotted,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PatternStyle::Blank => "blank",
            PatternStyle::Lined => "lined",
            PatternStyle::Grid => "grid",
            PatternStyle::Dotted => "dotted",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    pub fn default_spacing(self) -> f64 {
        match self {
            PatternStyle::Lined => 8.0 * UNITS_PER_MM,
            _ => 5.0 * UNITS_PER_MM,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBackground {
    pub pattern: PatternStyle,
    pub spacing: f64,
    pub paper: Rgba,
    pub ink: Rgba,
}

impl Default for PageBackground {
    fn default() -> Self {
        let pattern = PatternStyle::default();
        Self {
            pattern,
            spacing: pattern.default_spacing(),
            paper: Rgba::rgb(0.99, 0.99, 0.98),
            ink: Rgba::rgb(0.66, 0.68, 0.76),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PageSettings {
    pub kind: PageKind,
    pub background: PageBackground,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheets_stack_and_center() {
        let sheets = stack_sheets(&[(100.0, 50.0), (60.0, 80.0)]);
        assert_eq!(sheets[0], (0.0, 0.0, 100.0, 50.0));
        assert_eq!(sheets[1], (20.0, 50.0 + SHEET_GAP, 60.0, 80.0));
    }

    #[test]
    fn names_round_trip() {
        for s in PaperSize::ALL {
            assert_eq!(PaperSize::from_name(s.name()), Some(s));
        }
        for p in PatternStyle::ALL {
            assert_eq!(PatternStyle::from_name(p.name()), Some(p));
        }
    }
}
