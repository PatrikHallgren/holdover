//! Holdover reticle files, the generic set, and FFP/SFP subtension.
//!
//! A file stores nominal angles in milliradians or MOA. First-focal-plane
//! marks keep that angle at every magnification. Second-focal-plane marks
//! scale with `calibrated_magnification / current_magnification`.
//!
//! The generic set is drawn in this file. It is not a manufacturer reticle.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

pub const FORMAT: &str = "holdover-reticle";
pub const VERSION: u32 = 1;
/// Starting apparent field of view for the eyepiece view. It is not a scope specification.
pub const DEFAULT_APPARENT_FOV_DEG: f64 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FocalPlane {
    Ffp,
    Sfp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReticleUnit {
    Mrad,
    Moa,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reticle {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub name: String,
    pub focal_plane: FocalPlane,
    pub unit: ReticleUnit,
    #[serde(default = "default_calibrated")]
    pub calibrated_magnification: f64,
    pub licence: String,
    pub source: String,
    pub elements: Vec<Mark>,
}

fn default_calibrated() -> f64 {
    10.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Mark {
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        #[serde(default = "thin")]
        weight: f64,
    },
    Hash {
        x: f64,
        y: f64,
        length: f64,
        /// Degrees. 90 is a vertical hash.
        angle: f64,
        #[serde(default = "thin")]
        weight: f64,
    },
    Dot {
        x: f64,
        y: f64,
        diameter: f64,
    },
    Circle {
        x: f64,
        y: f64,
        diameter: f64,
    },
    Polyline {
        points: Vec<[f64; 2]>,
        #[serde(default)]
        closed: bool,
    },
    Text {
        x: f64,
        y: f64,
        text: String,
    },
}

fn thin() -> f64 {
    0.04
}

#[derive(Clone, Copy, Debug)]
pub struct Eyepiece {
    pub magnification: f64,
    pub apparent_fov_deg: f64,
    pub viewport_px: f64,
}

impl Eyepiece {
    /// Pixels per true milliradian. Apparent field is fixed, so the true field is `apparent / magnification`.
    pub fn px_per_mrad(self) -> Result<f64> {
        if self.magnification <= 0.0 {
            return Err(Error::Invalid("magnification must be positive"));
        }
        if self.apparent_fov_deg <= 0.0 || self.apparent_fov_deg > 80.0 {
            return Err(Error::Invalid(
                "apparent field of view must be between 0 and 80 degrees",
            ));
        }
        if self.viewport_px <= 0.0 {
            return Err(Error::Invalid("viewport must be positive"));
        }
        let apparent_mrad = self.apparent_fov_deg.to_radians() * 1000.0;
        Ok(self.viewport_px / (apparent_mrad / self.magnification))
    }

    pub fn pixels(self, true_mrad: f64) -> Result<f64> {
        Ok(true_mrad * self.px_per_mrad()?)
    }
}

/// `actual = nominal` for FFP. For SFP, `actual = nominal * calibrated / current`.
pub fn subtension_factor(
    focal: FocalPlane,
    calibrated_magnification: f64,
    current_magnification: f64,
) -> Result<f64> {
    if current_magnification <= 0.0 {
        return Err(Error::Invalid("magnification must be positive"));
    }
    match focal {
        FocalPlane::Ffp => Ok(1.0),
        FocalPlane::Sfp => {
            if calibrated_magnification <= 0.0 {
                return Err(Error::Invalid(
                    "an SFP reticle needs a calibrated magnification",
                ));
            }
            Ok(calibrated_magnification / current_magnification)
        }
    }
}

pub fn to_mrad(value: f64, unit: ReticleUnit) -> f64 {
    match unit {
        ReticleUnit::Mrad => value,
        ReticleUnit::Moa => value * std::f64::consts::PI / 10.8,
    }
}

pub fn angular_mrad(size_m: f64, range_m: f64) -> Result<f64> {
    if range_m <= 0.0 {
        return Err(Error::Invalid("range must be positive"));
    }
    if size_m < 0.0 {
        return Err(Error::Invalid("a target size cannot be negative"));
    }
    Ok(size_m / range_m * 1000.0)
}

/// Pixel span of a target feature. This is the rendering path the 1 px check uses.
pub fn target_diameter_px(diameter_m: f64, range_m: f64, eye: Eyepiece) -> Result<f64> {
    eye.pixels(angular_mrad(diameter_m, range_m)?)
}

/// Centimetres per true milliradian at a range. 1 mrad is `range_m / 10` centimetres.
pub fn cm_per_mrad(range_m: f64) -> Result<f64> {
    if range_m <= 0.0 {
        return Err(Error::Invalid("range must be positive"));
    }
    Ok(range_m / 10.0)
}

pub fn sfp_warning(reticle: &Reticle, current_magnification: f64) -> bool {
    reticle.focal_plane == FocalPlane::Sfp
        && (current_magnification - reticle.calibrated_magnification).abs() > 0.05
}

pub fn parse_reticle(value: &Value) -> Result<Reticle> {
    let reticle: Reticle = serde_json::from_value(value.clone())
        .map_err(|_| Error::Message("this is not a Holdover reticle file".into()))?;
    validate(&reticle)?;
    Ok(reticle)
}

pub fn reticle_json(reticle: &Reticle) -> Value {
    serde_json::to_value(reticle).unwrap_or(Value::Null)
}

fn validate(reticle: &Reticle) -> Result<()> {
    if reticle.format != FORMAT {
        return Err(Error::Message(
            "a reticle file must use the Holdover reticle format".into(),
        ));
    }
    if reticle.version != VERSION {
        return Err(Error::Message(
            "this reticle file version is not supported".into(),
        ));
    }
    if reticle.name.trim().is_empty() {
        return Err(Error::Invalid("a reticle needs a name"));
    }
    if !matches!(reticle.licence.as_str(), "in-house" | "user-drawn") {
        return Err(Error::Invalid(
            "reticle licence must be in-house or user-drawn",
        ));
    }
    if reticle.elements.is_empty() {
        return Err(Error::Invalid("a reticle needs at least one mark"));
    }
    if reticle.elements.len() > 4000 {
        return Err(Error::Invalid("a reticle has too many marks"));
    }
    if reticle.focal_plane == FocalPlane::Sfp && reticle.calibrated_magnification <= 0.0 {
        return Err(Error::Invalid(
            "an SFP reticle needs a calibrated magnification",
        ));
    }
    for mark in &reticle.elements {
        if !mark_finite(mark) {
            return Err(Error::Invalid(
                "a reticle mark has a non-numeric coordinate",
            ));
        }
    }
    Ok(())
}

fn mark_finite(mark: &Mark) -> bool {
    match mark {
        Mark::Line {
            x1,
            y1,
            x2,
            y2,
            weight,
        } => [x1, y1, x2, y2, weight].iter().all(|v| v.is_finite()),
        Mark::Hash {
            x,
            y,
            length,
            angle,
            weight,
        } => [x, y, length, angle, weight].iter().all(|v| v.is_finite()),
        Mark::Dot { x, y, diameter } | Mark::Circle { x, y, diameter } => {
            [x, y, diameter].iter().all(|v| v.is_finite()) && *diameter > 0.0
        }
        Mark::Polyline { points, .. } => {
            !points.is_empty()
                && points
                    .iter()
                    .all(|point| point[0].is_finite() && point[1].is_finite())
        }
        Mark::Text { x, y, text } => x.is_finite() && y.is_finite() && !text.is_empty(),
    }
}

/// Nominal marks converted to true milliradians at `current_magnification`.
pub fn marks_true_mrad(reticle: &Reticle, current_magnification: f64) -> Result<Vec<Mark>> {
    let factor = subtension_factor(
        reticle.focal_plane,
        reticle.calibrated_magnification,
        current_magnification,
    )?;
    let unit = reticle.unit;
    let mut out = Vec::with_capacity(reticle.elements.len());
    for mark in &reticle.elements {
        out.push(scale_mark(mark, unit, factor));
    }
    Ok(out)
}

fn scale_mark(mark: &Mark, unit: ReticleUnit, factor: f64) -> Mark {
    let s = |value: f64| to_mrad(value, unit) * factor;
    match mark {
        Mark::Line {
            x1,
            y1,
            x2,
            y2,
            weight,
        } => Mark::Line {
            x1: s(*x1),
            y1: s(*y1),
            x2: s(*x2),
            y2: s(*y2),
            weight: s(*weight),
        },
        Mark::Hash {
            x,
            y,
            length,
            angle,
            weight,
        } => Mark::Hash {
            x: s(*x),
            y: s(*y),
            length: s(*length),
            angle: *angle,
            weight: s(*weight),
        },
        Mark::Dot { x, y, diameter } => Mark::Dot {
            x: s(*x),
            y: s(*y),
            diameter: s(*diameter),
        },
        Mark::Circle { x, y, diameter } => Mark::Circle {
            x: s(*x),
            y: s(*y),
            diameter: s(*diameter),
        },
        Mark::Polyline { points, closed } => Mark::Polyline {
            points: points
                .iter()
                .map(|point| [s(point[0]), s(point[1])])
                .collect(),
            closed: *closed,
        },
        Mark::Text { x, y, text } => Mark::Text {
            x: s(*x),
            y: s(*y),
            text: text.clone(),
        },
    }
}

pub fn snap_value(value: f64, step: f64) -> f64 {
    if step <= 0.0 || !value.is_finite() {
        return value;
    }
    (value / step).round() * step
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MirrorAxis {
    Horizontal,
    Vertical,
}

pub fn mirror_reticle(reticle: &Reticle, axis: MirrorAxis) -> Reticle {
    let mut copy = reticle.clone();
    copy.elements = copy
        .elements
        .iter()
        .map(|mark| mirror_mark(mark, axis))
        .collect();
    copy
}

fn mirror_mark(mark: &Mark, axis: MirrorAxis) -> Mark {
    let flip = |x: f64, y: f64| match axis {
        MirrorAxis::Horizontal => (-x, y),
        MirrorAxis::Vertical => (x, -y),
    };
    match mark {
        Mark::Line {
            x1,
            y1,
            x2,
            y2,
            weight,
        } => {
            let (a, b) = flip(*x1, *y1);
            let (c, d) = flip(*x2, *y2);
            Mark::Line {
                x1: a,
                y1: b,
                x2: c,
                y2: d,
                weight: *weight,
            }
        }
        Mark::Hash {
            x,
            y,
            length,
            angle,
            weight,
        } => {
            let (x, y) = flip(*x, *y);
            let angle = match axis {
                MirrorAxis::Horizontal => 180.0 - angle,
                MirrorAxis::Vertical => -angle,
            };
            Mark::Hash {
                x,
                y,
                length: *length,
                angle,
                weight: *weight,
            }
        }
        Mark::Dot { x, y, diameter } => {
            let (x, y) = flip(*x, *y);
            Mark::Dot {
                x,
                y,
                diameter: *diameter,
            }
        }
        Mark::Circle { x, y, diameter } => {
            let (x, y) = flip(*x, *y);
            Mark::Circle {
                x,
                y,
                diameter: *diameter,
            }
        }
        Mark::Polyline { points, closed } => Mark::Polyline {
            points: points
                .iter()
                .map(|point| {
                    let (x, y) = flip(point[0], point[1]);
                    [x, y]
                })
                .collect(),
            closed: *closed,
        },
        Mark::Text { x, y, text } => {
            let (x, y) = flip(*x, *y);
            Mark::Text {
                x,
                y,
                text: text.clone(),
            }
        }
    }
}

/// Extra copies stepped by `dx` and `dy` in the reticle's own unit. `copies` includes the original.
pub fn repeat_reticle(reticle: &Reticle, dx: f64, dy: f64, copies: u32) -> Result<Reticle> {
    if !(1..=40).contains(&copies) {
        return Err(Error::Invalid("repeat between 1 and 40 copies"));
    }
    let mut copy = reticle.clone();
    let base = reticle.elements.clone();
    copy.elements.clear();
    for index in 0..copies {
        let ox = dx * f64::from(index);
        let oy = dy * f64::from(index);
        for mark in &base {
            copy.elements.push(translate(mark, ox, oy));
        }
    }
    if copy.elements.len() > 4000 {
        return Err(Error::Invalid("a reticle has too many marks"));
    }
    Ok(copy)
}

fn translate(mark: &Mark, dx: f64, dy: f64) -> Mark {
    match mark {
        Mark::Line {
            x1,
            y1,
            x2,
            y2,
            weight,
        } => Mark::Line {
            x1: x1 + dx,
            y1: y1 + dy,
            x2: x2 + dx,
            y2: y2 + dy,
            weight: *weight,
        },
        Mark::Hash {
            x,
            y,
            length,
            angle,
            weight,
        } => Mark::Hash {
            x: x + dx,
            y: y + dy,
            length: *length,
            angle: *angle,
            weight: *weight,
        },
        Mark::Dot { x, y, diameter } => Mark::Dot {
            x: x + dx,
            y: y + dy,
            diameter: *diameter,
        },
        Mark::Circle { x, y, diameter } => Mark::Circle {
            x: x + dx,
            y: y + dy,
            diameter: *diameter,
        },
        Mark::Polyline { points, closed } => Mark::Polyline {
            points: points
                .iter()
                .map(|point| [point[0] + dx, point[1] + dy])
                .collect(),
            closed: *closed,
        },
        Mark::Text { x, y, text } => Mark::Text {
            x: x + dx,
            y: y + dy,
            text: text.clone(),
        },
    }
}

pub fn generic_reticles() -> Vec<Reticle> {
    vec![
        duplex(),
        mil_dot(),
        moa_hash(),
        mil_hash(),
        mil_tree(),
        centre_dot(),
        dot_circle(),
    ]
}

pub fn generic_by_id(id: &str) -> Option<Reticle> {
    generic_reticles().into_iter().find(|item| item.id == id)
}

fn base(id: &str, name: &str, unit: ReticleUnit) -> Reticle {
    Reticle {
        format: FORMAT.into(),
        version: VERSION,
        id: id.into(),
        name: name.into(),
        focal_plane: FocalPlane::Ffp,
        unit,
        calibrated_magnification: 10.0,
        licence: "in-house".into(),
        source: "Holdover generic set. Drawn in-house. Not a manufacturer reticle.".into(),
        elements: Vec::new(),
    }
}

fn line(x1: f64, y1: f64, x2: f64, y2: f64, weight: f64) -> Mark {
    Mark::Line {
        x1,
        y1,
        x2,
        y2,
        weight,
    }
}

fn duplex() -> Reticle {
    let mut reticle = base("generic-duplex", "Duplex", ReticleUnit::Mrad);
    let posts = [
        line(-8.0, 0.0, -1.4, 0.0, 0.22),
        line(1.4, 0.0, 8.0, 0.0, 0.22),
        line(0.0, -8.0, 0.0, -1.4, 0.22),
        line(0.0, 1.4, 0.0, 8.0, 0.22),
        line(-1.0, 0.0, 1.0, 0.0, 0.03),
        line(0.0, -1.0, 0.0, 1.0, 0.03),
    ];
    reticle.elements.extend(posts);
    reticle
}

fn mil_dot() -> Reticle {
    let mut reticle = base("generic-mil-dot", "Mil-dot", ReticleUnit::Mrad);
    reticle.elements.push(line(-5.0, 0.0, 5.0, 0.0, 0.035));
    reticle.elements.push(line(0.0, -5.0, 0.0, 5.0, 0.035));
    for step in 1..=4 {
        let at = f64::from(step);
        for (x, y) in [(at, 0.0), (-at, 0.0), (0.0, at), (0.0, -at)] {
            reticle.elements.push(Mark::Dot {
                x,
                y,
                diameter: 0.2,
            });
        }
    }
    reticle
}

fn moa_hash() -> Reticle {
    let mut reticle = base("generic-moa-hash", "MOA hash", ReticleUnit::Moa);
    reticle.elements.push(line(-20.0, 0.0, 20.0, 0.0, 0.15));
    reticle.elements.push(line(0.0, -20.0, 0.0, 20.0, 0.15));
    for step in -20..=20 {
        if step == 0 {
            continue;
        }
        let at = f64::from(step);
        let long = step % 5 == 0;
        let length = if long { 1.0 } else { 0.45 };
        reticle.elements.push(Mark::Hash {
            x: at,
            y: 0.0,
            length,
            angle: 90.0,
            weight: 0.12,
        });
        reticle.elements.push(Mark::Hash {
            x: 0.0,
            y: at,
            length,
            angle: 0.0,
            weight: 0.12,
        });
    }
    reticle
}

fn mil_hash() -> Reticle {
    let mut reticle = base("generic-mil-hash", "Mil-hash", ReticleUnit::Mrad);
    reticle.elements.push(line(-5.0, 0.0, 5.0, 0.0, 0.035));
    reticle.elements.push(line(0.0, -5.0, 0.0, 5.0, 0.035));
    let mut at = -5.0_f64;
    while at <= 5.0 + 1e-9 {
        if at.abs() > 1e-9 {
            let whole = (at * 2.0).round() % 2.0 == 0.0;
            let length = if whole { 0.5 } else { 0.25 };
            reticle.elements.push(Mark::Hash {
                x: at,
                y: 0.0,
                length,
                angle: 90.0,
                weight: 0.03,
            });
            reticle.elements.push(Mark::Hash {
                x: 0.0,
                y: at,
                length,
                angle: 0.0,
                weight: 0.03,
            });
        }
        at += 0.5;
    }
    reticle
}

fn mil_tree() -> Reticle {
    let mut reticle = base("generic-mil-tree", "Mil-tree", ReticleUnit::Mrad);
    reticle.elements.push(line(-4.0, 0.0, 4.0, 0.0, 0.035));
    reticle.elements.push(line(0.0, 1.0, 0.0, -6.0, 0.035));
    let mut at = -4.0_f64;
    while at <= 4.0 + 1e-9 {
        if at.abs() > 1e-9 {
            reticle.elements.push(Mark::Hash {
                x: at,
                y: 0.0,
                length: if (at * 2.0).round() % 2.0 == 0.0 {
                    0.4
                } else {
                    0.2
                },
                angle: 90.0,
                weight: 0.03,
            });
        }
        at += 0.5;
    }
    for row in 1..=6 {
        let y = -f64::from(row);
        let dots = row;
        for col in 1..=dots {
            let x = 0.5 * f64::from(col);
            reticle.elements.push(Mark::Dot {
                x,
                y,
                diameter: 0.12,
            });
            reticle.elements.push(Mark::Dot {
                x: -x,
                y,
                diameter: 0.12,
            });
        }
    }
    reticle
}

fn centre_dot() -> Reticle {
    let mut reticle = base("generic-dot", "Centre dot", ReticleUnit::Mrad);
    reticle.elements.push(Mark::Dot {
        x: 0.0,
        y: 0.0,
        diameter: 0.15,
    });
    reticle
}

fn dot_circle() -> Reticle {
    let mut reticle = base("generic-dot-circle", "Dot and circle", ReticleUnit::Mrad);
    reticle.elements.push(Mark::Dot {
        x: 0.0,
        y: 0.0,
        diameter: 0.12,
    });
    reticle.elements.push(Mark::Circle {
        x: 0.0,
        y: 0.0,
        diameter: 2.0,
    });
    reticle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sfp_subtension_at_three_magnifications_and_ffp_stays_put() {
        let mut reticle = generic_by_id("generic-mil-dot").unwrap();
        reticle.focal_plane = FocalPlane::Sfp;
        reticle.calibrated_magnification = 10.0;
        let eye = |mag: f64| Eyepiece {
            magnification: mag,
            apparent_fov_deg: 22.0,
            viewport_px: 1000.0,
        };
        let nominal = 1.0_f64;
        let at_cal = eye(10.0).pixels(nominal).unwrap();
        for (mag, factor) in [(5.0, 2.0), (10.0, 1.0), (20.0, 0.5)] {
            let scale = subtension_factor(FocalPlane::Sfp, 10.0, mag).unwrap();
            assert!((scale - factor).abs() < 1e-12, "{scale}");
            let px = eye(mag).pixels(nominal * scale).unwrap();
            assert!(
                (px - at_cal).abs() < 1.0,
                "SFP eyepiece size at {mag}× is {px}, calibrated is {at_cal}"
            );
        }
        let ffp = |mag: f64| subtension_factor(FocalPlane::Ffp, 10.0, mag).unwrap();
        assert!((ffp(5.0) - 1.0).abs() < 1e-12);
        assert!((ffp(20.0) - 1.0).abs() < 1e-12);
        let px5 = eye(5.0).pixels(1.0).unwrap();
        let px10 = eye(10.0).pixels(1.0).unwrap();
        let px20 = eye(20.0).pixels(1.0).unwrap();
        assert!((px10 - 2.0 * px5).abs() < 1.0);
        assert!((px20 - 2.0 * px10).abs() < 1.0);

        let marks = marks_true_mrad(&reticle, 20.0).unwrap();
        let Mark::Dot { x, diameter, .. } = marks
            .iter()
            .find(|mark| matches!(mark, Mark::Dot { x, .. } if (*x - 0.5).abs() < 1e-9))
            .unwrap()
        else {
            unreachable!()
        };
        assert!((x - 0.5).abs() < 1e-9);
        assert!((diameter - 0.1).abs() < 1e-9);
    }

    #[test]
    fn target_angular_size_matches_within_one_pixel() {
        let cases = [
            (0.15, 100.0, 10.0, 22.0, 1000.0),
            (0.415, 80.0, 12.0, 21.5, 1440.0),
            (0.10, 50.0, 6.0, 18.0, 900.0),
        ];
        for (diameter_m, range_m, mag, fov, viewport) in cases {
            let eye = Eyepiece {
                magnification: mag,
                apparent_fov_deg: fov,
                viewport_px: viewport,
            };
            let got = target_diameter_px(diameter_m, range_m, eye).unwrap();
            let mrad = diameter_m / range_m * 1000.0;
            let apparent = fov.to_radians() * 1000.0;
            let expected = mrad / (apparent / mag) * viewport;
            assert!(
                (got - expected).abs() < 1.0,
                "got {got} expected {expected}"
            );
            let cm = diameter_m * 100.0;
            let half = cm / 2.0;
            let px_per = eye.px_per_mrad().unwrap();
            let cm_to_mrad = 10.0 / range_m;
            let span = (half * cm_to_mrad - (-half * cm_to_mrad)) * px_per;
            assert!((span - got).abs() < 1.0, "edge span {span} diameter {got}");
        }
    }

    #[test]
    fn generic_set_has_no_maker_names_and_round_trips() {
        let banned = [
            "nightforce",
            "leupold",
            "vortex",
            "schmidt",
            "kahles",
            "swarovski",
            "zeiss",
            "horus",
            "tremor",
            "bushnell",
            "primary arms",
            "athlon",
            "sightron",
            "steiner",
            "meopta",
            "hawke",
            "tangent",
            "minox",
        ];
        let set = generic_reticles();
        assert!(set.len() >= 6);
        for reticle in &set {
            let blob = format!(
                "{} {} {}",
                reticle.id.to_ascii_lowercase(),
                reticle.name.to_ascii_lowercase(),
                reticle.source.to_ascii_lowercase()
            );
            for word in banned {
                assert!(!blob.contains(word), "{} contains {word}", reticle.name);
            }
            assert_eq!(reticle.licence, "in-house");
            assert_eq!(reticle.focal_plane, FocalPlane::Ffp);
            let json = reticle_json(reticle);
            let parsed = parse_reticle(&json).unwrap();
            assert_eq!(parsed, *reticle);
        }
    }

    #[test]
    fn mirror_and_repeat_and_snap() {
        let reticle = generic_by_id("generic-dot").unwrap();
        let mirrored = mirror_reticle(&reticle, MirrorAxis::Horizontal);
        assert_eq!(mirrored.elements.len(), 1);
        let repeated = repeat_reticle(&generic_by_id("generic-dot").unwrap(), 1.0, 0.0, 3).unwrap();
        assert_eq!(repeated.elements.len(), 3);
        assert!((snap_value(1.24, 0.5) - 1.0).abs() < 1e-12);
        assert!((snap_value(1.26, 0.5) - 1.5).abs() < 1e-12);
    }
}
