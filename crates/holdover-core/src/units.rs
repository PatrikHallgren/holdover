//! Unit parsing and conversion.
//!
//! Stored and computed quantities are SI. A typed suffix on input selects the
//! unit; a bare number is interpreted with the caller's default unit. `gr` is
//! grains and `g` is grams. The token `gr.` is rejected.

use crate::error::{Error, Result};

pub const INCH_M: f64 = 0.0254;
pub const FOOT_M: f64 = 0.3048;
pub const YARD_M: f64 = 0.9144;
pub const POUND_KG: f64 = 0.45359237;
pub const OUNCE_KG: f64 = POUND_KG / 16.0;
pub const GRAIN_KG: f64 = POUND_KG / 7000.0;
pub const MPH_MPS: f64 = 0.44704;
pub const KN_MPS: f64 = 1852.0 / 3600.0;
pub const KMH_MPS: f64 = 1000.0 / 3600.0;
/// NIST inch of mercury (0 °C) in pascals.
pub const PASCAL_PER_INHG: f64 = 3386.389;
pub const PASCAL_PER_MMHG: f64 = PASCAL_PER_INHG / 25.4;
pub const PASCAL_PER_PSI: f64 = 6894.757293168361;
pub const MOA_RAD: f64 = std::f64::consts::PI / (180.0 * 60.0);
pub const MRAD_RAD: f64 = 1.0e-3;
/// Small-angle inch-per-hundred-yards. 1 mrad = 3.6 IPHY exactly.
pub const IPHY_RAD: f64 = MRAD_RAD / 3.6;
/// Small-angle centimetre at 100 m. 1 mrad = 10 cm@100 m exactly.
pub const CM_PER_100M_RAD: f64 = 1.0e-4;
/// Mechanical joule per foot-pound, from the foot, the pound and standard gravity.
pub const JOULE_PER_FT_LBF: f64 = FOOT_M * POUND_KG * crate::constants::STANDARD_GRAVITY;
/// Plan §7.9 divisor: grains and feet per second to foot-pounds.
pub const FT_LBF_DIVISOR: f64 = 450_240.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dimension {
    Length,
    Mass,
    Velocity,
    Temperature,
    Pressure,
    Angle,
    Energy,
}

/// Canonical value for `dim`: metres, kilograms, m/s, degrees Celsius, pascals,
/// radians, or joules.
pub fn parse_quantity(input: &str, dim: Dimension, default_unit: Option<&str>) -> Result<f64> {
    let raw = input.trim().replace('\u{2212}', "-");
    if raw.is_empty() {
        return Err(Error::Parse {
            what: "quantity",
            input: input.to_string(),
            reason: "empty input",
        });
    }
    let (value, consumed) = parse_leading_number(&raw)?;
    let unit_raw = raw[consumed..].trim();
    if unit_is_ambiguous_grain(unit_raw) {
        return Err(Error::Parse {
            what: "unit",
            input: input.to_string(),
            reason: "'gr.' is ambiguous; use 'gr' for grains or 'g' for grams",
        });
    }
    let unit = if unit_raw.is_empty() {
        match default_unit {
            Some(u) => u,
            None => {
                return Err(Error::Parse {
                    what: "unit",
                    input: input.to_string(),
                    reason: missing_unit_reason(dim),
                });
            }
        }
    } else {
        unit_raw
    };
    apply_unit(value, unit, dim, input)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TwistHand {
    Right,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Twist {
    pub metres_per_turn: f64,
    pub hand: TwistHand,
}

/// Wind from a clock facing the target. `0` radians is 12 o'clock (headwind).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wind {
    pub speed_mps: f64,
    /// Clockwise from 12 o'clock, radians.
    pub from_clock_rad: f64,
}

impl Default for Wind {
    fn default() -> Self {
        Self {
            speed_mps: 0.0,
            from_clock_rad: 0.0,
        }
    }
}

pub fn parse_twist(input: &str) -> Result<Twist> {
    let raw = input.trim().replace('\u{2212}', "-");
    let lower = raw.to_lowercase();
    let (hand, rest) = if let Some(r) = lower.strip_prefix("lh") {
        (TwistHand::Left, r.trim_start_matches([' ', ':', '-']))
    } else if let Some(r) = lower.strip_prefix("left") {
        (TwistHand::Left, r.trim_start_matches([' ', ':', '-']))
    } else if let Some(r) = lower.strip_prefix("rh") {
        (TwistHand::Right, r.trim_start_matches([' ', ':', '-']))
    } else if let Some(r) = lower.strip_prefix("right") {
        (TwistHand::Right, r.trim_start_matches([' ', ':', '-']))
    } else {
        (TwistHand::Right, lower.as_str())
    };
    let rest = rest.trim();
    let spec = rest.strip_prefix("1:").unwrap_or(rest);
    let (value, consumed) = parse_leading_number(spec).map_err(|_| Error::Parse {
        what: "twist",
        input: input.to_string(),
        reason: "expected a length such as 1:8in or 1:203mm",
    })?;
    if value <= 0.0 {
        return Err(Error::Invalid("twist length must be positive"));
    }
    let unit_raw = spec[consumed..].trim().trim_matches('"');
    let metres = if unit_raw.is_empty() {
        value * INCH_M
    } else {
        apply_unit(value, unit_raw, Dimension::Length, input)?
    };
    Ok(Twist {
        metres_per_turn: metres,
        hand,
    })
}

/// Clock or degree wind direction. `9oc` is a wind from the left (9 o'clock).
/// `12` is a headwind, from the target toward the shooter.
pub fn parse_wind(input: &str) -> Result<Wind> {
    let raw = input.trim().replace('\u{2212}', "-");
    let (speed_s, dir_s) = raw.split_once('@').ok_or(Error::Parse {
        what: "wind",
        input: input.to_string(),
        reason: "expected speed@direction, for example 3mps@9oc",
    })?;
    let speed = parse_quantity(speed_s, Dimension::Velocity, None)?;
    let from_clock_rad = parse_wind_direction(dir_s.trim())?;
    if speed < 0.0 {
        return Err(Error::Invalid("wind speed cannot be negative"));
    }
    Ok(Wind {
        speed_mps: speed,
        from_clock_rad,
    })
}

pub fn parse_wind_direction(dir: &str) -> Result<f64> {
    let d = dir.trim().to_lowercase().replace('°', "deg");
    let d = d.replace("o'clock", "oc").replace("oclock", "oc");
    if let Some(hour) = d.strip_suffix("oc") {
        let hour = hour.trim();
        let h: f64 = hour.parse().map_err(|_| Error::Parse {
            what: "wind direction",
            input: dir.to_string(),
            reason: "clock hour must be a number",
        })?;
        if !(0.0..=12.0).contains(&h) {
            return Err(Error::Parse {
                what: "wind direction",
                input: dir.to_string(),
                reason: "clock hour must be from 0 to 12",
            });
        }
        return Ok(if h == 12.0 {
            0.0
        } else {
            h / 12.0 * std::f64::consts::TAU
        });
    }
    if let Some(num) = d.strip_suffix("deg") {
        let deg: f64 = num.trim().parse().map_err(|_| Error::Parse {
            what: "wind direction",
            input: dir.to_string(),
            reason: "degree value must be a number",
        })?;
        return Ok(deg.to_radians());
    }
    let n: f64 = d.parse().map_err(|_| Error::Parse {
        what: "wind direction",
        input: dir.to_string(),
        reason: "expected a clock hour (9oc) or degrees (90deg)",
    })?;
    if (0.0..=12.0).contains(&n) && (n - n.round()).abs() < 1e-9 {
        let h = n.round();
        Ok(if h == 12.0 {
            0.0
        } else {
            h / 12.0 * std::f64::consts::TAU
        })
    } else {
        Ok(n.to_radians())
    }
}

pub fn rad_to_mrad(rad: f64) -> f64 {
    rad / MRAD_RAD
}
pub fn rad_to_moa(rad: f64) -> f64 {
    rad / MOA_RAD
}
pub fn rad_to_iphy(rad: f64) -> f64 {
    rad / IPHY_RAD
}
pub fn rad_to_cm_per_100m(rad: f64) -> f64 {
    rad / CM_PER_100M_RAD
}

pub fn joules(mass_kg: f64, speed_mps: f64) -> f64 {
    0.5 * mass_kg * speed_mps * speed_mps
}

pub fn foot_pounds(mass_kg: f64, speed_mps: f64) -> f64 {
    let grains = mass_kg / GRAIN_KG;
    let fps = speed_mps / FOOT_M;
    grains * fps * fps / FT_LBF_DIVISOR
}

pub fn celsius_to_fahrenheit(c: f64) -> f64 {
    c * 9.0 / 5.0 + 32.0
}
pub fn fahrenheit_to_celsius(f: f64) -> f64 {
    (f - 32.0) * 5.0 / 9.0
}
pub fn pa_to_inhg(pa: f64) -> f64 {
    pa / PASCAL_PER_INHG
}

/// Convert a stored SI quantity into `unit` for display.
///
/// Temperature is an offset conversion. The caller keeps the original text
/// and the SI value; this result is not written back.
pub fn convert_from_si(si: f64, unit: &str) -> Result<f64> {
    let unit = normalize_unit(unit);
    let shown = match unit.as_str() {
        "m" | "kg" | "m/s" | "mps" | "c" | "celsius" | "pa" | "rad" | "radian" | "radians"
        | "j" => si,
        "mm" => si * 1.0e3,
        "cm" => si * 1.0e2,
        "in" | "inch" | "inches" => si / INCH_M,
        "ft" | "foot" | "feet" => si / FOOT_M,
        "yd" | "yard" | "yards" => si / YARD_M,
        "g" => si * 1.0e3,
        "gr" | "grain" | "grains" => si / GRAIN_KG,
        "lb" | "lbs" => si / POUND_KG,
        "oz" => si / OUNCE_KG,
        "fps" | "ft/s" => si / FOOT_M,
        "km/h" | "kph" => si / KMH_MPS,
        "mph" => si / MPH_MPS,
        "kn" | "kt" => si / KN_MPS,
        "f" | "fahrenheit" => celsius_to_fahrenheit(si),
        "k" | "kelvin" => si + 273.15,
        "hpa" | "mbar" | "mb" => si / 100.0,
        "bar" => si / 100_000.0,
        "inhg" => si / PASCAL_PER_INHG,
        "mmhg" => si / PASCAL_PER_MMHG,
        "psi" => si / PASCAL_PER_PSI,
        "deg" | "degree" | "degrees" => si.to_degrees(),
        "mrad" | "mil" => si / MRAD_RAD,
        "moa" => si / MOA_RAD,
        "iphy" => si / IPHY_RAD,
        "cm@100m" => si / CM_PER_100M_RAD,
        "ftlbf" | "ft-lbf" | "ftlb" => si / JOULE_PER_FT_LBF,
        _ => {
            return Err(Error::Parse {
                what: "unit",
                input: unit,
                reason: "unknown unit suffix",
            });
        }
    };
    Ok(shown)
}

/// Display form of `convert_from_si`, trimmed so trailing zeros do not look like extra precision.
pub fn format_quantity(si: f64, unit: &str) -> Result<String> {
    Ok(format_number(convert_from_si(si, unit)?))
}

pub fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return "—".to_string();
    }
    let scale = if value.abs() >= 100.0 {
        100.0
    } else {
        10_000.0
    };
    let rounded = (value * scale).round() / scale;
    let mut text = format!("{rounded:.4}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        "0".to_string()
    } else {
        text
    }
}

fn apply_unit(value: f64, unit_raw: &str, dim: Dimension, input: &str) -> Result<f64> {
    let unit = normalize_unit(unit_raw);
    let (got, si) = match unit.as_str() {
        "m" => (Dimension::Length, value),
        "mm" => (Dimension::Length, value * 1.0e-3),
        "cm" => (Dimension::Length, value * 1.0e-2),
        "in" | "inch" | "inches" => (Dimension::Length, value * INCH_M),
        "ft" | "foot" | "feet" => (Dimension::Length, value * FOOT_M),
        "yd" | "yard" | "yards" => (Dimension::Length, value * YARD_M),
        "g" => (Dimension::Mass, value * 1.0e-3),
        "kg" => (Dimension::Mass, value),
        "gr" | "grain" | "grains" => (Dimension::Mass, value * GRAIN_KG),
        "lb" | "lbs" => (Dimension::Mass, value * POUND_KG),
        "oz" => (Dimension::Mass, value * OUNCE_KG),
        "m/s" | "mps" => (Dimension::Velocity, value),
        "fps" | "ft/s" => (Dimension::Velocity, value * FOOT_M),
        "km/h" | "kph" => (Dimension::Velocity, value * KMH_MPS),
        "mph" => (Dimension::Velocity, value * MPH_MPS),
        "kn" | "kt" => (Dimension::Velocity, value * KN_MPS),
        "c" | "celsius" => (Dimension::Temperature, value),
        "f" | "fahrenheit" => (Dimension::Temperature, fahrenheit_to_celsius(value)),
        "k" | "kelvin" => (Dimension::Temperature, value - 273.15),
        "pa" => (Dimension::Pressure, value),
        "hpa" => (Dimension::Pressure, value * 100.0),
        "mbar" | "mb" => (Dimension::Pressure, value * 100.0),
        "bar" => (Dimension::Pressure, value * 100_000.0),
        "inhg" => (Dimension::Pressure, value * PASCAL_PER_INHG),
        "mmhg" => (Dimension::Pressure, value * PASCAL_PER_MMHG),
        "psi" => (Dimension::Pressure, value * PASCAL_PER_PSI),
        "rad" | "radian" | "radians" => (Dimension::Angle, value),
        "deg" | "degree" | "degrees" => (Dimension::Angle, value.to_radians()),
        "mrad" | "mil" => (Dimension::Angle, value * MRAD_RAD),
        "moa" => (Dimension::Angle, value * MOA_RAD),
        "iphy" => (Dimension::Angle, value * IPHY_RAD),
        "cm@100m" | "cm@100 m" => (Dimension::Angle, value * CM_PER_100M_RAD),
        "j" => (Dimension::Energy, value),
        "ftlbf" | "ft-lbf" | "ftlb" => (Dimension::Energy, value * JOULE_PER_FT_LBF),
        _ => {
            return Err(Error::Parse {
                what: "unit",
                input: input.to_string(),
                reason: "unknown unit suffix",
            });
        }
    };
    if got != dim {
        return Err(Error::Parse {
            what: "unit",
            input: input.to_string(),
            reason: "unit has the wrong dimension for this field",
        });
    }
    Ok(si)
}

fn missing_unit_reason(dim: Dimension) -> &'static str {
    match dim {
        Dimension::Length => "a unit suffix is required, for example 100m",
        Dimension::Velocity => "a unit suffix is required, for example 800mps",
        Dimension::Mass => "a unit suffix is required, for example 175gr",
        Dimension::Temperature => "a unit suffix is required, for example 15C",
        Dimension::Pressure => "a unit suffix is required, for example 1013.25hPa",
        Dimension::Angle => "a unit suffix is required, for example 0.1mrad",
        Dimension::Energy => "a unit suffix is required, for example 3600J",
    }
}

fn normalize_unit(unit: &str) -> String {
    unit.trim()
        .trim_matches('"')
        .to_lowercase()
        .replace(['°', '·', ' '], "")
}

fn unit_is_ambiguous_grain(unit: &str) -> bool {
    let u = unit.trim().to_lowercase();
    u == "gr." || u.starts_with("gr.")
}

fn parse_leading_number(s: &str) -> Result<(f64, usize)> {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let start = i;
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.' || b[i] == b',' || b[i] == b'/') {
        i += 1;
    }
    if i == start {
        return Err(Error::Parse {
            what: "number",
            input: s.to_string(),
            reason: "expected a number",
        });
    }
    let token = &s[..i];
    Ok((parse_number_token(token)?, i))
}

fn parse_number_token(token: &str) -> Result<f64> {
    let (sign, rest) = if let Some(r) = token.strip_prefix('+') {
        (1.0, r)
    } else if let Some(r) = token.strip_prefix('-') {
        (-1.0, r)
    } else {
        (1.0, token)
    };
    if rest.is_empty() {
        return Err(Error::Parse {
            what: "number",
            input: token.to_string(),
            reason: "expected a number",
        });
    }
    if let Some((num, den)) = rest.split_once('/') {
        if den.contains('/') {
            return Err(Error::Parse {
                what: "number",
                input: token.to_string(),
                reason: "too many slashes",
            });
        }
        let n = parse_decimal(num)?;
        let d = parse_decimal(den)?;
        if d == 0.0 {
            return Err(Error::Parse {
                what: "number",
                input: token.to_string(),
                reason: "division by zero",
            });
        }
        return Ok(sign * n / d);
    }
    Ok(sign * parse_decimal(rest)?)
}

fn parse_decimal(s: &str) -> Result<f64> {
    if s.is_empty() {
        return Err(Error::Parse {
            what: "number",
            input: s.to_string(),
            reason: "expected a number",
        });
    }
    let dots = s.chars().filter(|c| *c == '.').count();
    let commas = s.chars().filter(|c| *c == ',').count();
    let normalized = if dots > 0 && commas > 0 {
        let last_dot = s.rfind('.').unwrap();
        let last_comma = s.rfind(',').unwrap();
        if last_dot > last_comma {
            s.replace(',', "")
        } else {
            s.replace('.', "").replace(',', ".")
        }
    } else if commas > 1 {
        return Err(Error::Parse {
            what: "number",
            input: s.to_string(),
            reason: "several commas and no decimal point",
        });
    } else if commas == 1 {
        // A single comma is a decimal comma: 1,5 is 1.5.
        s.replace(',', ".")
    } else {
        s.to_string()
    };
    normalized.parse::<f64>().map_err(|_| Error::Parse {
        what: "number",
        input: s.to_string(),
        reason: "not a number",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_plan_suffix_round_trips_dimension() {
        let cases = [
            ("1 m", Dimension::Length, 1.0),
            ("100 cm", Dimension::Length, 1.0),
            ("1000 mm", Dimension::Length, 1.0),
            ("1 in", Dimension::Length, INCH_M),
            ("1 ft", Dimension::Length, FOOT_M),
            ("1 yd", Dimension::Length, YARD_M),
            ("1 g", Dimension::Mass, 0.001),
            ("1 kg", Dimension::Mass, 1.0),
            ("1 gr", Dimension::Mass, GRAIN_KG),
            ("1 lb", Dimension::Mass, POUND_KG),
            ("1 oz", Dimension::Mass, OUNCE_KG),
            ("1 m/s", Dimension::Velocity, 1.0),
            ("2 m/s", Dimension::Velocity, 2.0),
            ("-10 °C", Dimension::Temperature, -10.0),
            ("30 °C", Dimension::Temperature, 30.0),
            ("1 fps", Dimension::Velocity, FOOT_M),
            ("1 ft/s", Dimension::Velocity, FOOT_M),
            ("1 km/h", Dimension::Velocity, KMH_MPS),
            ("1 mph", Dimension::Velocity, MPH_MPS),
            ("1 kn", Dimension::Velocity, KN_MPS),
            ("15°C", Dimension::Temperature, 15.0),
            ("59°F", Dimension::Temperature, 15.0),
            ("288.15 K", Dimension::Temperature, 15.0),
            ("1013.25 hPa", Dimension::Pressure, 101_325.0),
            ("1013.25 mbar", Dimension::Pressure, 101_325.0),
            ("29.921252401894762 inHg", Dimension::Pressure, 101_325.0),
            ("760 mmHg", Dimension::Pressure, 760.0 * PASCAL_PER_MMHG),
            ("1 psi", Dimension::Pressure, PASCAL_PER_PSI),
            ("1 mrad", Dimension::Angle, MRAD_RAD),
            ("1 mil", Dimension::Angle, MRAD_RAD),
            ("1 MOA", Dimension::Angle, MOA_RAD),
            ("1 IPHY", Dimension::Angle, IPHY_RAD),
            ("10 cm@100m", Dimension::Angle, MRAD_RAD),
            ("1 J", Dimension::Energy, 1.0),
            ("1 ftlb", Dimension::Energy, JOULE_PER_FT_LBF),
            ("1 ft·lbf", Dimension::Energy, JOULE_PER_FT_LBF),
        ];
        for (text, dim, expected) in cases {
            let got = parse_quantity(text, dim, None).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert!((got - expected).abs() < 1e-9, "{text}: {got} != {expected}");
        }
    }

    #[test]
    fn decimal_comma_fraction_and_grain_rejection() {
        let v = parse_quantity("1,5 m", Dimension::Length, None).unwrap();
        assert!((v - 1.5).abs() < 1e-12);
        let q = parse_quantity("1/4 MOA", Dimension::Angle, None).unwrap();
        assert!((q - 0.25 * MOA_RAD).abs() < 1e-15);
        let err = parse_quantity("10 gr.", Dimension::Mass, None).unwrap_err();
        assert!(err.to_string().contains("ambiguous"));
        let err = parse_quantity("10 g", Dimension::Length, None).unwrap_err();
        assert!(err.to_string().contains("dimension"));
    }

    #[test]
    fn a_bare_length_suggests_an_example_suffix() {
        let err = parse_quantity("100", Dimension::Length, None).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("100m"), "{text}");
        assert!(text.contains("100"), "{text}");
    }

    #[test]
    fn angular_identities_from_the_plan() {
        let one_mrad = MRAD_RAD;
        let moa = rad_to_moa(one_mrad);
        assert!((moa - 10.8 / std::f64::consts::PI).abs() < 1e-12);
        assert!((moa - 3.4377).abs() < 5e-5);
        assert!((rad_to_iphy(one_mrad) - 3.6).abs() < 1e-12);
        assert!((rad_to_cm_per_100m(one_mrad) - 10.0).abs() < 1e-12);
        let one_moa_iphy = rad_to_iphy(MOA_RAD);
        assert!((one_moa_iphy - std::f64::consts::PI / 3.0).abs() < 1e-12);
        assert!((one_moa_iphy * 10_000.0).round() as i32 == 10_472);
    }

    #[test]
    fn display_conversion_does_not_replace_the_stored_value() {
        let si = parse_quantity("45 mm", Dimension::Length, None).unwrap();
        let shown = format_quantity(si, "in").unwrap();
        assert_eq!(shown, "1.7717");
        let parsed_again = parse_quantity("45 mm", Dimension::Length, None).unwrap();
        assert_eq!(si.to_bits(), parsed_again.to_bits());
        assert!((convert_from_si(15.0, "F").unwrap() - 59.0).abs() < 1e-12);
        assert_eq!(format_quantity(15.0, "C").unwrap(), "15");
    }

    #[test]
    fn temperature_offset_is_not_a_scale() {
        assert!((fahrenheit_to_celsius(32.0)).abs() < 1e-12);
        assert!((celsius_to_fahrenheit(15.0) - 59.0).abs() < 1e-12);
    }

    #[test]
    fn twist_and_wind_examples() {
        let t = parse_twist("1:8in").unwrap();
        assert!((t.metres_per_turn - 8.0 * INCH_M).abs() < 1e-12);
        assert_eq!(t.hand, TwistHand::Right);
        let t = parse_twist("LH 1:203mm").unwrap();
        assert!((t.metres_per_turn - 0.203).abs() < 1e-12);
        assert_eq!(t.hand, TwistHand::Left);
        let w = parse_wind("3mps@9oc").unwrap();
        assert!((w.speed_mps - 3.0).abs() < 1e-12);
        assert!((w.from_clock_rad - 1.5 * std::f64::consts::PI).abs() < 1e-12);
    }
}
