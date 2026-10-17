//! Published Swedish ring diameters used as presets.
//!
//! Outlines and silhouettes are not here. Those measurements are still open
//! (plan §33, items T2–T6).

/// Current official SJF älgfigur diameters, centimetres, for rings 5¹, 5, 4 and 3.
///
/// Patrik confirmed these on 2026-10-17. The older 15/25/35/45 cm set is not a preset.
pub const SJF_MOOSE_RING_DIAMETER_CM: [f64; 4] = [15.0, 25.0, 33.0, 41.5];

/// SJF roe-buck ring diameters from the published rules, centimetres.
/// Item T4 is still open: confirm these on an owned target.
pub const SJF_ROE_BUCK_RING_DIAMETER_CM: [f64; 3] = [10.0, 15.0, 20.0];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moose_preset_is_the_sjf_set() {
        assert_eq!(SJF_MOOSE_RING_DIAMETER_CM, [15.0, 25.0, 33.0, 41.5]);
        assert!(SJF_MOOSE_RING_DIAMETER_CM.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(SJF_ROE_BUCK_RING_DIAMETER_CM, [10.0, 15.0, 20.0]);
    }
}
