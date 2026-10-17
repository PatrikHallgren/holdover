//! Monotone piecewise-cubic Hermite interpolation (Fritsch–Carlson).
//!
//! The slope rules match the PCHIP used by the py-ballisticcalc oracle so a
//! shared Cd table produces the same interior values. Outside the knots,
//! evaluation clamps and reports that it did so. The oracle extrapolates; that
//! difference is intentional (plan §7.3).

use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub struct Spline {
    x: Vec<f64>,
    y: Vec<f64>,
    m: Vec<f64>,
}

impl Spline {
    pub fn new(x: Vec<f64>, y: Vec<f64>) -> Result<Self> {
        if x.len() != y.len() || x.len() < 2 {
            return Err(Error::Invalid("a drag curve needs at least two points"));
        }
        for i in 1..x.len() {
            if x[i] <= x[i - 1] {
                return Err(Error::Invalid("drag-curve Mach values must increase"));
            }
        }
        let m = slopes(&x, &y);
        Ok(Self { x, y, m })
    }

    /// Value at `x`. The boolean is true when `x` was outside the knots.
    pub fn eval_clamped(&self, x: f64) -> (f64, bool) {
        if x <= self.x[0] {
            return (self.y[0], x < self.x[0]);
        }
        let last = self.x.len() - 1;
        if x >= self.x[last] {
            return (self.y[last], x > self.x[last]);
        }
        let mut lo = 0;
        let mut hi = last;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.x[mid] <= x {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (
            hermite(
                x, self.x[lo], self.x[hi], self.y[lo], self.y[hi], self.m[lo], self.m[hi],
            ),
            false,
        )
    }
}

fn slopes(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len();
    let h: Vec<f64> = (0..n - 1).map(|i| x[i + 1] - x[i]).collect();
    let delta: Vec<f64> = (0..n - 1).map(|i| (y[i + 1] - y[i]) / h[i]).collect();
    let mut m = vec![0.0; n];
    if n == 2 {
        m[0] = delta[0];
        m[1] = delta[0];
        return m;
    }
    for i in 1..n - 1 {
        let d0 = delta[i - 1];
        let d1 = delta[i];
        if d0 == 0.0 || d1 == 0.0 || d0.signum() != d1.signum() {
            m[i] = 0.0;
        } else {
            let w1 = 2.0 * h[i] + h[i - 1];
            let w2 = h[i] + 2.0 * h[i - 1];
            m[i] = (w1 + w2) / (w1 / d0 + w2 / d1);
        }
    }
    let d0 = delta[0];
    let d1 = delta[1];
    let mut m0 = ((2.0 * h[0] + h[1]) * d0 - h[0] * d1) / (h[0] + h[1]);
    if m0.signum() != d0.signum() {
        m0 = 0.0;
    } else if m0.abs() > 3.0 * d0.abs() {
        m0 = 3.0 * d0;
    }
    m[0] = m0;

    let dn = delta[n - 2];
    let dnm = delta[n - 3];
    let hn = h[n - 2];
    let hnm = h[n - 3];
    let mut mn = ((2.0 * hn + hnm) * dn - hn * dnm) / (hn + hnm);
    if mn.signum() != dn.signum() {
        mn = 0.0;
    } else if mn.abs() > 3.0 * dn.abs() {
        mn = 3.0 * dn;
    }
    m[n - 1] = mn;
    m
}

fn hermite(x: f64, x0: f64, x1: f64, y0: f64, y1: f64, m0: f64, m1: f64) -> f64 {
    let h = x1 - x0;
    let t = (x - x0) / h;
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * y0 + h * h10 * m0 + h01 * y1 + h * h11 * m1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_knots_and_clamps() {
        let s = Spline::new(vec![0.0, 1.0, 2.0], vec![0.0, 1.0, 0.0]).unwrap();
        assert!((s.eval_clamped(0.0).0).abs() < 1e-15);
        assert!((s.eval_clamped(1.0).0 - 1.0).abs() < 1e-15);
        assert!(!s.eval_clamped(1.0).1);
        let (hi, clamped) = s.eval_clamped(3.0);
        assert!(clamped);
        assert!((hi - 0.0).abs() < 1e-15);
        let (lo, clamped) = s.eval_clamped(-1.0);
        assert!(clamped);
        assert!(lo.abs() < 1e-15);
    }
}
