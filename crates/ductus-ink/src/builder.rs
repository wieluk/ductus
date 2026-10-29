use crate::OneEuro2d;
use ductus_core::{InkPoint, Stroke, StrokeId, StrokeStyle};
use kurbo::Point;

#[derive(Debug, Clone, Copy)]
pub struct RawSample {
    pub x: f64,
    pub y: f64,
    pub pressure: Option<f64>,
    pub tilt_x: f64,
    pub tilt_y: f64,
    pub time_ms: u32,
}

/// Effective pressure is p^gamma: gamma < 1 makes light strokes thicker.
#[derive(Debug, Clone, Copy)]
pub struct PressureCurve {
    pub gamma: f64,
    pub fallback: f64,
}

impl Default for PressureCurve {
    fn default() -> Self {
        Self {
            gamma: 1.0,
            fallback: 0.6,
        }
    }
}

impl PressureCurve {
    pub fn map(&self, pressure: Option<f64>) -> f64 {
        match pressure {
            Some(p) => p.clamp(0.0, 1.0).powf(self.gamma),
            None => self.fallback,
        }
    }
}

/// Tuned in screen pixels so smoothing feels the same at any zoom.
#[derive(Debug, Clone, Copy)]
pub struct Smoothing {
    pub enabled: bool,
    pub min_cutoff: f64,
    pub beta: f64,
    pub min_distance: f64,
}

impl Default for Smoothing {
    fn default() -> Self {
        Self {
            enabled: true,
            min_cutoff: 2.0,
            beta: 0.05,
            min_distance: 0.4,
        }
    }
}

pub struct StrokeBuilder {
    style: StrokeStyle,
    curve: PressureCurve,
    smoothing: Smoothing,
    units_per_px: f64,
    filter: OneEuro2d,
    points: Vec<InkPoint>,
    start_ms: Option<u32>,
    last_raw: Option<RawSample>,
    smoothed_pressure: Option<f64>,
}

impl StrokeBuilder {
    pub fn new(style: StrokeStyle, curve: PressureCurve, smoothing: Smoothing, zoom: f64) -> Self {
        Self {
            style,
            curve,
            smoothing,
            units_per_px: 1.0 / zoom,
            filter: OneEuro2d::new(smoothing.min_cutoff, smoothing.beta),
            points: Vec::new(),
            start_ms: None,
            last_raw: None,
            smoothed_pressure: None,
        }
    }

    pub fn style(&self) -> &StrokeStyle {
        &self.style
    }

    pub fn points(&self) -> &[InkPoint] {
        &self.points
    }

    pub fn push(&mut self, s: RawSample) {
        let start = *self.start_ms.get_or_insert(s.time_ms);
        let t = s.time_ms.wrapping_sub(start);
        self.last_raw = Some(s);

        let raw_point = Point::new(s.x, s.y);
        let p = if self.smoothing.enabled {
            self.filter
                .filter(raw_point, f64::from(t) / 1000.0, 1.0 / self.units_per_px)
        } else {
            raw_point
        };
        let target = self.curve.map(s.pressure);
        let pressure = match self.smoothed_pressure {
            Some(prev) => prev + (target - prev) * 0.6,
            None => target,
        };
        self.smoothed_pressure = Some(pressure);

        let point = InkPoint {
            x: p.x,
            y: p.y,
            pressure: pressure as f32,
            tilt_x: s.tilt_x as f32,
            tilt_y: s.tilt_y as f32,
            t,
        };
        if let Some(last) = self.points.last_mut() {
            let d = (p - Point::new(last.x, last.y)).hypot();
            if d < self.smoothing.min_distance * self.units_per_px {
                // Too close to add, but keep pressure responsive (e.g. pressing harder in place).
                last.pressure = last.pressure.max(point.pressure);
                return;
            }
        }
        self.points.push(point);
    }

    /// The last raw point is added so the filter's lag never shortens the stroke.
    pub fn finish(mut self, id: StrokeId) -> Option<Stroke> {
        let raw = self.last_raw?;
        if let Some(last) = self.points.last().copied() {
            let d = (Point::new(raw.x, raw.y) - Point::new(last.x, last.y)).hypot();
            if d > self.smoothing.min_distance * self.units_per_px {
                self.points.push(InkPoint {
                    x: raw.x,
                    y: raw.y,
                    pressure: self.smoothed_pressure.map_or(last.pressure, |p| p as f32),
                    tilt_x: raw.tilt_x as f32,
                    tilt_y: raw.tilt_y as f32,
                    t: raw
                        .time_ms
                        .wrapping_sub(self.start_ms.unwrap_or(raw.time_ms)),
                });
            }
        }
        Some(Stroke {
            id,
            style: self.style,
            points: self.points,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(x: f64, t: u32) -> RawSample {
        RawSample {
            x,
            y: 0.0,
            pressure: Some(1.0),
            tilt_x: 0.0,
            tilt_y: 0.0,
            time_ms: t,
        }
    }

    #[test]
    fn stroke_ends_at_last_raw_point() {
        let mut b = StrokeBuilder::new(
            StrokeStyle::default(),
            PressureCurve::default(),
            Smoothing::default(),
            1.0,
        );
        for i in 0..50 {
            b.push(sample(f64::from(i) * 3.0, 1000 + i * 5));
        }
        let stroke = b.finish(StrokeId(1)).unwrap();
        let last = stroke.points.last().unwrap();
        assert_eq!(last.x, 49.0 * 3.0);
        assert_eq!(stroke.points[0].t, 0);
    }
}
