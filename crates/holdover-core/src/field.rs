//! Point-blank range, moving-target lead, truing, recoil, and chronograph statistics.
//!
//! These call the same trajectory functions as `solve`. They do not read files.

use crate::drag::CdScale;
use crate::error::{Error, Result};
use crate::solve::{self, hold, AngularHold, Click, Shot, TableSpec, Warning};
use crate::units::Wind;

/// Guidance shown beside an MV fit. The fit still runs when the shot is not supersonic.
pub const MV_GUIDANCE: &str = "Before transonic";
/// Guidance shown beside a drag fit.
pub const DRAG_GUIDANCE: &str = "True MV first";
/// Guidance shown beside a tracking fit.
pub const TRACKING_GUIDANCE: &str = "Affects clicks only";

/// Powder-gas speed used when the recoil form leaves ejecta velocity empty.
///
/// 4000 ft/s is a common published assumption. It has not been confirmed against
/// the SAAMI publication, and the app always says so.
pub const DEFAULT_EJECTA_MPS: f64 = 4000.0 * 0.3048;
pub const EJECTA_CONVENTION: &str = "Constant 1219.2 m/s (4000 ft/s). This is a common published powder-gas assumption. It has not been confirmed against the SAAMI publication.";

const PBR_STEP_M: f64 = 5.0;
const MV_WARN_FRACTION: f64 = 0.03;
const BC_WARN_FRACTION: f64 = 0.10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragMode {
    /// One Cd multiplier over the whole Mach range. The BC multiplier is its inverse.
    Multiplier,
    /// One Cd multiplier per observation, stored on the Mach band around that shot.
    MachBands,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PbrSample {
    pub range_m: f64,
    pub path_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pbr {
    pub radius_m: f64,
    pub near_m: f64,
    pub far_m: f64,
    pub optimal_zero_m: f64,
    /// Where the bullet first rises through the line of sight.
    pub near_zero_m: Option<f64>,
    /// Where the bullet comes back down through the line of sight.
    pub far_zero_m: Option<f64>,
    /// The muzzle is already inside the vital zone, so the point-blank interval starts at 0.
    pub starts_inside: bool,
    /// The path leaves through the top of the zone before it falls out the bottom.
    pub exits_above: bool,
    pub ordinate_m: f64,
    pub ordinate_range_m: f64,
    pub bore_angle_rad: f64,
    pub samples: Vec<PbrSample>,
    pub warnings: Vec<Warning>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lead {
    pub range_m: f64,
    pub impact_range_m: f64,
    pub tof_s: f64,
    pub iterations: u32,
    pub cross_mps: f64,
    pub along_mps: f64,
    pub lead_m: f64,
    pub reaction_s: f64,
    pub reaction_lead_m: f64,
    /// Time-of-flight lead, positive to the right.
    pub lead: AngularHold,
    /// Time-of-flight lead plus the reaction allowance.
    pub with_reaction: AngularHold,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MvFit {
    pub mv_mps: f64,
    pub previous_mv_mps: f64,
    pub change_fraction: f64,
    pub large_change: bool,
    pub residual_rad: f64,
    pub before_transonic: bool,
    pub mach: f64,
    pub guidance: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DragFit {
    pub bc_multiplier: f64,
    pub cd_multiplier: f64,
    pub pieces: Vec<CdScale>,
    pub change_fraction: f64,
    pub large_change: bool,
    pub residual_rad: f64,
    pub guidance: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecoilInput {
    pub rifle_mass_kg: f64,
    pub bullet_mass_kg: f64,
    pub charge_mass_kg: f64,
    pub mv_mps: f64,
    pub ejecta_mps: f64,
    /// Fraction removed from the recoil energy. Zero leaves the formula alone.
    pub reduction_fraction: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recoil {
    pub velocity_mps: f64,
    pub energy_j: f64,
    pub velocity_fps: f64,
    pub energy_ft_lbf: f64,
    pub ejecta_mps: f64,
    pub reduction_fraction: f64,
    pub reduction_is_estimate: bool,
    pub convention: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chrono {
    pub count: usize,
    pub mean_mps: f64,
    pub sd_mps: f64,
    pub es_mps: f64,
    pub min_mps: f64,
    pub max_mps: f64,
    /// Indexes into the original list. The shots stay in the mean and the SD.
    pub outliers: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PowderFit {
    pub slope_mps_per_c: f64,
    pub intercept_mps: f64,
    pub r_squared: f64,
    pub n: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunningPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub range_m: f64,
    pub speed_mps: f64,
    pub speed_known: bool,
    pub note: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardPage {
    pub id: &'static str,
    pub name: &'static str,
    pub width_mm: f64,
    pub height_mm: f64,
}

/// Swedish running-target speeds from the published track times in the plan.
pub fn running_presets() -> &'static [RunningPreset] {
    &[
        RunningPreset {
            id: "algbana",
            name: "Älgbana",
            range_m: 80.0,
            speed_mps: 5.3,
            speed_known: true,
            note: "80 m. The opening is 22.75–23.25 m and the run is 4.3 ± 0.2 s, about 5.3 m/s.",
        },
        RunningPreset {
            id: "viltmal",
            name: "Viltmålsbana",
            range_m: 50.0,
            speed_mps: 2.0,
            speed_known: true,
            note: "50 m. A normal run is 10 m in 5 s, so 2 m/s.",
        },
        RunningPreset {
            id: "boar-algbana",
            name: "Vildsvinspasset, älgbana speed",
            range_m: 30.0,
            speed_mps: 5.3,
            speed_known: true,
            note: "30 m running boar on a half-screened älgbana, using the moose run speed of 5.3 m/s.",
        },
        RunningPreset {
            id: "boar-fast",
            name: "Vildsvinspasset, fast run",
            range_m: 30.0,
            speed_mps: 0.0,
            speed_known: false,
            note: "30 m on the 50 m track. The SvSF/ISSF fast-run speed is not in Holdover. Enter it from the rulebook.",
        },
    ]
}

pub fn card_pages() -> &'static [CardPage] {
    &[
        CardPage {
            id: "compact",
            name: "Compact",
            width_mm: 90.0,
            height_mm: 50.0,
        },
        CardPage {
            id: "a6",
            name: "A6",
            width_mm: 105.0,
            height_mm: 148.0,
        },
        CardPage {
            id: "a5",
            name: "A5",
            width_mm: 148.0,
            height_mm: 210.0,
        },
        CardPage {
            id: "a4",
            name: "A4",
            width_mm: 210.0,
            height_mm: 297.0,
        },
        CardPage {
            id: "letter",
            name: "Letter",
            width_mm: 215.9,
            height_mm: 279.4,
        },
    ]
}

pub fn card_page(id: &str) -> Option<&'static CardPage> {
    card_pages().iter().find(|page| page.id == id)
}

/// Printed bar length. Compact stock cards are narrower than 10 cm, so they use a 5 cm bar.
pub fn scale_bar_mm(page: &CardPage) -> f64 {
    if page.width_mm >= 110.0 {
        100.0
    } else {
        50.0
    }
}

/// Print stylesheet for one card size. Lengths are millimetres. There are no pixel sizes.
pub fn print_sheet_css(page: &CardPage) -> String {
    let bar = scale_bar_mm(page);
    let margin_mm = 10.0;
    let inner = (page.width_mm - 2.0 * margin_mm).max(bar);
    format!(
        "@media print {{\n\
         @page {{ size: {width}mm {height}mm; margin: {margin}mm; }}\n\
         html, body, .page, .card, .print-sheet {{\n\
           width: {inner}mm; max-width: {inner}mm; margin: 0; padding: 0;\n\
         }}\n\
         .scale-bar-screen {{ display: none; }}\n\
         .scale-bar-print {{\n\
           display: block !important; box-sizing: border-box !important;\n\
           width: {bar}mm !important; min-width: {bar}mm !important; max-width: {bar}mm !important;\n\
           height: 8mm !important;\n\
         }}\n\
         }}\n",
        width = trim_mm(page.width_mm),
        height = trim_mm(page.height_mm),
        margin = trim_mm(margin_mm),
        inner = trim_mm(inner),
        bar = trim_mm(bar),
    )
}

fn trim_mm(value: f64) -> String {
    if (value - value.round()).abs() < 1.0e-6 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}

/// Optimise the bore so the apex reaches `+r`, then report where the path leaves `−r`.
///
/// The search is in still air. `vital_diameter_m` is the full zone, so the radius is half of it.
pub fn pbr(shot: &Shot, vital_diameter_m: f64) -> Result<Pbr> {
    if !(vital_diameter_m.is_finite() && vital_diameter_m > 0.0) {
        return Err(Error::Invalid("the vital zone diameter must be positive"));
    }
    let radius = vital_diameter_m * 0.5;
    let mut lo = 0.0;
    let mut hi = 0.002;
    let mut flo = max_path(shot, lo)? - radius;
    let mut fhi = max_path(shot, hi)? - radius;
    for _ in 0..16 {
        if flo <= 0.0 && fhi >= 0.0 {
            break;
        }
        if fhi < 0.0 {
            hi += 0.004;
            if hi > 0.2 {
                break;
            }
            fhi = max_path(shot, hi)? - radius;
        } else if flo > 0.0 {
            lo -= 0.002;
            flo = max_path(shot, lo)? - radius;
        }
    }
    if flo > 0.0 || fhi < 0.0 {
        return Err(Error::Invalid(
            "this load cannot put the apex on the vital-zone radius",
        ));
    }
    let bore = solve::brent(
        lo,
        hi,
        flo,
        fhi,
        |theta| Ok(max_path(shot, theta)? - radius),
        5.0e-4,
    )
    .map_err(|_| Error::Invalid("point-blank range did not find a bore angle for this zone"))?;
    let mut samples = sample_path(shot, bore, 800.0)?;
    if crossing(&samples, 0, -radius, true).is_none() {
        samples = sample_path(shot, bore, 1600.0)?;
    }
    let (apex_index, ordinate_range_m, ordinate_m) = apex(&samples);
    Ok(finish_pbr(
        radius,
        bore,
        samples,
        apex_index,
        ordinate_range_m,
        ordinate_m,
        true,
    ))
}

/// Point-blank limits for the shot's own zero. The bore is not moved to touch the zone.
pub fn pbr_fixed(shot: &Shot, vital_diameter_m: f64) -> Result<Pbr> {
    if !(vital_diameter_m.is_finite() && vital_diameter_m > 0.0) {
        return Err(Error::Invalid("the vital zone diameter must be positive"));
    }
    if shot.zero_range_m.is_none() && shot.bore_angle_rad.is_none() {
        return Err(Error::Invalid("this profile has no zero range"));
    }
    let radius = vital_diameter_m * 0.5;
    let bore = solve::solve_zero(shot)?.bore_angle_rad;
    let mut samples = sample_path(shot, bore, 800.0)?;
    if crossing(&samples, 0, -radius, true).is_none()
        && crossing(&samples, 0, radius, false).is_none()
    {
        samples = sample_path(shot, bore, 1600.0)?;
    }
    let (apex_index, ordinate_range_m, ordinate_m) = apex(&samples);
    Ok(finish_pbr(
        radius,
        bore,
        samples,
        apex_index,
        ordinate_range_m,
        ordinate_m,
        false,
    ))
}

fn finish_pbr(
    radius: f64,
    bore: f64,
    samples: Vec<(f64, f64)>,
    apex_index: usize,
    ordinate_range_m: f64,
    ordinate_m: f64,
    bore_was_optimised: bool,
) -> Pbr {
    let first = samples.first().map(|sample| sample.1).unwrap_or(0.0);
    let starts_inside = first >= -radius && first <= radius;
    let exit_top = crossing(&samples, 0, radius, false);
    let exits_above = exit_top.is_some_and(|range| range + 1.0e-6 < ordinate_range_m);
    let near_m = if starts_inside {
        0.0
    } else if first < -radius {
        crossing(&samples, 0, -radius, false).unwrap_or(0.0)
    } else {
        exit_top.unwrap_or(0.0)
    };
    let far_bottom = crossing(&samples, apex_index, -radius, true);
    let far_m = if bore_was_optimised {
        far_bottom.unwrap_or(samples.last().map(|sample| sample.0).unwrap_or(near_m))
    } else if exits_above {
        exit_top.unwrap_or(near_m)
    } else {
        far_bottom.unwrap_or(samples.last().map(|sample| sample.0).unwrap_or(near_m))
    };
    let near_zero_m = crossing(&samples, 0, 0.0, false);
    let far_zero_m = crossing(&samples, apex_index, 0.0, true);
    Pbr {
        radius_m: radius,
        near_m,
        far_m,
        optimal_zero_m: far_zero_m.unwrap_or(0.0),
        near_zero_m,
        far_zero_m,
        starts_inside,
        exits_above,
        ordinate_m,
        ordinate_range_m,
        bore_angle_rad: bore,
        samples: samples
            .into_iter()
            .map(|(range_m, path_m)| PbrSample { range_m, path_m })
            .collect(),
        warnings: vec![],
    }
}

/// `lead = V · sin(angle) · TOF`, iterated three times so along-track motion can change the range.
///
/// `direction_rad` is clockwise from 12 o'clock, the same sense as wind. 3 o'clock is to the right.
/// `reaction_s` is added only as a labelled user assumption.
pub fn lead(
    shot: &Shot,
    range_m: f64,
    target_speed_mps: f64,
    direction_rad: f64,
    reaction_s: f64,
) -> Result<Lead> {
    if range_m <= 0.0 {
        return Err(Error::Invalid("lead range must be positive"));
    }
    if target_speed_mps < 0.0 || reaction_s < 0.0 {
        return Err(Error::Invalid(
            "target speed and reaction time cannot be negative",
        ));
    }
    let along = target_speed_mps * direction_rad.cos();
    let cross = target_speed_mps * direction_rad.sin();
    let mut impact = range_m;
    let mut tof = 0.0;
    for _ in 0..3 {
        impact = impact.max(1.0);
        let solved = solve::solve(shot, impact)?;
        tof = solved
            .points
            .first()
            .map(|point| point.tof_s)
            .unwrap_or(0.0);
        impact = (range_m + along * tof).max(1.0);
    }
    let lead_m = cross * tof;
    let reaction_lead_m = cross * reaction_s;
    let lead_hold = hold(lead_m, impact, false, shot.click.as_ref());
    let with_reaction = hold(lead_m + reaction_lead_m, impact, false, shot.click.as_ref());
    Ok(Lead {
        range_m,
        impact_range_m: impact,
        tof_s: tof,
        iterations: 3,
        cross_mps: cross,
        along_mps: along,
        lead_m,
        reaction_s,
        reaction_lead_m,
        lead: lead_hold,
        with_reaction,
    })
}

/// Fit the muzzle velocity so a re-zeroed shot matches one observed elevation.
pub fn true_mv(shot: &Shot, range_m: f64, observed_elevation_rad: f64) -> Result<MvFit> {
    if shot.zero_range_m.is_none() {
        return Err(Error::Invalid("muzzle-velocity truing needs a zero range"));
    }
    if range_m <= 0.0 {
        return Err(Error::Invalid("the observation range must be positive"));
    }
    let previous = shot.mv_mps;
    if previous <= 0.0 {
        return Err(Error::Invalid("muzzle velocity must be positive"));
    }
    let mv = root_scale(
        previous,
        |mv| Ok(elevation_of(shot, Some(mv), None, range_m)? - observed_elevation_rad),
        1.0e-6,
    )?;
    let mut fitted = shot.clone();
    fitted.mv_mps = mv;
    fitted.bore_angle_rad = None;
    let solved = solve::solve(&fitted, range_m)?;
    let point = &solved.points[0];
    let before_transonic = match solved.transonic_range_m {
        Some(crossed) => range_m + 1.0 < crossed && point.mach > 1.05,
        None => point.mach > 1.05,
    };
    let change = (mv - previous) / previous;
    Ok(MvFit {
        mv_mps: mv,
        previous_mv_mps: previous,
        change_fraction: change,
        large_change: change.abs() > MV_WARN_FRACTION,
        residual_rad: point.elevation.rad - observed_elevation_rad,
        before_transonic,
        mach: point.mach,
        guidance: MV_GUIDANCE,
    })
}

/// Fit a Cd multiplier, or one multiplier per observation, so the elevations match.
pub fn true_drag(shot: &Shot, observations: &[(f64, f64)], mode: DragMode) -> Result<DragFit> {
    if observations.is_empty() {
        return Err(Error::Invalid("drag truing needs an observation"));
    }
    if shot.zero_range_m.is_none() {
        return Err(Error::Invalid("drag truing needs a zero range"));
    }
    for (range, _) in observations {
        if *range <= 0.0 {
            return Err(Error::Invalid("an observation range must be positive"));
        }
    }
    match mode {
        DragMode::Multiplier => fit_multiplier(shot, observations),
        DragMode::MachBands => fit_bands(shot, observations),
    }
}

/// `observed / advertised`. A factor above 1 means each click moves more than the advertised value.
pub fn true_tracking(advertised_rad: f64, observed_rad: f64) -> Result<f64> {
    if advertised_rad.abs() < 1.0e-9 {
        return Err(Error::Invalid(
            "the dialed angle has to be larger than zero",
        ));
    }
    let factor = observed_rad / advertised_rad;
    if !(factor.is_finite() && factor > 0.0) {
        return Err(Error::Invalid("the tracking factor must be positive"));
    }
    Ok(factor)
}

pub fn recoil(input: RecoilInput) -> Result<Recoil> {
    if input.rifle_mass_kg <= 0.0 || input.bullet_mass_kg <= 0.0 || input.mv_mps <= 0.0 {
        return Err(Error::Invalid(
            "rifle weight, bullet weight, and muzzle velocity must be positive",
        ));
    }
    if input.charge_mass_kg < 0.0 || input.ejecta_mps < 0.0 {
        return Err(Error::Invalid(
            "charge weight and ejecta velocity cannot be negative",
        ));
    }
    if !(0.0..1.0).contains(&input.reduction_fraction) {
        return Err(Error::Invalid(
            "the reduction is a fraction from 0 up to, but not including, 1",
        ));
    }
    let momentum = input.bullet_mass_kg * input.mv_mps + input.charge_mass_kg * input.ejecta_mps;
    let mut velocity = momentum / input.rifle_mass_kg;
    let mut energy = 0.5 * input.rifle_mass_kg * velocity * velocity;
    if input.reduction_fraction > 0.0 {
        energy *= 1.0 - input.reduction_fraction;
        velocity = (2.0 * energy / input.rifle_mass_kg).sqrt();
    }
    Ok(Recoil {
        velocity_mps: velocity,
        energy_j: energy,
        velocity_fps: velocity / 0.3048,
        energy_ft_lbf: energy / (0.3048 * 0.45359237 * 9.80665),
        ejecta_mps: input.ejecta_mps,
        reduction_fraction: input.reduction_fraction,
        reduction_is_estimate: input.reduction_fraction > 0.0,
        convention: EJECTA_CONVENTION,
    })
}

/// Sample standard deviation. Outliers are flagged at 2 SD when there are at least four shots.
pub fn chrono_stats(shots_mps: &[f64]) -> Result<Chrono> {
    if shots_mps.is_empty() {
        return Err(Error::Invalid("a chronograph session needs a shot"));
    }
    if shots_mps
        .iter()
        .any(|shot| !(shot.is_finite() && *shot > 0.0))
    {
        return Err(Error::Invalid("shot speeds must be positive"));
    }
    let count = shots_mps.len();
    let mean = shots_mps.iter().sum::<f64>() / count as f64;
    let sd = if count >= 2 {
        let sum = shots_mps
            .iter()
            .map(|shot| (shot - mean).powi(2))
            .sum::<f64>();
        (sum / (count as f64 - 1.0)).sqrt()
    } else {
        0.0
    };
    let min = shots_mps.iter().copied().fold(f64::INFINITY, f64::min);
    let max = shots_mps.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut outliers = Vec::new();
    if count >= 4 && sd > 0.0 {
        for (index, shot) in shots_mps.iter().enumerate() {
            if (shot - mean).abs() > 2.0 * sd {
                outliers.push(index);
            }
        }
    }
    Ok(Chrono {
        count,
        mean_mps: mean,
        sd_mps: sd,
        es_mps: max - min,
        min_mps: min,
        max_mps: max,
        outliers,
    })
}

/// Linear regression of muzzle velocity against temperature. Points are `(°C, m/s)`.
pub fn powder_coefficient(points: &[(f64, f64)]) -> Result<PowderFit> {
    if points.len() < 2 {
        return Err(Error::Invalid(
            "a powder-temperature fit needs sessions at two temperatures",
        ));
    }
    let n = points.len() as f64;
    let mean_x = points.iter().map(|point| point.0).sum::<f64>() / n;
    let mean_y = points.iter().map(|point| point.1).sum::<f64>() / n;
    let mut ss_xx = 0.0;
    let mut ss_xy = 0.0;
    for (x, y) in points {
        ss_xx += (x - mean_x) * (x - mean_x);
        ss_xy += (x - mean_x) * (y - mean_y);
    }
    if ss_xx < 1.0e-12 {
        return Err(Error::Invalid(
            "the sessions are at one temperature, so there is nothing to fit",
        ));
    }
    let slope = ss_xy / ss_xx;
    let intercept = mean_y - slope * mean_x;
    let mut ss_tot = 0.0;
    let mut ss_res = 0.0;
    for (x, y) in points {
        let predicted = intercept + slope * x;
        ss_tot += (y - mean_y).powi(2);
        ss_res += (y - predicted).powi(2);
    }
    let r_squared = if ss_tot < 1.0e-18 {
        1.0
    } else {
        1.0 - ss_res / ss_tot
    };
    Ok(PowderFit {
        slope_mps_per_c: slope,
        intercept_mps: intercept,
        r_squared,
        n: points.len(),
    })
}

/// Vertical path at `mv ± sd` with the bore angle fixed by the nominal zero.
pub fn vertical_mv_spread_m(shot: &Shot, range_m: f64, sd_mps: f64) -> Result<(f64, f64)> {
    if sd_mps < 0.0 {
        return Err(Error::Invalid("a velocity SD cannot be negative"));
    }
    let zero = solve::solve_zero(shot)?;
    let mut slowed = shot.clone();
    slowed.bore_angle_rad = Some(zero.bore_angle_rad);
    slowed.mv_mps = (shot.mv_mps - sd_mps).max(1.0);
    let mut faster = slowed.clone();
    faster.mv_mps = shot.mv_mps + sd_mps;
    let low = solve::solve(&slowed, range_m)?.points[0].path_m;
    let high = solve::solve(&faster, range_m)?.points[0].path_m;
    Ok((low, high))
}

pub fn combine_offsets(
    first_m: f64,
    second_m: f64,
    range_m: f64,
    elevation: bool,
    click: Option<&Click>,
) -> AngularHold {
    hold(first_m + second_m, range_m.max(1.0e-9), elevation, click)
}

fn fit_multiplier(shot: &Shot, observations: &[(f64, f64)]) -> Result<DragFit> {
    let scale = root_scale(
        1.0,
        |scale| {
            let mut sum = 0.0;
            for (range, observed) in observations {
                sum += elevation_of(shot, None, Some(scale), *range)? - observed;
            }
            Ok(sum)
        },
        1.0e-5,
    )?;
    let mut total = 0.0;
    for (range, observed) in observations {
        total += elevation_of(shot, None, Some(scale), *range)? - observed;
    }
    let mean_residual = total / observations.len() as f64;
    Ok(drag_fit(scale, mean_residual, vec![full_band(scale)]))
}

fn fit_bands(shot: &Shot, observations: &[(f64, f64)]) -> Result<DragFit> {
    let mut fitted = Vec::new();
    let mut residual_sum = 0.0;
    for (range, observed) in observations {
        let target = *observed;
        let scale = root_scale(
            1.0,
            |scale| Ok(elevation_of(shot, None, Some(scale), *range)? - target),
            1.0e-5,
        )?;
        residual_sum += elevation_of(shot, None, Some(scale), *range)? - target;
        let solved = {
            let mut trial = shot.clone();
            trial.bore_angle_rad = None;
            trial.cd_scales = vec![full_band(scale)];
            solve::solve(&trial, *range)?
        };
        fitted.push((solved.points[0].mach, scale));
    }
    fitted.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
    let mut pieces = Vec::new();
    for (index, (mach, scale)) in fitted.iter().enumerate() {
        let lo = if index == 0 {
            0.0
        } else {
            (fitted[index - 1].0 + mach) * 0.5
        };
        let hi = if index + 1 == fitted.len() {
            10.0
        } else {
            (mach + fitted[index + 1].0) * 0.5
        };
        pieces.push(CdScale {
            mach_min: lo,
            mach_max: hi,
            multiplier: *scale,
        });
    }
    let scale = fitted.last().map(|item| item.1).unwrap_or(1.0);
    Ok(drag_fit(
        scale,
        residual_sum / observations.len() as f64,
        pieces,
    ))
}

fn drag_fit(cd_multiplier: f64, residual_rad: f64, pieces: Vec<CdScale>) -> DragFit {
    let bc_multiplier = 1.0 / cd_multiplier;
    let change = (bc_multiplier - 1.0).abs();
    DragFit {
        bc_multiplier,
        cd_multiplier,
        pieces,
        change_fraction: bc_multiplier - 1.0,
        large_change: change > BC_WARN_FRACTION,
        residual_rad,
        guidance: DRAG_GUIDANCE,
    }
}

fn full_band(multiplier: f64) -> CdScale {
    CdScale {
        mach_min: 0.0,
        mach_max: 10.0,
        multiplier,
    }
}

fn elevation_of(shot: &Shot, mv: Option<f64>, scale: Option<f64>, range_m: f64) -> Result<f64> {
    let mut trial = shot.clone();
    trial.bore_angle_rad = None;
    if let Some(mv) = mv {
        trial.mv_mps = mv;
    }
    if let Some(scale) = scale {
        trial.cd_scales = vec![full_band(scale)];
    }
    let solved = solve::solve(&trial, range_m)?;
    Ok(solved.points[0].elevation.rad)
}

fn root_scale<F>(seed: f64, mut f: F, tol: f64) -> Result<f64>
where
    F: FnMut(f64) -> Result<f64>,
{
    let mut samples = Vec::new();
    let mut value = seed * 0.35;
    for _ in 0..22 {
        samples.push(value);
        value *= 1.12;
        if value > seed * 2.8 {
            break;
        }
    }
    let mut previous: Option<(f64, f64)> = None;
    for sample in samples {
        let y = match f(sample) {
            Ok(y) if y.is_finite() => y,
            Ok(_) => continue,
            Err(Error::RangeNotReached { .. }) => continue,
            Err(err) => return Err(err),
        };
        if let Some((left, left_y)) = previous {
            if left_y == 0.0 {
                return Ok(left);
            }
            if left_y * y <= 0.0 {
                return solve::brent(left, sample, left_y, y, &mut f, tol)
                    .map_err(|_| Error::Invalid("the fit did not converge"));
            }
        }
        previous = Some((sample, y));
    }
    Err(Error::Invalid(
        "the observation is outside the range this load can be fitted to",
    ))
}

fn max_path(shot: &Shot, bore: f64) -> Result<f64> {
    let samples = sample_path(shot, bore, 800.0)?;
    Ok(samples
        .iter()
        .map(|sample| sample.1)
        .fold(f64::NEG_INFINITY, f64::max))
}

fn sample_path(shot: &Shot, bore: f64, mut limit: f64) -> Result<Vec<(f64, f64)>> {
    let mut trial = shot.clone();
    trial.bore_angle_rad = Some(bore);
    trial.wind = Wind::default();
    for _ in 0..5 {
        match solve::table(
            &trial,
            TableSpec {
                start_m: 0.0,
                end_m: limit,
                step_m: PBR_STEP_M,
            },
        ) {
            Ok(solved) => {
                return Ok(solved
                    .points
                    .iter()
                    .map(|point| (point.range_m, point.path_m))
                    .collect());
            }
            Err(Error::RangeNotReached { reached_m, .. }) => {
                limit = (reached_m - PBR_STEP_M).max(PBR_STEP_M * 4.0);
            }
            Err(err) => return Err(err),
        }
    }
    Err(Error::Invalid(
        "the trajectory did not stay up long enough for a point-blank search",
    ))
}

fn apex(samples: &[(f64, f64)]) -> (usize, f64, f64) {
    let mut index = 0;
    let mut best = f64::NEG_INFINITY;
    for (i, (_, path)) in samples.iter().enumerate() {
        if *path > best {
            best = *path;
            index = i;
        }
    }
    (index, samples[index].0, best)
}

fn crossing(samples: &[(f64, f64)], start: usize, level: f64, descending: bool) -> Option<f64> {
    if samples.len() < 2 || start >= samples.len() {
        return None;
    }
    for index in start..samples.len() - 1 {
        let (x0, y0) = samples[index];
        let (x1, y1) = samples[index + 1];
        let hit = if descending {
            y0 >= level && y1 < level
        } else {
            y0 <= level && y1 > level
        };
        if hit {
            let dy = y1 - y0;
            let t = if dy.abs() < 1.0e-15 {
                0.0
            } else {
                (level - y0) / dy
            };
            return Some(x0 + t.clamp(0.0, 1.0) * (x1 - x0));
        }
    }
    None
}
