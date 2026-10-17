//! Published maker tables with the assumptions written next to the numbers.
//!
//! Plan §27.2: drop within 0.2 mrad of a maker table, or the difference explained.
//! Velocity within 0.5%.

use holdover_core::{
    parse_quantity, solve, Atmosphere, BcBand, BcReference, Bullet, Dimension, DragModel, DragSpec,
    Effects, Shot,
};

fn g_model(model: DragModel, bc: f64, reference: BcReference) -> DragSpec {
    DragSpec::Standard {
        model,
        bands: vec![BcBand {
            bc_lb_per_in2: bc,
            min_mps: 0.0,
            reference,
        }],
    }
}

fn bullet(weight: &str, diameter: &str, drag: DragSpec) -> Bullet {
    Bullet {
        weight_kg: parse_quantity(weight, Dimension::Mass, None).unwrap(),
        diameter_m: parse_quantity(diameter, Dimension::Length, None).unwrap(),
        length_m: None,
        drag,
    }
}

/// Berger Bullets, "7MM PRC 175 Gr Elite Hunter Rifle Ammunition", product page
/// retrieved 2026-10-17. Published columns: G7 BC 0.336, 2920 fps, 175 gr,
/// drop and velocity from the muzzle to 600 yards. The page says the figures
/// are for a 24 inch barrel at sea level. It does not print sight height or
/// temperature. The shot below uses Berger's calculator defaults, a 1.5 inch
/// sight and 59 °F / 29.92 inHg dry air, which is what makes the 100-yard
/// path equal the published +1.5 inches. Spin drift is off; the drop column
/// is vertical.
fn berger_7prc() -> Shot {
    let mut atmo = Atmosphere::icao();
    atmo.temperature_c = parse_quantity("59 F", Dimension::Temperature, None).unwrap();
    atmo.pressure_pa = parse_quantity("29.92 inhg", Dimension::Pressure, None).unwrap();
    atmo.humidity = 0.0;
    let mut shot = Shot::basic(
        parse_quantity("2920 fps", Dimension::Velocity, None).unwrap(),
        bullet(
            "175 gr",
            "0.284 in",
            g_model(DragModel::G7, 0.336, BcReference::Icao),
        ),
        atmo,
    );
    shot.sight_height_m = parse_quantity("1.5 in", Dimension::Length, None).unwrap();
    shot.zero_range_m = Some(parse_quantity("200 yd", Dimension::Length, None).unwrap());
    shot.zero_atmosphere = Some(atmo);
    shot.effects = Effects {
        spin_drift: false,
        ..Effects::default()
    };
    shot
}

#[test]
fn berger_7prc_175_matches_the_published_drop_and_velocity() {
    // (yards, fps, path inches relative to the line of sight)
    let published = [
        (100.0, 2779.0, 1.5),
        (200.0, 2641.0, 0.0),
        (300.0, 2507.0, -6.48),
        (400.0, 2377.0, -18.5),
        (500.0, 2251.0, -36.66),
        (600.0, 2129.0, -61.68),
    ];
    let shot = berger_7prc();
    for (yards, fps, inches) in published {
        let range_m = parse_quantity(&format!("{yards} yd"), Dimension::Length, None).unwrap();
        let point = solve(&shot, range_m).unwrap().points.remove(0);
        let path_in = point.path_m / 0.0254;
        let delta_mrad = ((path_in - inches) * 0.0254 / range_m).atan() * 1000.0;
        assert!(
            delta_mrad.abs() < 0.02,
            "{yards} yd path {path_in:.2} in vs {inches} in ({delta_mrad:.4} mrad)"
        );
        let got_fps = point.velocity_mps / 0.3048;
        let rel = (got_fps - fps).abs() / fps;
        assert!(
            rel < 0.003,
            "{yards} yd {got_fps:.1} fps vs {fps} (relative {rel})"
        );
    }
}

/// Federal Gold Medal .308 Winchester 175 gr Sierra MatchKing (GM308M2),
/// velocity column as quoted from Federal's table: 2600, 2427, 2262, 2102,
/// 1949, 1803 fps at 0–500 yards. G1 BC 0.505. Atmosphere is Army Standard
/// Metro (59 °F, 29.5275 inHg, 78% RH), the air that reproduces that column.
/// The Federal page was not re-fetched here; the column and the G1 BC are
/// the ones cited with that atmosphere on 2026-10-17.
#[test]
fn federal_gm308m2_velocity_matches_army_standard_metro() {
    let atmo = Atmosphere::army_standard_metro();
    let mut shot = Shot::basic(
        parse_quantity("2600 fps", Dimension::Velocity, None).unwrap(),
        bullet(
            "175 gr",
            "0.308 in",
            g_model(DragModel::G1, 0.505, BcReference::ArmyStandardMetro),
        ),
        atmo,
    );
    shot.bore_angle_rad = Some(0.0);
    shot.effects.spin_drift = false;
    let published = [
        (100.0, 2427.0),
        (200.0, 2262.0),
        (300.0, 2102.0),
        (400.0, 1949.0),
        (500.0, 1803.0),
    ];
    for (yards, fps) in published {
        let range_m = parse_quantity(&format!("{yards} yd"), Dimension::Length, None).unwrap();
        let point = solve(&shot, range_m).unwrap().points.remove(0);
        let got = point.velocity_mps / 0.3048;
        assert!((got - fps).abs() < 3.0, "{yards} yd {got:.1} fps vs {fps}");
    }
}
