//! Zeroing and trajectory points.
//!
//! Integration is in the world frame. Slant range is distance along the line
//! of sight. A supplied bore angle is used as-is; otherwise Brent's method
//! finds the angle that crosses the line of sight at the zero range.

use crate::atmosphere::{AirState, Atmosphere};
use crate::constants::STANDARD_GRAVITY;
use crate::drag::{DragSpec, PreparedDrag};
use crate::error::{Error, Result};
use crate::stability::{sg_for_shot, spin_drift_m};
use crate::traj::{self, Flight, Frame, Stepper};
use crate::units::{self, Twist, TwistHand, Wind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effects {
    pub incline: bool,
    pub spin_drift: bool,
    pub coriolis: bool,
    pub cant: bool,
    pub aerodynamic_jump: bool,
}

impl Default for Effects {
    fn default() -> Self {
        Self {
            incline: true,
            spin_drift: true,
            coriolis: false,
            cant: false,
            aerodynamic_jump: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Click {
    pub value_rad: f64,
    pub tracking_factor: f64,
    pub travel_up_rad: Option<f64>,
    pub travel_down_rad: Option<f64>,
    pub travel_right_rad: Option<f64>,
    pub travel_left_rad: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bullet {
    pub weight_kg: f64,
    pub diameter_m: f64,
    pub length_m: Option<f64>,
    pub drag: DragSpec,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Integrator {
    Dopri { rtol: f64, atol: f64 },
    Rk4 { dt: f64 },
}

impl Default for Integrator {
    fn default() -> Self {
        Self::Dopri {
            rtol: 1.0e-7,
            atol: 1.0e-8,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Shot {
    pub mv_mps: f64,
    pub mv_temperature_c: Option<f64>,
    pub powder_temperature_c: Option<f64>,
    /// Powder temperature used only while solving the zero. `None` uses the zero atmosphere's air temperature.
    pub zero_powder_temperature_c: Option<f64>,
    /// Added to muzzle velocity per degree Celsius above the measurement temperature.
    pub temperature_sensitivity_mps_per_c: Option<f64>,
    pub bullet: Bullet,
    pub sight_height_m: f64,
    pub zero_range_m: Option<f64>,
    pub zero_offset_m: f64,
    pub zero_atmosphere: Option<Atmosphere>,
    pub atmosphere: Atmosphere,
    pub inclination_rad: f64,
    pub cant_rad: f64,
    /// When set, the zero solve is skipped and this bore angle is used.
    pub bore_angle_rad: Option<f64>,
    pub twist: Option<Twist>,
    pub click: Option<Click>,
    pub wind: Wind,
    pub effects: Effects,
    pub latitude_rad: f64,
    pub azimuth_rad: f64,
    /// Standard gravity unless a test overrides it.
    pub gravity_mps2: f64,
    pub integrator: Integrator,
}

impl Shot {
    pub fn basic(mv_mps: f64, bullet: Bullet, atmosphere: Atmosphere) -> Self {
        Self {
            mv_mps,
            mv_temperature_c: None,
            powder_temperature_c: None,
            zero_powder_temperature_c: None,
            temperature_sensitivity_mps_per_c: None,
            bullet,
            sight_height_m: 0.0,
            zero_range_m: None,
            zero_offset_m: 0.0,
            zero_atmosphere: None,
            atmosphere,
            inclination_rad: 0.0,
            cant_rad: 0.0,
            bore_angle_rad: None,
            twist: None,
            click: None,
            wind: Wind::default(),
            effects: Effects::default(),
            latitude_rad: 0.0,
            azimuth_rad: 0.0,
            gravity_mps2: STANDARD_GRAVITY,
            integrator: Integrator::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AngularHold {
    /// Signed hold in radians. Positive elevation is up. Positive windage is right.
    pub rad: f64,
    pub mrad: f64,
    pub moa: f64,
    pub iphy: f64,
    pub cm_per_100m: f64,
    pub direction: Direction,
    pub clicks: Option<i32>,
    pub residual_mrad: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Warning {
    DragClamped,
    DefaultZeroAtmosphere,
    PressureKindSuspicious,
    AerodynamicJumpNotApplied,
    SpinDriftMissingLength,
    ExceedsTravel { axis: &'static str },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrajectoryPoint {
    pub range_m: f64,
    pub path_m: f64,
    pub drop_m: f64,
    pub windage_m: f64,
    pub velocity_mps: f64,
    pub energy_j: f64,
    pub energy_ft_lbf: f64,
    pub tof_s: f64,
    pub mach: f64,
    pub elevation: AngularHold,
    pub windage: AngularHold,
    pub sg: Option<f64>,
    pub sg_corrected: Option<f64>,
    pub spin_drift_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ZeroSolution {
    pub bore_angle_rad: f64,
    /// `None` when the shot already carried an explicit bore angle.
    pub crossing_error_m: Option<f64>,
    pub warnings: Vec<Warning>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Solution {
    pub bore_angle_rad: f64,
    pub zero_crossing_error_m: Option<f64>,
    pub points: Vec<TrajectoryPoint>,
    pub warnings: Vec<Warning>,
    pub transonic_range_m: Option<f64>,
    pub density_kg_m3: f64,
    pub speed_of_sound_mps: f64,
    pub density_altitude_m: f64,
    pub muzzle_velocity_mps: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct TableSpec {
    pub start_m: f64,
    pub end_m: f64,
    pub step_m: f64,
}

pub fn solve_zero(shot: &Shot) -> Result<ZeroSolution> {
    let (angle, error, warnings) = bore_angle(shot)?;
    Ok(ZeroSolution {
        bore_angle_rad: angle,
        crossing_error_m: error,
        warnings,
    })
}

pub fn solve(shot: &Shot, range_m: f64) -> Result<Solution> {
    solve_ranges(shot, &[range_m])
}

pub fn table(shot: &Shot, spec: TableSpec) -> Result<Solution> {
    if spec.step_m <= 0.0 {
        return Err(Error::Invalid("table step must be positive"));
    }
    if spec.end_m + 1e-9 < spec.start_m {
        return Err(Error::Invalid("table end is before the start"));
    }
    let mut ranges = Vec::new();
    let mut range = spec.start_m;
    let mut guard = 0;
    while range <= spec.end_m + spec.step_m * 1e-9 {
        ranges.push(range);
        range += spec.step_m;
        guard += 1;
        if guard > 100_000 {
            return Err(Error::Invalid("table has too many rows"));
        }
    }
    solve_ranges(shot, &ranges)
}

pub fn pbr() -> Result<()> {
    Err(Error::NotInMilestone {
        feature: "point-blank range",
        milestone: "M3",
    })
}

pub fn lead() -> Result<()> {
    Err(Error::NotInMilestone {
        feature: "moving-target lead",
        milestone: "M3",
    })
}

pub fn true_mv() -> Result<()> {
    Err(Error::NotInMilestone {
        feature: "muzzle-velocity truing",
        milestone: "M3",
    })
}

pub fn true_drag() -> Result<()> {
    Err(Error::NotInMilestone {
        feature: "drag truing",
        milestone: "M3",
    })
}

pub fn group_stats() -> Result<()> {
    Err(Error::NotInMilestone {
        feature: "group statistics",
        milestone: "M8",
    })
}

fn solve_ranges(shot: &Shot, ranges: &[f64]) -> Result<Solution> {
    let (bore, zero_error, mut warnings) = bore_angle(shot)?;
    let air = shot.atmosphere.state()?;
    if air.pressure_kind_suspicious {
        warnings.push(Warning::PressureKindSuspicious);
    }
    if shot.effects.aerodynamic_jump {
        warnings.push(Warning::AerodynamicJumpNotApplied);
    }
    let mv = adjusted_mv_with(
        shot,
        shot.atmosphere.temperature_c,
        shot.powder_temperature_c,
    );
    let flight = integrate_shot(shot, bore, mv, &air, ranges)?;
    if flight.drag_clamped {
        warnings.push(Warning::DragClamped);
    }
    let (sg, sg_corrected, spin_ready) = stability_of(shot, mv, &air);
    if shot.effects.spin_drift && shot.twist.is_some() && shot.bullet.length_m.is_none() {
        warnings.push(Warning::SpinDriftMissingLength);
    }
    let mut points = Vec::with_capacity(flight.samples.len());
    for sample in &flight.samples {
        let mut pos = sample.position;
        let spin = if spin_ready && shot.effects.spin_drift {
            let right = shot.twist.unwrap().hand == TwistHand::Right;
            spin_drift_m(sg_corrected.unwrap(), sample.time_s, right)
        } else {
            0.0
        };
        pos[2] += spin;
        let frame = frame_for(shot, bore);
        let miss = traj::miss(&frame, pos);
        let elevation = hold(
            -miss.scope_above_m,
            sample.range_m.max(1e-9),
            true,
            shot.click.as_ref(),
        );
        let windage = hold(
            -miss.scope_right_m,
            sample.range_m.max(1e-9),
            false,
            shot.click.as_ref(),
        );
        if let Some(click) = shot.click {
            if exceeds(elevation.rad, click.travel_up_rad, click.travel_down_rad) {
                warnings.push(Warning::ExceedsTravel { axis: "elevation" });
            }
            if exceeds(windage.rad, click.travel_right_rad, click.travel_left_rad) {
                warnings.push(Warning::ExceedsTravel { axis: "windage" });
            }
        }
        let speed = traj::norm(sample.velocity);
        points.push(TrajectoryPoint {
            range_m: sample.range_m,
            path_m: miss.above_m,
            drop_m: miss.drop_m,
            windage_m: miss.right_m,
            velocity_mps: speed,
            energy_j: units::joules(shot.bullet.weight_kg, speed),
            energy_ft_lbf: units::foot_pounds(shot.bullet.weight_kg, speed),
            tof_s: sample.time_s,
            mach: sample.mach,
            elevation,
            windage,
            sg,
            sg_corrected,
            spin_drift_m: spin,
        });
    }
    warnings.dedup_by(|a, b| a == b);
    Ok(Solution {
        bore_angle_rad: bore,
        zero_crossing_error_m: zero_error,
        points,
        warnings,
        transonic_range_m: flight.transonic_range_m,
        density_kg_m3: air.density_kg_m3,
        speed_of_sound_mps: air.speed_of_sound_mps,
        density_altitude_m: air.density_altitude_m,
        muzzle_velocity_mps: mv,
    })
}

fn exceeds(hold_rad: f64, positive_limit: Option<f64>, negative_limit: Option<f64>) -> bool {
    if hold_rad > 0.0 {
        positive_limit.is_some_and(|limit| hold_rad > limit)
    } else {
        negative_limit.is_some_and(|limit| -hold_rad > limit)
    }
}

fn hold(offset_m: f64, range_m: f64, elevation: bool, click: Option<&Click>) -> AngularHold {
    let rad = (offset_m / range_m).atan();
    let direction = if rad.abs() < 1e-9 {
        Direction::None
    } else if elevation {
        if rad > 0.0 {
            Direction::Up
        } else {
            Direction::Down
        }
    } else if rad > 0.0 {
        Direction::Right
    } else {
        Direction::Left
    };
    let (clicks, residual_mrad) = if let Some(click) = click {
        let step = click.value_rad * click.tracking_factor;
        if step.abs() < 1e-15 {
            (None, None)
        } else {
            let n = (rad / step).round() as i32;
            let residual = rad - n as f64 * step;
            (Some(n), Some(units::rad_to_mrad(residual)))
        }
    } else {
        (None, None)
    };
    AngularHold {
        rad,
        mrad: units::rad_to_mrad(rad),
        moa: units::rad_to_moa(rad),
        iphy: units::rad_to_iphy(rad),
        cm_per_100m: units::rad_to_cm_per_100m(rad),
        direction,
        clicks,
        residual_mrad,
    }
}

fn bore_angle(shot: &Shot) -> Result<(f64, Option<f64>, Vec<Warning>)> {
    let mut warnings = Vec::new();
    if let Some(angle) = shot.bore_angle_rad {
        return Ok((angle, None, warnings));
    }
    let zero_range = shot.zero_range_m.ok_or(Error::Invalid(
        "a shot needs a zero range or an explicit bore angle",
    ))?;
    let zero_atmo = match &shot.zero_atmosphere {
        Some(atmo) => *atmo,
        None => {
            warnings.push(Warning::DefaultZeroAtmosphere);
            Atmosphere::icao()
        }
    };
    let air = zero_atmo.state()?;
    let powder = shot.zero_powder_temperature_c.or(shot.powder_temperature_c);
    let mv = adjusted_mv_with(shot, zero_atmo.temperature_c, powder);
    let drag = prepare(shot)?;
    let objective = |theta: f64| -> Result<f64> {
        let flight = fly(shot, theta, mv, &air, &drag, &[zero_range])?;
        let frame = frame_for(shot, theta);
        let miss = traj::miss(&frame, flight.samples[0].position);
        Ok(miss.scope_above_m - shot.zero_offset_m)
    };
    let mut lo = -0.02;
    let mut hi = 0.08;
    let mut flo = objective(lo)?;
    let mut fhi = objective(hi)?;
    for _ in 0..12 {
        if flo * fhi <= 0.0 {
            break;
        }
        if flo > 0.0 && fhi > 0.0 {
            lo -= 0.05;
            flo = objective(lo)?;
        } else {
            hi += 0.05;
            fhi = objective(hi)?;
        }
    }
    if flo * fhi > 0.0 {
        return Err(Error::ZeroNotBracketed);
    }
    let angle = brent(lo, hi, flo, fhi, objective, 1.0e-5)?;
    let error = objective(angle)?;
    Ok((angle, Some(error), warnings))
}

/// Brent's method. `tol` is on the function value (metres of line-of-sight error).
fn brent<F>(mut a: f64, mut b: f64, mut fa: f64, mut fb: f64, mut f: F, tol: f64) -> Result<f64>
where
    F: FnMut(f64) -> Result<f64>,
{
    if fa * fb > 0.0 {
        return Err(Error::ZeroNotBracketed);
    }
    if fa.abs() < fb.abs() {
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut fa, &mut fb);
    }
    let mut c = a;
    let mut fc = fa;
    let mut d = b - a;
    let mut mflag = true;
    for _ in 0..80 {
        if fb.abs() <= tol || (b - a).abs() < 1.0e-12 {
            return Ok(b);
        }
        let mut s = if fa != fc && fb != fc {
            a * fb * fc / ((fa - fb) * (fa - fc))
                + b * fa * fc / ((fb - fa) * (fb - fc))
                + c * fa * fb / ((fc - fa) * (fc - fb))
        } else {
            b - fb * (b - a) / (fb - fa)
        };
        let outside = (s - (3.0 * a + b) / 4.0) * (s - b) > 0.0;
        let too_far = (mflag && (s - b).abs() >= (b - c).abs() * 0.5)
            || (!mflag && (s - b).abs() >= (c - d).abs() * 0.5);
        let too_close = (mflag && (b - c).abs() < 1.0e-12) || (!mflag && (c - d).abs() < 1.0e-12);
        if outside || too_far || too_close || !s.is_finite() {
            s = 0.5 * (a + b);
            mflag = true;
        } else {
            mflag = false;
        }
        let fs = f(s)?;
        d = c;
        c = b;
        fc = fb;
        if fa * fs < 0.0 {
            b = s;
            fb = fs;
        } else {
            a = s;
            fa = fs;
        }
        if fa.abs() < fb.abs() {
            std::mem::swap(&mut a, &mut b);
            std::mem::swap(&mut fa, &mut fb);
        }
    }
    Err(Error::ZeroNotConverged)
}

fn integrate_shot(
    shot: &Shot,
    bore: f64,
    mv: f64,
    air: &AirState,
    ranges: &[f64],
) -> Result<Flight> {
    let drag = prepare(shot)?;
    fly(shot, bore, mv, air, &drag, ranges)
}

fn fly(
    shot: &Shot,
    bore: f64,
    mv: f64,
    air: &AirState,
    drag: &PreparedDrag,
    ranges: &[f64],
) -> Result<Flight> {
    let frame = frame_for(shot, bore);
    let stepper = match shot.integrator {
        Integrator::Dopri { rtol, atol } => Stepper::Dopri { rtol, atol },
        Integrator::Rk4 { dt } => Stepper::Rk4 { dt },
    };
    traj::fly(
        drag,
        air,
        frame,
        mv,
        traj::wind_vector(shot.wind.speed_mps, shot.wind.from_clock_rad),
        shot.gravity_mps2,
        shot.effects.coriolis,
        shot.latitude_rad,
        shot.azimuth_rad,
        ranges.to_vec(),
        stepper,
    )
}

fn frame_for(shot: &Shot, bore: f64) -> Frame {
    let inclination = if shot.effects.incline {
        shot.inclination_rad
    } else {
        0.0
    };
    let cant = if shot.effects.cant {
        shot.cant_rad
    } else {
        0.0
    };
    Frame::new(inclination, bore, cant, shot.sight_height_m)
}

fn prepare(shot: &Shot) -> Result<PreparedDrag> {
    PreparedDrag::prepare(
        &shot.bullet.drag,
        shot.bullet.weight_kg,
        shot.bullet.diameter_m,
    )
}

fn adjusted_mv_with(shot: &Shot, air_c: f64, powder: Option<f64>) -> f64 {
    let Some(sens) = shot.temperature_sensitivity_mps_per_c else {
        return shot.mv_mps;
    };
    let measured = shot.mv_temperature_c.unwrap_or(air_c);
    let powder = powder.unwrap_or(air_c);
    shot.mv_mps + sens * (powder - measured)
}

fn stability_of(shot: &Shot, mv: f64, air: &AirState) -> (Option<f64>, Option<f64>, bool) {
    let Some(twist) = shot.twist else {
        return (None, None, false);
    };
    let Some(length) = shot.bullet.length_m else {
        return (None, None, false);
    };
    if air.station_pressure_pa <= 0.0 {
        return (None, None, false);
    }
    let (sg, corrected) = sg_for_shot(
        shot.bullet.weight_kg,
        shot.bullet.diameter_m,
        length,
        twist.metres_per_turn,
        mv,
        shot.atmosphere.temperature_c,
        air.station_pressure_pa,
    );
    (Some(sg), Some(corrected), true)
}
