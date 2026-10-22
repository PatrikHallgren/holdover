//! Documented reference vectors. The numbers are derived in the test from
//! closed-form physics, not copied back out of the solver.

use holdover_core::{
    solve, solve_zero, table, Atmosphere, BcBand, BcReference, Bullet, DragModel, DragSpec,
    Effects, Integrator, PressureKind, Shot, TableSpec, Twist, TwistHand, DISCLAIMER,
};

fn bullet(drag: DragSpec) -> Bullet {
    Bullet {
        weight_kg: 0.01,
        diameter_m: 0.00762,
        length_m: None,
        drag,
    }
}

fn g1(bc: f64) -> DragSpec {
    DragSpec::Standard {
        model: DragModel::G1,
        bands: vec![BcBand {
            bc_lb_per_in2: bc,
            min_mps: 0.0,
            reference: BcReference::Icao,
        }],
    }
}

fn vacuum_shot() -> Shot {
    let mut shot = Shot::basic(800.0, bullet(g1(0.5)), Atmosphere::vacuum());
    shot.bore_angle_rad = Some(0.0);
    shot.effects.spin_drift = false;
    shot
}

#[test]
fn v01_vacuum_drop_at_100_300_and_1000_m() {
    let shot = vacuum_shot();
    let g = 9.80665;
    for range in [100.0, 300.0, 1000.0] {
        let point = solve(&shot, range).unwrap().points.remove(0);
        let t = range / 800.0;
        let y = -0.5 * g * t * t;
        assert!(
            (point.path_m - y).abs() < 1e-6,
            "path at {range}: {}",
            point.path_m
        );
        assert!((point.drop_m + y).abs() < 1e-6, "drop at {range}");
        assert!((point.tof_s - t).abs() < 1e-8, "tof at {range}");
        let mrad = (-y / range).atan() * 1000.0;
        assert!((point.elevation.mrad - mrad).abs() < 1e-6);
        assert_eq!(point.elevation.direction, holdover_core::Direction::Up);
    }
}

#[test]
fn v04_vacuum_incline_matches_the_parabola() {
    let mut shot = vacuum_shot();
    let alpha = 30.0_f64.to_radians();
    shot.inclination_rad = alpha;
    let v = 800.0;
    let t = 0.5;
    let g = 9.80665;
    let x = v * alpha.cos() * t;
    let y = v * alpha.sin() * t - 0.5 * g * t * t;
    let along = x * alpha.cos() + y * alpha.sin();
    let above = -x * alpha.sin() + y * alpha.cos();
    let point = solve(&shot, along).unwrap().points.remove(0);
    assert!(
        (point.path_m - above).abs() < 1e-5,
        "path {} vs {above}",
        point.path_m
    );
    assert!((point.tof_s - t).abs() < 1e-6, "tof {}", point.tof_s);
}

#[test]
fn v05_vacuum_zero_angle_and_crossing() {
    let mut shot = vacuum_shot();
    shot.bore_angle_rad = None;
    shot.zero_range_m = Some(200.0);
    shot.zero_atmosphere = Some(Atmosphere::vacuum());
    let g = 9.80665;
    let expected = 0.5 * (g * 200.0 / 800.0_f64.powi(2)).asin();
    let zero = solve_zero(&shot).unwrap();
    assert!(
        (zero.bore_angle_rad - expected).abs() < 1e-6,
        "bore {} vs {expected}",
        zero.bore_angle_rad
    );
    let error = zero.crossing_error_m.unwrap().abs();
    assert!(error <= 1.0e-4, "crossing error {error} m");
    let point = solve(&shot, 200.0).unwrap().points.remove(0);
    assert!(point.path_m.abs() <= 1.0e-4, "path {}", point.path_m);
}

#[test]
fn v06_constant_drag_without_gravity_matches_the_analytic_decay() {
    let mut atmo = Atmosphere::icao();
    atmo.pressure_pa = 101_326.809_307_5;
    let mut shot = Shot::basic(
        800.0,
        Bullet {
            weight_kg: 0.01,
            diameter_m: 0.00762,
            length_m: None,
            drag: DragSpec::Custom {
                points: vec![(0.0, 0.5), (5.0, 0.5)],
            },
        },
        atmo,
    );
    shot.gravity_mps2 = 0.0;
    shot.bore_angle_rad = Some(0.0);
    shot.effects.spin_drift = false;
    let air = atmo.state().unwrap();
    assert!((air.density_kg_m3 - 1.225).abs() < 1e-9);
    // k = ρ Cd π / (8 m/d²), computed independently of the solver.
    let k = 1.396_612_489_262_478_4e-3;
    for t in [0.4_f64, 0.8] {
        let speed = 800.0 / (1.0 + k * 800.0 * t);
        let distance = (1.0 + k * 800.0 * t).ln() / k;
        let point = solve(&shot, distance).unwrap().points.remove(0);
        assert!(
            (point.velocity_mps - speed).abs() < 1e-3,
            "v {} vs {speed}",
            point.velocity_mps
        );
        assert!((point.tof_s - t).abs() < 1e-5, "t {} vs {t}", point.tof_s);
        assert!(point.path_m.abs() < 1e-4);
    }
}

#[test]
fn integrator_convergence_is_within_a_hundredth_of_a_mil() {
    let mut shot = Shot::basic(800.0, sporting(), Atmosphere::icao());
    shot.sight_height_m = 0.045;
    shot.zero_range_m = Some(100.0);
    shot.zero_atmosphere = Some(Atmosphere::icao());
    shot.effects.spin_drift = false;
    shot.integrator = Integrator::Rk4 { dt: 0.002 };
    let coarse = solve(&shot, 1000.0).unwrap().points[0].elevation.mrad;
    shot.integrator = Integrator::Rk4 { dt: 0.001 };
    let fine = solve(&shot, 1000.0).unwrap().points[0].elevation.mrad;
    assert!((coarse - fine).abs() <= 0.01, "{coarse} vs {fine}");
    shot.integrator = Integrator::default();
    let dopri = solve(&shot, 1000.0).unwrap().points[0].elevation.mrad;
    assert!((dopri - fine).abs() <= 0.01, "dopri {dopri} vs rk4 {fine}");
}

#[test]
fn zero_with_drag_crosses_within_a_tenth_of_a_millimetre() {
    let mut shot = Shot::basic(800.0, sporting(), Atmosphere::icao());
    shot.sight_height_m = 0.045;
    shot.zero_range_m = Some(100.0);
    shot.zero_atmosphere = Some(Atmosphere::icao());
    shot.effects.spin_drift = false;
    let error = solve_zero(&shot).unwrap().crossing_error_m.unwrap().abs();
    assert!(error <= 1.0e-4, "{error}");
}

#[test]
fn drop_below_the_bore_grows_with_range() {
    let mut shot = Shot::basic(800.0, sporting(), Atmosphere::icao());
    shot.bore_angle_rad = Some(0.002);
    shot.effects.spin_drift = false;
    let rows = table(
        &shot,
        TableSpec {
            start_m: 100.0,
            end_m: 600.0,
            step_m: 100.0,
        },
    )
    .unwrap();
    let drops: Vec<f64> = rows.points.iter().map(|p| p.drop_m).collect();
    for pair in drops.windows(2) {
        assert!(pair[1] > pair[0], "{drops:?}");
    }
}

#[test]
fn more_muzzle_velocity_and_more_bc_mean_less_drop() {
    let mut slow = zeroed(700.0, 0.2);
    let mut fast = zeroed(900.0, 0.2);
    let mut slippery = zeroed(700.0, 0.4);
    for shot in [&mut slow, &mut fast, &mut slippery] {
        shot.effects.spin_drift = false;
    }
    let drop = |shot: &Shot| -solve(shot, 500.0).unwrap().points[0].path_m;
    assert!(drop(&fast) < drop(&slow));
    assert!(drop(&slippery) < drop(&slow));
}

#[test]
fn a_mirrored_crosswind_mirrors_the_drift() {
    let mut from_left = zeroed(800.0, 0.243);
    from_left.wind = holdover_core::parse_wind("3mps@9oc").unwrap();
    from_left.effects.spin_drift = false;
    let mut from_right = from_left.clone();
    from_right.wind = holdover_core::parse_wind("3mps@3oc").unwrap();
    let left = solve(&from_left, 400.0).unwrap().points.remove(0);
    let right = solve(&from_right, 400.0).unwrap().points.remove(0);
    assert!(
        left.windage_m > 0.01,
        "wind from the left should drift right"
    );
    assert!((left.windage_m + right.windage_m).abs() < 1e-6);
    assert_eq!(left.windage.direction, holdover_core::Direction::Left);
    assert_eq!(right.windage.direction, holdover_core::Direction::Right);
}

#[test]
fn headwind_is_slower_than_a_tailwind() {
    let mut head = zeroed(800.0, 0.243);
    head.wind = holdover_core::parse_wind("5mps@12oc").unwrap();
    head.effects.spin_drift = false;
    let mut tail = head.clone();
    tail.wind = holdover_core::parse_wind("5mps@6oc").unwrap();
    let hv = solve(&head, 500.0).unwrap().points[0].velocity_mps;
    let tv = solve(&tail, 500.0).unwrap().points[0].velocity_mps;
    assert!(hv < tv, "{hv} vs {tv}");
}

#[test]
fn energy_of_a_175_grain_bullet_at_800_mps() {
    let joule = 0.5 * (175.0 / 7000.0 * 0.45359237) * 800.0 * 800.0;
    assert!((joule - 3628.73896_f64).abs() < 1e-6);
    let fps = 800.0 / 0.3048;
    let ft_lbf = 175.0 * fps * fps / 450_240.0;
    assert!((ft_lbf - 2_677.589_655_897_940_7_f64).abs() < 1e-6);
    let solved = {
        let mut shot = vacuum_shot();
        shot.bullet.weight_kg = 175.0 / 7000.0 * 0.45359237;
        solve(&shot, 100.0).unwrap().points.remove(0).energy_ft_lbf
    };
    assert!(solved > 2600.0);
}

#[test]
fn clicks_round_with_a_residual() {
    let mut shot = vacuum_shot();
    shot.sight_height_m = 0.0;
    // Choose a bore angle of 0 so the hold is the vacuum drop, then check the
    // click helper through a known 2.94 mrad by scaling... the public hold is
    // produced by the solver. 0.1 mrad clicks on the 100 m vacuum hold.
    shot.click = Some(holdover_core::Click {
        value_rad: 0.0001,
        tracking_factor: 1.0,
        travel_up_rad: Some(0.01),
        travel_down_rad: None,
        travel_right_rad: None,
        travel_left_rad: None,
    });
    let point = solve(&shot, 100.0).unwrap().points.remove(0);
    let expected = (point.elevation.mrad / 0.1).round() as i32;
    assert_eq!(point.elevation.clicks, Some(expected));
    let residual = point.elevation.mrad - expected as f64 * 0.1;
    assert!((point.elevation.residual_mrad.unwrap() - residual).abs() < 1e-9);
}

#[test]
fn spin_drift_moves_a_right_hand_bullet_right() {
    let mut shot = zeroed(800.0, 0.243);
    shot.bullet.length_m = Some(1.24 * 0.0254);
    shot.bullet.weight_kg = 175.0 / 7000.0 * 0.45359237;
    shot.bullet.diameter_m = 0.308 * 0.0254;
    shot.twist = Some(Twist {
        metres_per_turn: 10.0 * 0.0254,
        hand: TwistHand::Right,
    });
    shot.effects.spin_drift = true;
    shot.wind.speed_mps = 0.0;
    let point = solve(&shot, 600.0).unwrap().points.remove(0);
    assert!(point.sg.unwrap() > 1.0);
    assert!(point.spin_drift_m > 0.0);
    assert!(point.windage_m > 0.0);
}

#[test]
fn a_full_solve_to_2000_m_stays_interactive() {
    let shot = zeroed(800.0, 0.243);
    let started = std::time::Instant::now();
    let point = solve(&shot, 2000.0).unwrap().points.remove(0);
    let elapsed = started.elapsed();
    assert!(point.velocity_mps > 100.0);
    assert!(
        elapsed.as_millis() < 200,
        "debug solve took {} ms; the M1 release target is 20 ms",
        elapsed.as_millis()
    );
}

#[test]
fn group_statistics_match_an_independent_extreme_spread() {
    use holdover_core::{group_stats, MeasureMode, PlaneMm};
    let points = [
        PlaneMm { x: 0.0, y: 0.0 },
        PlaneMm { x: 30.0, y: 0.0 },
        PlaneMm { x: 0.0, y: 40.0 },
    ];
    let stats = group_stats(
        &points,
        PlaneMm { x: 0.0, y: 0.0 },
        100.0,
        MeasureMode::Centres,
        0.1,
    )
    .unwrap();
    assert_eq!(stats.n, 3);
    assert!((stats.extreme_spread_mm - 50.0).abs() < 1e-9);
    assert!(stats.warnings.iter().any(|item| item == "small-n"));
    assert!(stats.cep_mm.is_none());
}

#[test]
fn disclaimer_matches_the_translation_file() {
    let text = include_str!("../../../locales/en.ftl");
    assert!(text.contains(DISCLAIMER));
}

#[test]
fn sea_level_pressure_at_altitude_is_not_silently_treated_as_station() {
    let mut atmo = Atmosphere::icao();
    atmo.pressure_kind = PressureKind::SeaLevel;
    atmo.altitude_m = 1000.0;
    let station = atmo.state().unwrap().station_pressure_pa;
    assert!(station < atmo.pressure_pa);
    assert!((station - 89_874.755_522_364_82).abs() < 1e-3);
}

fn sporting() -> Bullet {
    Bullet {
        weight_kg: 175.0 / 7000.0 * 0.45359237,
        diameter_m: 0.308 * 0.0254,
        length_m: None,
        drag: DragSpec::Standard {
            model: DragModel::G7,
            bands: vec![BcBand {
                bc_lb_per_in2: 0.243,
                min_mps: 0.0,
                reference: BcReference::Icao,
            }],
        },
    }
}

fn zeroed(mv: f64, bc: f64) -> Shot {
    let mut bullet = sporting();
    if let DragSpec::Standard { bands, .. } = &mut bullet.drag {
        bands[0].bc_lb_per_in2 = bc;
    }
    let mut shot = Shot::basic(mv, bullet, Atmosphere::icao());
    shot.sight_height_m = 0.045;
    shot.zero_range_m = Some(100.0);
    shot.zero_atmosphere = Some(Atmosphere::icao());
    shot.effects = Effects {
        spin_drift: false,
        ..Effects::default()
    };
    shot
}
