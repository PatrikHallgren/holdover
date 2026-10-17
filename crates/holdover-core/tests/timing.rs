//! Release-only gate for the M1 budget: a full solve to 2000 m in under 20 ms.
//!
//! `cargo test --release -p holdover-core --test timing` is the CI step.
//! Debug builds skip the assertion; the debug suite still checks a looser bound.

use std::time::Instant;

use holdover_core::{
    solve, Atmosphere, BcBand, BcReference, Bullet, DragModel, DragSpec, Effects, Shot,
};

fn profile() -> Shot {
    let mut shot = Shot::basic(
        800.0,
        Bullet {
            weight_kg: 175.0 / 7000.0 * 0.45359237,
            diameter_m: 0.308 * 0.0254,
            length_m: Some(1.24 * 0.0254),
            drag: DragSpec::Standard {
                model: DragModel::G7,
                bands: vec![BcBand {
                    bc_lb_per_in2: 0.243,
                    min_mps: 0.0,
                    reference: BcReference::Icao,
                }],
            },
        },
        Atmosphere::icao(),
    );
    shot.sight_height_m = 0.045;
    shot.zero_range_m = Some(100.0);
    shot.zero_atmosphere = Some(Atmosphere::icao());
    shot.effects = Effects {
        spin_drift: true,
        ..Effects::default()
    };
    shot
}

#[test]
fn full_solve_to_2000_m_is_under_20_ms() {
    let shot = profile();
    let warmup = solve(&shot, 2000.0).unwrap().points.remove(0);
    assert!(warmup.range_m > 1999.0);
    let started = Instant::now();
    let point = solve(&shot, 2000.0).unwrap().points.remove(0);
    let elapsed = started.elapsed();
    eprintln!("solve_2000m_ms {:.3}", elapsed.as_secs_f64() * 1000.0);
    assert!(point.velocity_mps > 100.0);
    if cfg!(debug_assertions) {
        assert!(
            elapsed.as_millis() < 200,
            "debug solve took {} ms",
            elapsed.as_millis()
        );
        return;
    }
    assert!(
        elapsed.as_secs_f64() * 1000.0 < 20.0,
        "release solve to 2000 m took {:.2} ms; the M1 budget is 20 ms",
        elapsed.as_secs_f64() * 1000.0
    );
}
