//! Point-mass integration in a world frame: x downrange, y up, z right.
//!
//! The default stepper is Dormand–Prince 5(4). Fixed-step RK4 is kept for the
//! convergence check. Samples land on a requested slant range by cubic Hermite
//! interpolation inside the crossing step.

use crate::atmosphere::AirState;
use crate::constants::EARTH_OMEGA;
use crate::drag::PreparedDrag;
use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub u: [f64; 3],
    pub n_vert: [f64; 3],
    pub r: [f64; 3],
    pub n_scope: [f64; 3],
    pub r_scope: [f64; 3],
    pub launch: [f64; 3],
    pub bore_up: [f64; 3],
    pub sight: [f64; 3],
}

impl Frame {
    pub(crate) fn new(
        inclination_rad: f64,
        bore_rad: f64,
        cant_rad: f64,
        sight_height_m: f64,
    ) -> Self {
        let u = [inclination_rad.cos(), inclination_rad.sin(), 0.0];
        let n_vert = [-inclination_rad.sin(), inclination_rad.cos(), 0.0];
        let r = [0.0, 0.0, 1.0];
        let n_scope = add(scale(n_vert, cant_rad.cos()), scale(r, cant_rad.sin()));
        let r_scope = add(scale(n_vert, -cant_rad.sin()), scale(r, cant_rad.cos()));
        let launch = add(scale(u, bore_rad.cos()), scale(n_scope, bore_rad.sin()));
        let bore_up = add(scale(u, -bore_rad.sin()), scale(n_scope, bore_rad.cos()));
        let sight = scale(n_scope, sight_height_m);
        Self {
            u,
            n_vert,
            r,
            n_scope,
            r_scope,
            launch,
            bore_up,
            sight,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Miss {
    #[allow(dead_code)]
    pub along_m: f64,
    pub above_m: f64,
    pub right_m: f64,
    pub scope_above_m: f64,
    pub scope_right_m: f64,
    pub drop_m: f64,
}

pub(crate) fn miss(frame: &Frame, pos: [f64; 3]) -> Miss {
    let d = sub(pos, frame.sight);
    Miss {
        along_m: dot(d, frame.u),
        above_m: dot(d, frame.n_vert),
        right_m: dot(d, frame.r),
        scope_above_m: dot(d, frame.n_scope),
        scope_right_m: dot(d, frame.r_scope),
        drop_m: -dot(pos, frame.bore_up),
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Stepper {
    Dopri { rtol: f64, atol: f64 },
    Rk4 { dt: f64 },
}

#[derive(Clone, Debug)]
pub(crate) struct Kinematic {
    pub time_s: f64,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub mach: f64,
    pub range_m: f64,
}

pub(crate) struct Flight {
    pub samples: Vec<Kinematic>,
    pub transonic_range_m: Option<f64>,
    pub drag_clamped: bool,
}

struct Ctx<'a> {
    drag: &'a PreparedDrag,
    air: &'a AirState,
    wind: [f64; 3],
    gravity: f64,
    coriolis: bool,
    latitude_rad: f64,
    azimuth_rad: f64,
    frame: Frame,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn fly(
    drag: &PreparedDrag,
    air: &AirState,
    frame: Frame,
    muzzle_mps: f64,
    wind: [f64; 3],
    gravity: f64,
    coriolis: bool,
    latitude_rad: f64,
    azimuth_rad: f64,
    mut ranges: Vec<f64>,
    stepper: Stepper,
) -> Result<Flight> {
    if muzzle_mps <= 0.0 {
        return Err(Error::Invalid("muzzle velocity must be positive"));
    }
    ranges.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for range in &ranges {
        if *range < 0.0 {
            return Err(Error::Invalid("range must be zero or positive"));
        }
    }
    let ctx = Ctx {
        drag,
        air,
        wind,
        gravity,
        coriolis,
        latitude_rad,
        azimuth_rad,
        frame,
    };
    let mut y = [
        0.0,
        0.0,
        0.0,
        muzzle_mps * frame.launch[0],
        muzzle_mps * frame.launch[1],
        muzzle_mps * frame.launch[2],
    ];
    let mut deriv = acceleration_state(&ctx, &y);
    let mut t = 0.0;
    let mut samples = Vec::new();
    let mut drag_clamped = false;
    let mut transonic_range_m = None;
    let mut mach_prev = mach_of(&ctx, &y);

    while ranges.first().copied().unwrap_or(1.0) <= 1.0e-9 {
        ranges.remove(0);
        let (sample, clamped) = sample_at(&ctx, 0.0, &y, ranges_placeholder(0.0));
        drag_clamped |= clamped;
        samples.push(sample);
    }

    let mut h = match stepper {
        Stepper::Dopri { .. } => 0.001,
        Stepper::Rk4 { dt } => dt,
    };
    let mut steps = 0usize;
    while !ranges.is_empty() {
        steps += 1;
        if steps > 200_000 {
            return Err(Error::RangeNotReached {
                requested_m: ranges[0],
                reached_m: along(&ctx.frame, &y),
            });
        }
        if t > 90.0 || speed(&y) < 20.0 {
            return Err(Error::RangeNotReached {
                requested_m: ranges[0],
                reached_m: along(&ctx.frame, &y),
            });
        }
        let (y1, deriv1, h_used, h_next, accepted) = match stepper {
            Stepper::Rk4 { dt } => {
                let (y1, deriv1) = rk4(&ctx, &y, dt);
                (y1, deriv1, dt, dt, true)
            }
            Stepper::Dopri { rtol, atol } => dopri_adaptive(&ctx, &y, &deriv, h, rtol, atol),
        };
        if !accepted {
            h = h_next.max(1.0e-8);
            continue;
        }
        let mach1 = mach_of(&ctx, &y1);
        if transonic_range_m.is_none() && (mach_prev - 1.0) * (mach1 - 1.0) < 0.0 {
            let s = find_fraction(&y, &deriv, &y1, &deriv1, h_used, |state| {
                mach_of(&ctx, state) - 1.0
            });
            let crossed = hermite_state(&y, &deriv, &y1, &deriv1, h_used, s);
            transonic_range_m = Some(along(&ctx.frame, &crossed));
        }
        let a0 = along(&ctx.frame, &y);
        let a1 = along(&ctx.frame, &y1);
        while let Some(&range) = ranges.first() {
            if a1 + 1.0e-8 < range {
                break;
            }
            ranges.remove(0);
            let s = find_fraction(&y, &deriv, &y1, &deriv1, h_used, |state| {
                along(&ctx.frame, state) - range
            });
            let state = hermite_state(&y, &deriv, &y1, &deriv1, h_used, s);
            let time = t + s * h_used;
            let (sample, clamped) = sample_at(&ctx, time, &state, range);
            drag_clamped |= clamped;
            // Keep the caller's range even if the interpolant is a fraction of a millimetre off.
            let mut sample = sample;
            sample.range_m = range;
            let _ = a0;
            samples.push(sample);
        }
        y = y1;
        deriv = deriv1;
        t += h_used;
        h = h_next;
        mach_prev = mach1;
    }
    Ok(Flight {
        samples,
        transonic_range_m,
        drag_clamped,
    })
}

fn ranges_placeholder(range: f64) -> f64 {
    range
}

fn sample_at(ctx: &Ctx<'_>, time: f64, y: &[f64; 6], range: f64) -> (Kinematic, bool) {
    let (_, clamped) = ctx
        .drag
        .k(mach_of(ctx, y), airspeed(ctx, y), ctx.air.density_kg_m3);
    (
        Kinematic {
            time_s: time,
            position: [y[0], y[1], y[2]],
            velocity: [y[3], y[4], y[5]],
            mach: mach_of(ctx, y),
            range_m: range,
        },
        clamped,
    )
}

fn dopri_adaptive(
    ctx: &Ctx<'_>,
    y: &[f64; 6],
    k1: &[f64; 6],
    h: f64,
    rtol: f64,
    atol: f64,
) -> ([f64; 6], [f64; 6], f64, f64, bool) {
    let h = h.clamp(1.0e-8, 0.05);
    let (y5, k7, err) = dopri_step(ctx, y, k1, h);
    let mut err_norm = 0.0;
    for i in 0..6 {
        let scale = atol + rtol * y5[i].abs().max(y[i].abs());
        let e = err[i] / scale;
        err_norm += e * e;
    }
    err_norm = (err_norm / 6.0).sqrt();
    if err_norm <= 1.0 {
        let factor = if err_norm < 1.0e-12 {
            5.0
        } else {
            (0.9 * err_norm.powf(-0.2)).clamp(0.2, 5.0)
        };
        (y5, k7, h, (h * factor).clamp(1.0e-8, 0.05), true)
    } else {
        let factor = (0.9 * err_norm.powf(-0.25)).clamp(0.2, 0.9);
        (y5, k7, h, (h * factor).max(1.0e-8), false)
    }
}

fn dopri_step(
    ctx: &Ctx<'_>,
    y: &[f64; 6],
    k1: &[f64; 6],
    h: f64,
) -> ([f64; 6], [f64; 6], [f64; 6]) {
    const A21: f64 = 1.0 / 5.0;
    const A31: f64 = 3.0 / 40.0;
    const A32: f64 = 9.0 / 40.0;
    const A41: f64 = 44.0 / 45.0;
    const A42: f64 = -56.0 / 15.0;
    const A43: f64 = 32.0 / 9.0;
    const A51: f64 = 19372.0 / 6561.0;
    const A52: f64 = -25360.0 / 2187.0;
    const A53: f64 = 64448.0 / 6561.0;
    const A54: f64 = -212.0 / 729.0;
    const A61: f64 = 9017.0 / 3168.0;
    const A62: f64 = -355.0 / 33.0;
    const A63: f64 = 46732.0 / 5247.0;
    const A64: f64 = 49.0 / 176.0;
    const A65: f64 = -5103.0 / 18656.0;
    const B1: f64 = 35.0 / 384.0;
    const B3: f64 = 500.0 / 1113.0;
    const B4: f64 = 125.0 / 192.0;
    const B5: f64 = -2187.0 / 6784.0;
    const B6: f64 = 11.0 / 84.0;
    const E1: f64 = 5179.0 / 57600.0 - 35.0 / 384.0;
    const E3: f64 = 7571.0 / 16695.0 - 500.0 / 1113.0;
    const E4: f64 = 393.0 / 640.0 - 125.0 / 192.0;
    const E5: f64 = -92097.0 / 339200.0 - (-2187.0 / 6784.0);
    const E6: f64 = 187.0 / 2100.0 - 11.0 / 84.0;
    const E7: f64 = 1.0 / 40.0;

    let y2 = axpy(y, h, &[(A21, k1)]);
    let k2 = acceleration_state(ctx, &y2);
    let y3 = axpy(y, h, &[(A31, k1), (A32, &k2)]);
    let k3 = acceleration_state(ctx, &y3);
    let y4 = axpy(y, h, &[(A41, k1), (A42, &k2), (A43, &k3)]);
    let k4 = acceleration_state(ctx, &y4);
    let y5_stage = axpy(y, h, &[(A51, k1), (A52, &k2), (A53, &k3), (A54, &k4)]);
    let k5 = acceleration_state(ctx, &y5_stage);
    let y6 = axpy(
        y,
        h,
        &[(A61, k1), (A62, &k2), (A63, &k3), (A64, &k4), (A65, &k5)],
    );
    let k6 = acceleration_state(ctx, &y6);
    let y_next = axpy(
        y,
        h,
        &[(B1, k1), (B3, &k3), (B4, &k4), (B5, &k5), (B6, &k6)],
    );
    let k7 = acceleration_state(ctx, &y_next);
    let err = axpy(
        &[0.0; 6],
        h,
        &[
            (E1, k1),
            (E3, &k3),
            (E4, &k4),
            (E5, &k5),
            (E6, &k6),
            (E7, &k7),
        ],
    );
    (y_next, k7, err)
}

fn rk4(ctx: &Ctx<'_>, y: &[f64; 6], dt: f64) -> ([f64; 6], [f64; 6]) {
    let k1 = acceleration_state(ctx, y);
    let y2 = axpy(y, dt * 0.5, &[(1.0, &k1)]);
    let k2 = acceleration_state(ctx, &y2);
    let y3 = axpy(y, dt * 0.5, &[(1.0, &k2)]);
    let k3 = acceleration_state(ctx, &y3);
    let y4 = axpy(y, dt, &[(1.0, &k3)]);
    let k4 = acceleration_state(ctx, &y4);
    let mut next = *y;
    for i in 0..6 {
        next[i] += dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
    }
    let deriv = acceleration_state(ctx, &next);
    (next, deriv)
}

fn find_fraction(
    y0: &[f64; 6],
    f0: &[f64; 6],
    y1: &[f64; 6],
    f1: &[f64; 6],
    h: f64,
    mut metric: impl FnMut(&[f64; 6]) -> f64,
) -> f64 {
    let mut lo = 0.0;
    let mut hi = 1.0;
    let flo = metric(y0);
    if flo.abs() < 1e-12 {
        return 0.0;
    }
    for _ in 0..50 {
        let mid = 0.5 * (lo + hi);
        let state = hermite_state(y0, f0, y1, f1, h, mid);
        if metric(&state).signum() == flo.signum() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn hermite_state(
    y0: &[f64; 6],
    f0: &[f64; 6],
    y1: &[f64; 6],
    f1: &[f64; 6],
    h: f64,
    s: f64,
) -> [f64; 6] {
    let s2 = s * s;
    let s3 = s2 * s;
    let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
    let h10 = s3 - 2.0 * s2 + s;
    let h01 = -2.0 * s3 + 3.0 * s2;
    let h11 = s3 - s2;
    let mut out = [0.0; 6];
    for i in 0..6 {
        out[i] = h00 * y0[i] + h10 * h * f0[i] + h01 * y1[i] + h11 * h * f1[i];
    }
    out
}

fn acceleration_state(ctx: &Ctx<'_>, y: &[f64; 6]) -> [f64; 6] {
    let a = acceleration(ctx, y);
    [y[3], y[4], y[5], a[0], a[1], a[2]]
}

fn acceleration(ctx: &Ctx<'_>, y: &[f64; 6]) -> [f64; 3] {
    let v = [y[3], y[4], y[5]];
    let rel = sub(v, ctx.wind);
    let airspeed = norm(rel);
    let sound = ctx.air.speed_of_sound_mps.max(1.0);
    let mach = airspeed / sound;
    let (k, _) = ctx.drag.k(mach, airspeed, ctx.air.density_kg_m3);
    let mut a = [
        -k * airspeed * rel[0],
        -k * airspeed * rel[1] - ctx.gravity,
        -k * airspeed * rel[2],
    ];
    if ctx.coriolis {
        let c = coriolis(v, ctx.latitude_rad, ctx.azimuth_rad);
        a[0] += c[0];
        a[1] += c[1];
        a[2] += c[2];
    }
    a
}

fn coriolis(v: [f64; 3], latitude_rad: f64, azimuth_rad: f64) -> [f64; 3] {
    let ox = EARTH_OMEGA * latitude_rad.cos() * azimuth_rad.cos();
    let oy = EARTH_OMEGA * latitude_rad.sin();
    let oz = -EARTH_OMEGA * latitude_rad.cos() * azimuth_rad.sin();
    let cx = oy * v[2] - oz * v[1];
    let cy = oz * v[0] - ox * v[2];
    let cz = ox * v[1] - oy * v[0];
    [-2.0 * cx, -2.0 * cy, -2.0 * cz]
}

fn mach_of(ctx: &Ctx<'_>, y: &[f64; 6]) -> f64 {
    airspeed(ctx, y) / ctx.air.speed_of_sound_mps.max(1.0)
}

fn airspeed(ctx: &Ctx<'_>, y: &[f64; 6]) -> f64 {
    let rel = [y[3] - ctx.wind[0], y[4] - ctx.wind[1], y[5] - ctx.wind[2]];
    norm(rel)
}

fn speed(y: &[f64; 6]) -> f64 {
    norm([y[3], y[4], y[5]])
}

fn along(frame: &Frame, y: &[f64; 6]) -> f64 {
    let d = sub([y[0], y[1], y[2]], frame.sight);
    dot(d, frame.u)
}

fn axpy(y: &[f64; 6], h: f64, terms: &[(f64, &[f64; 6])]) -> [f64; 6] {
    let mut out = *y;
    for (coeff, k) in terms {
        for i in 0..6 {
            out[i] += h * coeff * k[i];
        }
    }
    out
}

pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

pub(crate) fn wind_vector(speed_mps: f64, from_clock_rad: f64) -> [f64; 3] {
    [
        -speed_mps * from_clock_rad.cos(),
        0.0,
        -speed_mps * from_clock_rad.sin(),
    ]
}
