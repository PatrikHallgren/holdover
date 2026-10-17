//! Miller stability and the Litz spin-drift approximation from plan §7.7–7.8.
//!
//! The coefficients are those of equation 6.1 in Bryan Litz, *Applied
//! Ballistics for Long Range Shooting*, 3rd ed. (2015):
//! `SD[in] = 1.25·(SG + 1.2)·TOF^1.83`.
//!
//! That book is not in this repository. On 2026-10-17 the same equation
//! number and coefficients were quoted at <https://stonerphysics.net/spin-drift>.
//! Litz's own 2021 note, "Gyroscopic (spin) Drift and Coriolis Effect"
//! (<https://appliedballisticsllc.com/wp-content/uploads/2021/06/Gyroscopic-Drift-and-Coriolis-Effect.pdf>),
//! gives the typical size (about 8–9 inches at 1000 yards, and no more than
//! 10–12 for flat fire) and does not print the closed form. The coefficient
//! is cited, not marked verified against the primary text.

/// Provenance of [`spin_drift_m`]. Kept as data so a test can lock the status.
pub const SPIN_DRIFT_PROVENANCE: &str = "cited as equation 6.1 of Litz, Applied Ballistics for Long Range Shooting, 3rd ed. (2015); the book is not in this repository, so the coefficient is not marked verified against the primary text";

use crate::units::{celsius_to_fahrenheit, pa_to_inhg, FOOT_M, INCH_M};

pub fn miller_sg(
    weight_grains: f64,
    diameter_in: f64,
    length_in: f64,
    twist_in_per_turn: f64,
) -> f64 {
    let t = twist_in_per_turn.abs() / diameter_in;
    let l = length_in / diameter_in;
    30.0 * weight_grains / (t * t * diameter_in.powi(3) * l * (1.0 + l * l))
}

pub fn miller_sg_corrected(
    sg: f64,
    velocity_fps: f64,
    temperature_f: f64,
    pressure_inhg: f64,
) -> f64 {
    sg * (velocity_fps / 2800.0).powf(1.0 / 3.0)
        * ((temperature_f + 460.0) / 519.0)
        * (29.92 / pressure_inhg)
}

/// Lateral spin drift in metres. Positive is to the right for right-hand twist.
pub fn spin_drift_m(sg_corrected: f64, tof_s: f64, right_hand: bool) -> f64 {
    if tof_s <= 0.0 {
        return 0.0;
    }
    let inches = 1.25 * (sg_corrected + 1.2) * tof_s.powf(1.83);
    let metres = inches * INCH_M;
    if right_hand {
        metres
    } else {
        -metres
    }
}

pub fn sg_for_shot(
    weight_kg: f64,
    diameter_m: f64,
    length_m: f64,
    metres_per_turn: f64,
    velocity_mps: f64,
    temperature_c: f64,
    station_pressure_pa: f64,
) -> (f64, f64) {
    let grains = weight_kg / crate::units::GRAIN_KG;
    let diameter_in = diameter_m / INCH_M;
    let length_in = length_m / INCH_M;
    let twist_in = metres_per_turn / INCH_M;
    let sg = miller_sg(grains, diameter_in, length_in, twist_in);
    let fps = velocity_mps / FOOT_M;
    let corrected = miller_sg_corrected(
        sg,
        fps,
        celsius_to_fahrenheit(temperature_c),
        pa_to_inhg(station_pressure_pa),
    );
    (sg, corrected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miller_worked_example() {
        // 175 gr, .308 in, 1.24 in long, 1:10 in twist.
        let sg = miller_sg(175.0, 0.308, 1.24, 10.0);
        assert!((sg - 2.460_341_762_414_797_6).abs() < 1e-12);
        let at_reference = miller_sg_corrected(sg, 2800.0, 59.0, 29.92);
        assert!((at_reference - sg).abs() < 1e-12);
        let slower = miller_sg_corrected(sg, 800.0 / 0.3048, 59.0, 29.92);
        assert!(slower < sg);
    }

    #[test]
    fn spin_drift_formula() {
        let inches = 1.25 * (1.5 + 1.2) * 1.0_f64.powf(1.83);
        assert!((inches - 3.375).abs() < 1e-12);
        let metres = spin_drift_m(1.5, 1.0, true);
        assert!((metres - inches * INCH_M).abs() < 1e-12);
        assert!(spin_drift_m(1.5, 1.0, false) < 0.0);
    }

    #[test]
    fn cited_equation_6_1_matches_the_quoted_10_4_inch_example() {
        // Sg 1.91 and 1.714 s are the Federal GM308M2, 1:11.25, 1000-yard
        // figures quoted with equation 6.1. The page rounds the result to 10.4 in.
        let inches = 1.25 * (1.91 + 1.2) * 1.714_f64.powf(1.83);
        assert!(((inches * 10.0).round() / 10.0 - 10.4).abs() < 1e-9);
        let metres = spin_drift_m(1.91, 1.714, true);
        assert!((metres / INCH_M - inches).abs() < 1e-12);
        assert!(SPIN_DRIFT_PROVENANCE.contains("not marked verified against the primary text"));
    }
}
