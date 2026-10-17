//! Miller stability and the Litz spin-drift approximation from plan §7.7–7.8.
//!
//! The spin-drift coefficient is the one written in the plan. It still needs
//! a check against the primary Litz text before a v1 release.

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
}
