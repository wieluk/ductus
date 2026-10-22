//! One Euro filter (Casiez et al. 2012): smooth when slow, little lag when fast.

use kurbo::{Point, Vec2};
use std::f64::consts::TAU;

fn alpha(cutoff_hz: f64, dt: f64) -> f64 {
    let tau = 1.0 / (TAU * cutoff_hz);
    1.0 / (1.0 + tau / dt)
}

#[derive(Debug, Clone)]
pub struct OneEuro2d {
    pub min_cutoff: f64,
    pub beta: f64,
    pub d_cutoff: f64,
    prev: Option<(Point, f64)>,
    velocity: Vec2,
}

impl OneEuro2d {
    pub fn new(min_cutoff: f64, beta: f64) -> Self {
        Self {
            min_cutoff,
            beta,
            d_cutoff: 1.0,
            prev: None,
            velocity: Vec2::ZERO,
        }
    }

    pub fn filter(&mut self, p: Point, t: f64, speed_scale: f64) -> Point {
        let Some((prev_p, prev_t)) = self.prev else {
            self.prev = Some((p, t));
            return p;
        };
        // Identical timestamps happen with coalesced events; assume ~1 kHz.
        let dt = (t - prev_t).max(0.001);
        let raw_velocity = (p - prev_p) / dt;
        let a_d = alpha(self.d_cutoff, dt);
        self.velocity = self.velocity + (raw_velocity - self.velocity) * a_d;
        let cutoff = self.min_cutoff + self.beta * self.velocity.hypot() * speed_scale;
        let a = alpha(cutoff, dt);
        let out = prev_p + (p - prev_p) * a;
        self.prev = Some((out, t));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_is_reduced() {
        let mut f = OneEuro2d::new(1.0, 0.0);
        let mut max_dev: f64 = 0.0;
        for i in 0..200 {
            let jitter = if i % 2 == 0 { 1.0 } else { -1.0 };
            let out = f.filter(Point::new(jitter, 0.0), i as f64 * 0.005, 1.0);
            if i > 150 {
                max_dev = max_dev.max(out.x.abs());
            }
        }
        assert!(max_dev < 0.2, "max deviation {max_dev}");
    }

    #[test]
    fn fast_motion_has_little_lag() {
        let mut f = OneEuro2d::new(1.0, 0.05);
        let mut last = Point::ZERO;
        for i in 0..100 {
            let t = i as f64 * 0.005;
            last = f.filter(Point::new(t * 1000.0, 0.0), t, 1.0);
        }
        let expected = 99.0 * 0.005 * 1000.0;
        assert!(expected - last.x < 10.0, "lag {}", expected - last.x);
    }
}
