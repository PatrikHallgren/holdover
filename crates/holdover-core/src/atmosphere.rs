//! Constant-atmosphere model used along a trajectory in v1.
//!
//! Density uses dry air plus water vapour. Sea-level pressure is converted to
//! station pressure with the ICAO troposphere, independent of the actual
//! temperature (the same split an altimeter setting uses).

use crate::constants::{
    GAMMA_AIR, LAPSE_K_PER_M, P0_PA, RHO0, R_DRY, R_VAPOR, STANDARD_GRAVITY, T0_K,
};
use crate::error::{Error, Result};
use crate::units::PASCAL_PER_INHG;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressureKind {
    Station,
    SeaLevel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    pub temperature_c: f64,
    /// Absolute pressure in pascals, interpreted by `pressure_kind`.
    pub pressure_pa: f64,
    pub pressure_kind: PressureKind,
    /// Relative humidity, 0 to 1.
    pub humidity: f64,
    pub altitude_m: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirState {
    pub density_kg_m3: f64,
    pub speed_of_sound_mps: f64,
    pub station_pressure_pa: f64,
    pub vapor_pressure_pa: f64,
    pub density_altitude_m: f64,
    pub temperature_k: f64,
    /// Station pressure looks like sea-level pressure well above the ground.
    pub pressure_kind_suspicious: bool,
}

impl Atmosphere {
    pub fn icao() -> Self {
        Self {
            temperature_c: 15.0,
            pressure_pa: P0_PA,
            pressure_kind: PressureKind::Station,
            humidity: 0.0,
            altitude_m: 0.0,
        }
    }

    /// Army Standard Metro: 59 °F, 29.5275 inHg, 78% relative humidity.
    pub fn army_standard_metro() -> Self {
        Self {
            temperature_c: 15.0,
            pressure_pa: 29.5275 * PASCAL_PER_INHG,
            pressure_kind: PressureKind::Station,
            humidity: 0.78,
            altitude_m: 0.0,
        }
    }

    pub fn vacuum() -> Self {
        Self {
            temperature_c: 15.0,
            pressure_pa: 0.0,
            pressure_kind: PressureKind::Station,
            humidity: 0.0,
            altitude_m: 0.0,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !self.temperature_c.is_finite()
            || self.temperature_c < -100.0
            || self.temperature_c > 60.0
        {
            return Err(Error::Invalid("temperature is outside -100 to 60 °C"));
        }
        if !self.pressure_pa.is_finite() || self.pressure_pa < 0.0 {
            return Err(Error::Invalid("pressure must be zero or positive"));
        }
        if !self.humidity.is_finite() || !(0.0..=1.0).contains(&self.humidity) {
            return Err(Error::Invalid("humidity must be a fraction from 0 to 1"));
        }
        if !self.altitude_m.is_finite() {
            return Err(Error::Invalid("altitude is not finite"));
        }
        Ok(())
    }

    pub fn state(&self) -> Result<AirState> {
        self.validate()?;
        let station = self.station_pressure_pa();
        let temperature_k = self.temperature_c + 273.15;
        if station <= 0.0 {
            let speed = (GAMMA_AIR * R_DRY * temperature_k).sqrt();
            return Ok(AirState {
                density_kg_m3: 0.0,
                speed_of_sound_mps: speed,
                station_pressure_pa: 0.0,
                vapor_pressure_pa: 0.0,
                density_altitude_m: f64::INFINITY,
                temperature_k,
                pressure_kind_suspicious: false,
            });
        }
        let saturation = buck_saturation_pa(self.temperature_c);
        let vapor = (self.humidity * saturation).min(station * 0.99);
        let dry = (station - vapor).max(0.0);
        let density = dry / (R_DRY * temperature_k) + vapor / (R_VAPOR * temperature_k);
        let tv = virtual_temperature_k(temperature_k, station, vapor);
        let speed = (GAMMA_AIR * R_DRY * tv).sqrt();
        Ok(AirState {
            density_kg_m3: density,
            speed_of_sound_mps: speed,
            station_pressure_pa: station,
            vapor_pressure_pa: vapor,
            density_altitude_m: density_altitude_m(density),
            temperature_k,
            pressure_kind_suspicious: pressure_kind_suspicious(self),
        })
    }

    pub fn station_pressure_pa(&self) -> f64 {
        match self.pressure_kind {
            PressureKind::Station => self.pressure_pa,
            PressureKind::SeaLevel => isa_station_pressure_pa(self.altitude_m, self.pressure_pa),
        }
    }
}

/// Buck 1981 saturation vapour pressure over water, pascals.
pub fn buck_saturation_pa(temperature_c: f64) -> f64 {
    let es_hpa = 6.1121
        * ((18.678 - temperature_c / 234.5) * (temperature_c / (257.14 + temperature_c))).exp();
    es_hpa * 100.0
}

pub fn virtual_temperature_k(temperature_k: f64, pressure_pa: f64, vapor_pa: f64) -> f64 {
    if pressure_pa <= 0.0 {
        return temperature_k;
    }
    let factor = 1.0 - (vapor_pa / pressure_pa) * (1.0 - R_DRY / R_VAPOR);
    temperature_k / factor
}

pub fn isa_station_pressure_pa(altitude_m: f64, sea_level_pa: f64) -> f64 {
    let t = isa_temperature_k(altitude_m);
    let exponent = STANDARD_GRAVITY / (LAPSE_K_PER_M * R_DRY);
    sea_level_pa * (t / T0_K).powf(exponent)
}

#[allow(dead_code)]
pub fn isa_sea_level_pressure_pa(altitude_m: f64, station_pa: f64) -> f64 {
    let t = isa_temperature_k(altitude_m);
    let exponent = STANDARD_GRAVITY / (LAPSE_K_PER_M * R_DRY);
    station_pa * (T0_K / t).powf(exponent)
}

pub fn isa_temperature_k(altitude_m: f64) -> f64 {
    T0_K - LAPSE_K_PER_M * altitude_m
}

pub fn density_altitude_m(density: f64) -> f64 {
    if density <= 0.0 {
        return f64::INFINITY;
    }
    let sigma = density / RHO0;
    let exponent = R_DRY * LAPSE_K_PER_M / STANDARD_GRAVITY;
    (T0_K / LAPSE_K_PER_M) * (1.0 - sigma.powf(exponent))
}

/// About 1013 hPa entered as station pressure above 500 m.
pub fn pressure_kind_suspicious(atmo: &Atmosphere) -> bool {
    if atmo.pressure_kind != PressureKind::Station || atmo.altitude_m <= 500.0 {
        return false;
    }
    let hpa = atmo.pressure_pa / 100.0;
    (hpa - 1013.25).abs() < 5.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icao_dry_sea_level() {
        let air = Atmosphere::icao().state().unwrap();
        assert!((air.density_kg_m3 - 1.224_978_126_206_651_3).abs() < 1e-12);
        assert!((air.speed_of_sound_mps - 340.297_028_755_762_1).abs() < 1e-9);
        assert!(air.density_altitude_m.abs() < 1.0);
        assert!(!air.pressure_kind_suspicious);
    }

    #[test]
    fn humidity_lowers_density() {
        let mut wet = Atmosphere::icao();
        wet.humidity = 1.0;
        let air = wet.state().unwrap();
        assert!((air.vapor_pressure_pa - 1_705.172_836_105_207_4).abs() < 1e-6);
        assert!((air.density_kg_m3 - 1.217_186_076_112_947_2).abs() < 1e-9);
        assert!(air.density_kg_m3 < Atmosphere::icao().state().unwrap().density_kg_m3);
    }

    #[test]
    fn isa_points_and_pressure_round_trip() {
        let p1000 = isa_station_pressure_pa(1000.0, P0_PA);
        assert!((p1000 - 89_874.755_522_364_82).abs() < 1e-6);
        let p2000 = isa_station_pressure_pa(2000.0, P0_PA);
        assert!(p2000 < p1000);
        assert!((isa_temperature_k(1000.0) - 281.65).abs() < 1e-12);
        let back = isa_sea_level_pressure_pa(1500.0, isa_station_pressure_pa(1500.0, P0_PA));
        assert!((back - P0_PA).abs() < 1e-6);

        let mut qnh = Atmosphere::icao();
        qnh.pressure_kind = PressureKind::SeaLevel;
        qnh.altitude_m = 1000.0;
        let station = qnh.station_pressure_pa();
        assert!((station - p1000).abs() < 1e-6);
        let mut as_station = qnh;
        as_station.pressure_kind = PressureKind::Station;
        as_station.pressure_pa = station;
        let back =
            isa_sea_level_pressure_pa(as_station.altitude_m, as_station.station_pressure_pa());
        assert!((back - P0_PA).abs() < 1e-4);
    }

    #[test]
    fn suspicious_station_pressure_above_500_m() {
        let mut atmo = Atmosphere::icao();
        atmo.altitude_m = 800.0;
        assert!(atmo.state().unwrap().pressure_kind_suspicious);
        atmo.pressure_pa = 90_000.0;
        assert!(!atmo.state().unwrap().pressure_kind_suspicious);
        atmo.pressure_pa = P0_PA;
        atmo.altitude_m = 100.0;
        assert!(!atmo.state().unwrap().pressure_kind_suspicious);
    }
}
