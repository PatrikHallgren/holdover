//! Milestone 3 checks: point-blank range, lead, truing recovery, recoil, and chrono stats.

use holdover_core::{
    card_page, chrono_stats, lead, pbr, powder_coefficient, recoil, running_presets, scale_bar_mm,
    solve, table, true_drag, true_mv, true_tracking, vertical_mv_spread_m, Atmosphere, BcBand,
    BcReference, Bullet, DragMode, DragModel, DragSpec, Effects, RecoilInput, Shot, TableSpec,
    Wind,
};

fn bullet(bc: f64) -> Bullet {
    Bullet {
        weight_kg: 0.011,
        diameter_m: 0.00782,
        length_m: Some(0.032),
        drag: DragSpec::Standard {
            model: DragModel::G7,
            bands: vec![BcBand {
                bc_lb_per_in2: bc,
                min_mps: 0.0,
                reference: BcReference::Icao,
            }],
        },
    }
}

fn zeroed(mv: f64, bc: f64) -> Shot {
    let mut shot = Shot::basic(mv, bullet(bc), Atmosphere::icao());
    shot.sight_height_m = 0.045;
    shot.zero_range_m = Some(100.0);
    shot.zero_atmosphere = Some(Atmosphere::icao());
    shot.effects = Effects {
        spin_drift: false,
        ..Effects::default()
    };
    shot
}

fn first_exit(shot: &Shot, bore: f64, radius: f64) -> Option<f64> {
    let mut trial = shot.clone();
    trial.bore_angle_rad = Some(bore);
    trial.wind = Wind::default();
    let solved = table(
        &trial,
        TableSpec {
            start_m: 0.0,
            end_m: 800.0,
            step_m: 5.0,
        },
    )
    .ok()?;
    let samples: Vec<(f64, f64)> = solved
        .points
        .iter()
        .map(|point| (point.range_m, point.path_m))
        .collect();
    if samples.len() < 2 {
        return None;
    }
    let mut best = f64::NEG_INFINITY;
    for (_, path) in &samples {
        best = best.max(*path);
    }
    // A sampled apex within 1 mm of the radius still counts as touching the zone.
    if best <= radius + 0.001 {
        return descend(&samples, -radius);
    }
    ascend(&samples, radius)
}

fn descend(samples: &[(f64, f64)], level: f64) -> Option<f64> {
    for window in samples.windows(2) {
        let (x0, y0) = window[0];
        let (x1, y1) = window[1];
        if y0 >= level && y1 < level {
            let t = (level - y0) / (y1 - y0);
            return Some(x0 + t * (x1 - x0));
        }
    }
    None
}

fn ascend(samples: &[(f64, f64)], level: f64) -> Option<f64> {
    for window in samples.windows(2) {
        let (x0, y0) = window[0];
        let (x1, y1) = window[1];
        if y0 <= level && y1 > level {
            let t = (level - y0) / (y1 - y0);
            return Some(x0 + t * (x1 - x0));
        }
    }
    None
}

#[test]
fn point_blank_range_matches_a_bore_search() {
    let shot = zeroed(800.0, 0.22);
    let zone = 0.10;
    let solved = pbr(&shot, zone).unwrap();
    let radius = zone * 0.5;
    assert!(
        (solved.ordinate_m - radius).abs() < 0.001,
        "{}",
        solved.ordinate_m
    );
    assert!(solved.near_m < solved.optimal_zero_m);
    assert!(solved.optimal_zero_m < solved.far_m);
    assert!(solved.far_m > solved.ordinate_range_m);

    let mut best: f64 = 0.0;
    let mut best_bore = 0.0;
    let mut bore = 0.0002;
    while bore < 0.012 {
        if let Some(exit) = first_exit(&shot, bore, radius) {
            if exit > best {
                best = exit;
                best_bore = bore;
            }
        }
        bore += 0.0002;
    }
    let mut fine = best_bore - 0.00025;
    while fine < best_bore + 0.00025 {
        if let Some(exit) = first_exit(&shot, fine, radius) {
            best = best.max(exit);
        }
        fine += 0.00002;
    }
    let direct = first_exit(&shot, solved.bore_angle_rad, radius).unwrap();
    assert!(
        (direct - solved.far_m).abs() < 1.0,
        "same bore: {direct} vs {}",
        solved.far_m
    );
    assert!(
        best <= solved.far_m + 2.0,
        "a searched bore stayed in the zone past the point-blank range: {best} vs {}",
        solved.far_m
    );
    assert!(
        (solved.far_m - best).abs() < 2.0,
        "pbr {} vs brute {best}",
        solved.far_m
    );
}

#[test]
fn truing_recovers_a_synthetic_muzzle_velocity_and_bc() {
    let truth = zeroed(820.0, 0.25);
    let observed = solve(&truth, 400.0).unwrap().points[0].elevation.rad;
    let guess = zeroed(760.0, 0.25);
    let fitted = true_mv(&guess, 400.0, observed).unwrap();
    assert!(
        (fitted.mv_mps - 820.0).abs() / 820.0 < 0.01,
        "mv {}",
        fitted.mv_mps
    );
    assert!(fitted.residual_rad.abs() < 1.0e-5);
    assert!(fitted.before_transonic, "mach {}", fitted.mach);

    let truth = zeroed(800.0, 0.30);
    let observed = solve(&truth, 500.0).unwrap().points[0].elevation.rad;
    let guess = zeroed(800.0, 0.24);
    let fitted = true_drag(&guess, &[(500.0, observed)], DragMode::Multiplier).unwrap();
    let effective = 0.24 * fitted.bc_multiplier;
    assert!(
        (effective - 0.30).abs() / 0.30 < 0.01,
        "bc {effective} from multiplier {}",
        fitted.bc_multiplier
    );
    assert_eq!(fitted.pieces.len(), 1);
    assert!((fitted.pieces[0].multiplier - 1.0 / fitted.bc_multiplier).abs() < 1.0e-9);

    let factor = true_tracking(0.010, 0.0104).unwrap();
    assert!((factor - 1.04).abs() < 1.0e-12);
}

#[test]
fn lead_is_speed_times_time_of_flight_and_presets_match_the_plan() {
    let shot = zeroed(800.0, 0.25);
    let crossing = std::f64::consts::FRAC_PI_2;
    let solved = lead(&shot, 80.0, 5.3, crossing, 0.0).unwrap();
    let tof = solve(&shot, solved.impact_range_m).unwrap().points[0].tof_s;
    assert!((solved.lead_m - 5.3 * tof).abs() < 1.0e-3);
    assert!((solved.impact_range_m - 80.0).abs() < 0.05);
    assert!(solved.lead.mrad > 0.0);
    assert_eq!(solved.lead.direction, holdover_core::Direction::Right);

    let away = lead(&shot, 80.0, 5.3, 0.0, 0.0).unwrap();
    assert!(away.impact_range_m > 80.5, "{}", away.impact_range_m);
    assert!(away.lead_m.abs() < 1.0e-6);

    let allowance = lead(&shot, 80.0, 5.3, crossing, 0.4).unwrap();
    assert!((allowance.reaction_lead_m - 5.3 * 0.4).abs() < 1.0e-9);
    assert!(allowance.with_reaction.mrad > allowance.lead.mrad);

    let moose = running_presets()
        .iter()
        .find(|preset| preset.id == "algbana")
        .unwrap();
    assert_eq!(moose.range_m, 80.0);
    assert_eq!(moose.speed_mps, 5.3);
    let roe = running_presets()
        .iter()
        .find(|preset| preset.id == "viltmal")
        .unwrap();
    assert_eq!(roe.speed_mps, 2.0);
    assert_eq!(roe.range_m, 50.0);
    let fast = running_presets()
        .iter()
        .find(|preset| preset.id == "boar-fast")
        .unwrap();
    assert!(!fast.speed_known);
    assert_eq!(fast.range_m, 30.0);
}

#[test]
fn recoil_matches_the_grain_formula_and_chrono_keeps_outliers() {
    let grain = 0.00006479891;
    let result = recoil(RecoilInput {
        rifle_mass_kg: 8.0 * 0.45359237,
        bullet_mass_kg: 150.0 * grain,
        charge_mass_kg: 45.0 * grain,
        mv_mps: 2800.0 * 0.3048,
        ejecta_mps: 4000.0 * 0.3048,
        reduction_fraction: 0.0,
    })
    .unwrap();
    assert!((result.velocity_fps - (600_000.0 / 56_000.0)).abs() < 1.0e-3);
    assert!(!result.reduction_is_estimate);
    assert!(result.convention.contains("not been confirmed"));
    let reduced = recoil(RecoilInput {
        reduction_fraction: 0.20,
        ..result_input()
    })
    .unwrap();
    assert!(reduced.reduction_is_estimate);
    assert!((reduced.energy_j - result.energy_j * 0.8).abs() < 1.0e-6);

    let shots = [800.0, 800.0, 800.0, 800.0, 800.0, 800.0, 800.0, 900.0];
    let stats = chrono_stats(&shots).unwrap();
    assert!((stats.mean_mps - 812.5).abs() < 1.0e-9);
    assert_eq!(stats.outliers, vec![7]);
    assert!((stats.es_mps - 100.0).abs() < 1.0e-9);
    let fit = powder_coefficient(&[(0.0, 800.0), (10.0, 810.0), (20.0, 820.0)]).unwrap();
    assert!((fit.slope_mps_per_c - 1.0).abs() < 1.0e-9);
    assert!((fit.r_squared - 1.0).abs() < 1.0e-9);
    assert_eq!(fit.n, 3);

    let (low, high) = vertical_mv_spread_m(&zeroed(800.0, 0.25), 300.0, 5.0).unwrap();
    assert!(high > low);
}

fn result_input() -> RecoilInput {
    let grain = 0.00006479891;
    RecoilInput {
        rifle_mass_kg: 8.0 * 0.45359237,
        bullet_mass_kg: 150.0 * grain,
        charge_mass_kg: 45.0 * grain,
        mv_mps: 2800.0 * 0.3048,
        ejecta_mps: 4000.0 * 0.3048,
        reduction_fraction: 0.0,
    }
}

#[test]
fn printed_card_bar_is_the_labelled_length() {
    assert_eq!(scale_bar_mm(card_page("a4").unwrap()), 100.0);
    assert_eq!(scale_bar_mm(card_page("a5").unwrap()), 100.0);
    assert_eq!(scale_bar_mm(card_page("a6").unwrap()), 50.0);
    assert_eq!(scale_bar_mm(card_page("letter").unwrap()), 100.0);
    assert_eq!(scale_bar_mm(card_page("compact").unwrap()), 50.0);
    assert_eq!(card_page("a4").unwrap().width_mm, 210.0);
    assert_eq!(card_page("letter").unwrap().width_mm, 215.9);
}
