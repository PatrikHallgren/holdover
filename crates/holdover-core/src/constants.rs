/// Standard gravity, m/s².
pub const STANDARD_GRAVITY: f64 = 9.80665;

/// Specific gas constant for dry air, J/(kg·K). Plan §8.
pub const R_DRY: f64 = 287.058;
/// Specific gas constant for water vapour, J/(kg·K). Plan §8.
pub const R_VAPOR: f64 = 461.495;
pub const GAMMA_AIR: f64 = 1.4;
pub const T0_K: f64 = 288.15;
pub const P0_PA: f64 = 101_325.0;
pub const LAPSE_K_PER_M: f64 = 0.0065;
/// ICAO sea-level density used for density altitude, kg/m³.
pub const RHO0: f64 = 1.225;

/// Exact lb/in² → kg/m². The plan quotes this as 703.07.
pub const BC_LB_PER_IN2_TO_KG_PER_M2: f64 = 0.45359237 / (0.0254 * 0.0254);

/// Earth rotation rate from plan §7.7, rad/s.
pub const EARTH_OMEGA: f64 = 7.2921e-5;
