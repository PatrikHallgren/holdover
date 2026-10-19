//! Standard Cd–Mach curves and the drag factor k(M).

use std::sync::OnceLock;

use crate::constants::BC_LB_PER_IN2_TO_KG_PER_M2;
use crate::error::{Error, Result};
use crate::pchip::Spline;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragModel {
    G1,
    G2,
    G5,
    G6,
    G7,
    G8,
}

impl DragModel {
    pub fn parse(name: &str) -> Result<Self> {
        match name.trim().to_ascii_uppercase().as_str() {
            "G1" => Ok(Self::G1),
            "G2" => Ok(Self::G2),
            "G5" => Ok(Self::G5),
            "G6" => Ok(Self::G6),
            "G7" => Ok(Self::G7),
            "G8" => Ok(Self::G8),
            _ => Err(Error::Parse {
                what: "drag model",
                input: name.to_string(),
                reason: "expected G1, G2, G5, G6, G7 or G8",
            }),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::G1 => "G1",
            Self::G2 => "G2",
            Self::G5 => "G5",
            Self::G6 => "G6",
            Self::G7 => "G7",
            Self::G8 => "G8",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BcReference {
    Icao,
    ArmyStandardMetro,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BcBand {
    pub bc_lb_per_in2: f64,
    /// Band applies at airspeeds greater than or equal to this, m/s.
    pub min_mps: f64,
    pub reference: BcReference,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DragSpec {
    Standard {
        model: DragModel,
        bands: Vec<BcBand>,
    },
    Custom {
        points: Vec<(f64, f64)>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CdSample {
    pub cd: f64,
    pub clamped: bool,
}

/// A Mach band whose drag coefficient is multiplied while the bullet is inside it.
///
/// A multiplier above 1 is more drag (a lower effective BC). An empty list means
/// the entered BC is used as-is.
#[derive(Clone, Debug, PartialEq)]
pub struct CdScale {
    pub mach_min: f64,
    pub mach_max: f64,
    pub multiplier: f64,
}

pub fn standard_cd(model: DragModel, mach: f64) -> CdSample {
    let (cd, clamped) = spline(model).eval_clamped(mach);
    CdSample { cd, clamped }
}

pub(crate) struct PreparedDrag {
    kind: Kind,
    scales: Vec<CdScale>,
}

enum Kind {
    Standard {
        model: DragModel,
        bands: Vec<BcBand>,
    },
    Custom {
        spline: Spline,
        sectional_density: f64,
    },
}

impl PreparedDrag {
    pub(crate) fn prepare(spec: &DragSpec, mass_kg: f64, diameter_m: f64) -> Result<Self> {
        if mass_kg <= 0.0 || diameter_m <= 0.0 {
            return Err(Error::Invalid(
                "bullet weight and diameter must be positive",
            ));
        }
        match spec {
            DragSpec::Standard { model, bands } => {
                if bands.is_empty() {
                    return Err(Error::Invalid(
                        "a standard drag model needs at least one BC",
                    ));
                }
                for band in bands {
                    if band.bc_lb_per_in2 <= 0.0 {
                        return Err(Error::Invalid("BC must be positive"));
                    }
                }
                let mut bands = bands.clone();
                bands.sort_by(|a, b| a.min_mps.partial_cmp(&b.min_mps).unwrap());
                Ok(Self {
                    kind: Kind::Standard {
                        model: *model,
                        bands,
                    },
                    scales: Vec::new(),
                })
            }
            DragSpec::Custom { points } => {
                let spline = Spline::new(
                    points.iter().map(|p| p.0).collect(),
                    points.iter().map(|p| p.1).collect(),
                )?;
                Ok(Self {
                    kind: Kind::Custom {
                        spline,
                        sectional_density: mass_kg / (diameter_m * diameter_m),
                    },
                    scales: Vec::new(),
                })
            }
        }
    }

    pub(crate) fn set_scales(&mut self, scales: Vec<CdScale>) -> Result<()> {
        for scale in &scales {
            if !(scale.multiplier.is_finite() && scale.multiplier > 0.0) {
                return Err(Error::Invalid("a drag scale must be positive"));
            }
            if !(scale.mach_min.is_finite() && scale.mach_max.is_finite())
                || scale.mach_max < scale.mach_min
            {
                return Err(Error::Invalid("a drag-scale Mach band is reversed"));
            }
        }
        self.scales = scales;
        Ok(())
    }

    /// Drag factor k such that acceleration is `-k |v_rel| v_rel`.
    /// Also reports whether Cd was clamped.
    pub(crate) fn k(&self, mach: f64, airspeed: f64, density: f64) -> (f64, bool) {
        if density <= 0.0 {
            return (0.0, false);
        }
        let (cd, clamped, bc_si) = match &self.kind {
            Kind::Standard { model, bands } => {
                let sample = standard_cd(*model, mach);
                let bc = select_bc(bands, airspeed);
                (sample.cd, sample.clamped, bc * BC_LB_PER_IN2_TO_KG_PER_M2)
            }
            Kind::Custom {
                spline,
                sectional_density,
            } => {
                let (cd, clamped) = spline.eval_clamped(mach);
                (cd, clamped, *sectional_density)
            }
        };
        let k =
            density * cd * cd_multiplier(&self.scales, mach) * std::f64::consts::PI / (8.0 * bc_si);
        (k, clamped)
    }
}

fn cd_multiplier(scales: &[CdScale], mach: f64) -> f64 {
    let mut chosen = 1.0;
    let mut best_min = f64::NEG_INFINITY;
    for scale in scales {
        if mach + 1.0e-12 >= scale.mach_min
            && mach <= scale.mach_max + 1.0e-12
            && scale.mach_min >= best_min
        {
            chosen = scale.multiplier;
            best_min = scale.mach_min;
        }
    }
    chosen
}

fn select_bc(bands: &[BcBand], airspeed: f64) -> f64 {
    let mut chosen = bands[0].bc_lb_per_in2;
    for band in bands {
        if airspeed + 1e-12 >= band.min_mps {
            chosen = band.bc_lb_per_in2;
        }
    }
    chosen
}

fn spline(model: DragModel) -> &'static Spline {
    match model {
        DragModel::G1 => G1.get_or_init(|| load(include_str!("../data/g1.txt"))),
        DragModel::G2 => G2.get_or_init(|| load(include_str!("../data/g2.txt"))),
        DragModel::G5 => G5.get_or_init(|| load(include_str!("../data/g5.txt"))),
        DragModel::G6 => G6.get_or_init(|| load(include_str!("../data/g6.txt"))),
        DragModel::G7 => G7.get_or_init(|| load(include_str!("../data/g7.txt"))),
        DragModel::G8 => G8.get_or_init(|| load(include_str!("../data/g8.txt"))),
    }
}

fn load(text: &str) -> Spline {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let mach: f64 = parts.next().unwrap().parse().unwrap();
        let cd: f64 = parts.next().unwrap().parse().unwrap();
        xs.push(mach);
        ys.push(cd);
    }
    Spline::new(xs, ys).expect("bundled drag table")
}

static G1: OnceLock<Spline> = OnceLock::new();
static G2: OnceLock<Spline> = OnceLock::new();
static G5: OnceLock<Spline> = OnceLock::new();
static G6: OnceLock<Spline> = OnceLock::new();
static G7: OnceLock<Spline> = OnceLock::new();
static G8: OnceLock<Spline> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g1_and_g7_match_oracle_pchip() {
        let g1_093 = standard_cd(DragModel::G1, 0.93);
        assert!(!g1_093.clamped);
        assert!((g1_093.cd - 0.380_170_45).abs() < 2e-8);
        assert!((standard_cd(DragModel::G1, 1.0).cd - 0.4805).abs() < 1e-12);
        assert!((standard_cd(DragModel::G1, 0.0).cd - 0.2629).abs() < 1e-12);
        let g7 = standard_cd(DragModel::G7, 0.93);
        assert!((g7.cd - 0.171_672_06).abs() < 2e-8);
        let clamped = standard_cd(DragModel::G1, 6.0);
        assert!(clamped.clamped);
        assert!((clamped.cd - standard_cd(DragModel::G1, 5.0).cd).abs() < 1e-12);
    }

    #[test]
    fn multi_bc_is_a_step() {
        let spec = DragSpec::Standard {
            model: DragModel::G7,
            bands: vec![
                BcBand {
                    bc_lb_per_in2: 0.2,
                    min_mps: 0.0,
                    reference: BcReference::Icao,
                },
                BcBand {
                    bc_lb_per_in2: 0.4,
                    min_mps: 400.0,
                    reference: BcReference::Icao,
                },
            ],
        };
        let prepared = PreparedDrag::prepare(&spec, 0.01, 0.0078).unwrap();
        let (k_slow, _) = prepared.k(2.0, 300.0, 1.225);
        let (k_fast, _) = prepared.k(2.0, 500.0, 1.225);
        assert!((k_slow / k_fast - 2.0).abs() < 1e-12);
    }

    #[test]
    fn a_cd_scale_multiplies_drag_inside_its_band() {
        let spec = DragSpec::Standard {
            model: DragModel::G7,
            bands: vec![BcBand {
                bc_lb_per_in2: 0.3,
                min_mps: 0.0,
                reference: BcReference::Icao,
            }],
        };
        let mut prepared = PreparedDrag::prepare(&spec, 0.01, 0.0078).unwrap();
        let (base, _) = prepared.k(2.0, 700.0, 1.225);
        prepared
            .set_scales(vec![CdScale {
                mach_min: 1.5,
                mach_max: 3.0,
                multiplier: 1.25,
            }])
            .unwrap();
        let (scaled, _) = prepared.k(2.0, 700.0, 1.225);
        let (outside, _) = prepared.k(1.0, 340.0, 1.225);
        let plain = PreparedDrag::prepare(&spec, 0.01, 0.0078).unwrap();
        let (plain_low, _) = plain.k(1.0, 340.0, 1.225);
        assert!((scaled / base - 1.25).abs() < 1e-12);
        assert!((outside - plain_low).abs() < 1e-12);
    }
}
