//! Synthetic group photographs and the published scoring rules.
//!
//! Plan §27.2: a 4-point homography on a synthetic image keeps each hole within
//! 0.5 mm at A4 scale, and extreme spread, mean radius, and the offset stay
//! within 1% of the known group.

use holdover_core::{
    analyze_document, group_stats, homography, score_holes, target_by_id, two_point_scale,
    ImagePoint, MeasureMode, PlaneMm, CEP_SIGMA,
};

fn spread(points: &[PlaneMm]) -> f64 {
    let mut best: f64 = 0.0;
    for left in 0..points.len() {
        for right in (left + 1)..points.len() {
            best = best
                .max((points[left].x - points[right].x).hypot(points[left].y - points[right].y));
        }
    }
    best
}

fn centre(points: &[PlaneMm]) -> PlaneMm {
    let n = points.len() as f64;
    PlaneMm {
        x: points.iter().map(|point| point.x).sum::<f64>() / n,
        y: points.iter().map(|point| point.y).sum::<f64>() / n,
    }
}

fn mean_radius(points: &[PlaneMm]) -> f64 {
    let middle = centre(points);
    points
        .iter()
        .map(|point| (point.x - middle.x).hypot(point.y - middle.y))
        .sum::<f64>()
        / points.len() as f64
}

#[test]
fn four_corners_round_trip_and_a_known_translation() {
    let plane = [
        PlaneMm { x: 0.0, y: 50.0 },
        PlaneMm { x: 100.0, y: 50.0 },
        PlaneMm { x: 100.0, y: 0.0 },
        PlaneMm { x: 0.0, y: 0.0 },
    ];
    let image = [
        ImagePoint { x: 10.0, y: 20.0 },
        ImagePoint { x: 110.0, y: 20.0 },
        ImagePoint { x: 110.0, y: 70.0 },
        ImagePoint { x: 10.0, y: 70.0 },
    ];
    let mapped = homography(image, plane).unwrap();
    let hole = mapped.map(ImagePoint { x: 60.0, y: 45.0 }).unwrap();
    assert!((hole.x - 50.0).abs() < 1e-6, "{}", hole.x);
    assert!((hole.y - 25.0).abs() < 1e-6, "{}", hole.y);
    let back = mapped.invert_map(hole).unwrap();
    assert!((back.x - 60.0).abs() < 1e-6);
    assert!((back.y - 45.0).abs() < 1e-6);
}

#[test]
fn collinear_corners_are_refused() {
    let image = [
        ImagePoint { x: 0.0, y: 0.0 },
        ImagePoint { x: 10.0, y: 0.0 },
        ImagePoint { x: 20.0, y: 0.0 },
        ImagePoint { x: 30.0, y: 0.0 },
    ];
    let plane = [
        PlaneMm { x: 0.0, y: 0.0 },
        PlaneMm { x: 10.0, y: 0.0 },
        PlaneMm { x: 10.0, y: 10.0 },
        PlaneMm { x: 0.0, y: 10.0 },
    ];
    assert!(homography(image, plane).is_err());
}

#[test]
fn synthetic_homography_meets_the_millimetre_tolerance() {
    let check = holdover_core::check_synthetic().unwrap();
    assert!(
        check.passed,
        "position {:.3} mm, ES ratio {:.4}, MR ratio {:.4}, offset ratio {:.4}\n{}",
        check.max_position_error_mm,
        check.extreme_spread_ratio,
        check.mean_radius_ratio,
        check.offset_ratio,
        check.json()
    );
    assert!(check.max_position_error_mm <= 0.5);
    assert!(check.extreme_spread_ratio <= 0.01);
    assert!(check.mean_radius_ratio <= 0.01);
    assert!(check.offset_ratio <= 0.01);
}

#[test]
fn group_stats_match_the_definitions_within_a_percent() {
    let scene = holdover_core::synthetic_scene();
    let stats = group_stats(
        &scene.holes_mm,
        scene.poa_mm,
        scene.distance_m,
        MeasureMode::Centres,
        0.1,
    )
    .unwrap();
    let expected_spread = spread(&scene.holes_mm);
    let expected_radius = mean_radius(&scene.holes_mm);
    let middle = centre(&scene.holes_mm);
    assert!(relative(stats.extreme_spread_mm, expected_spread) <= 0.01);
    assert!(relative(stats.mean_radius_mm, expected_radius) <= 0.01);
    assert!(relative(stats.offset_x_mm, middle.x - scene.poa_mm.x) <= 0.01);
    assert!(relative(stats.offset_y_mm, middle.y - scene.poa_mm.y) <= 0.01);
    assert!((stats.offset_x_mm - 22.0).abs() < 1e-9);
    assert!((stats.offset_y_mm - -16.0).abs() < 1e-9);
    assert_eq!(stats.vertical, "DOWN");
    assert_eq!(stats.horizontal, "RIGHT");
    let sigma = ((stats.sd_x_mm.powi(2) + stats.sd_y_mm.powi(2)) / 2.0).sqrt();
    let cep = stats.cep_mm.expect("five shots show a CEP");
    assert!((cep - CEP_SIGMA * sigma).abs() < 1e-9);
    assert!(stats.median_radius_mm.is_some());
    assert!(stats.warnings.iter().any(|item| item == "cep-small-sample"));
    let mrad = (stats.extreme_spread_mm / 1000.0 / scene.distance_m).atan() * 1000.0;
    assert!((stats.es_mrad.unwrap() - mrad).abs() < 1e-9);
    assert!((stats.es_moa.unwrap() - mrad * (10.8 / std::f64::consts::PI)).abs() < 1e-9);
    assert!(
        (stats.correction_elevation_mrad.unwrap() + stats.offset_y_mrad.unwrap()).abs() < 1e-12
    );
}

#[test]
fn two_point_scale_warns_and_refuses_a_fraction_of_a_millimetre() {
    let scale = two_point_scale(
        ImagePoint { x: 0.0, y: 0.0 },
        ImagePoint { x: 100.0, y: 0.0 },
        50.0,
        Some([
            ImagePoint { x: 0.0, y: 0.0 },
            ImagePoint { x: 100.0, y: 2.0 },
            ImagePoint { x: 0.0, y: 40.0 },
            ImagePoint { x: 100.0, y: 40.0 },
        ]),
    )
    .unwrap();
    assert!((scale.mm_per_px - 0.5).abs() < 1e-12);
    assert_eq!(scale.precision_mm, 1.0);
    assert!(scale.skew_deg.unwrap() > 0.5);
    let points = [PlaneMm { x: 0.4, y: 0.0 }, PlaneMm { x: 10.6, y: 0.2 }];
    let stats = group_stats(
        &points,
        PlaneMm { x: 0.0, y: 0.0 },
        100.0,
        MeasureMode::Centres,
        scale.precision_mm,
    )
    .unwrap();
    assert!(stats.warnings.iter().any(|item| item == "no-sub-mm"));
    assert_eq!(
        stats.reported_extreme_spread_mm,
        stats.reported_extreme_spread_mm.round()
    );
    assert!(stats.reported_extreme_spread_mm.fract() == 0.0);
}

#[test]
fn outside_edges_lose_one_bullet_diameter() {
    let points = [PlaneMm { x: 0.0, y: 0.0 }, PlaneMm { x: 20.0, y: 0.0 }];
    let centres = group_stats(
        &points,
        PlaneMm { x: 0.0, y: 0.0 },
        50.0,
        MeasureMode::Centres,
        0.1,
    )
    .unwrap();
    let edges = group_stats(
        &points,
        PlaneMm { x: 0.0, y: 0.0 },
        50.0,
        MeasureMode::OutsideEdges {
            bullet_diameter_mm: 7.82,
        },
        0.1,
    )
    .unwrap();
    assert!((centres.extreme_spread_mm - 20.0).abs() < 1e-9);
    assert!((edges.extreme_spread_mm - (20.0 - 7.82)).abs() < 1e-9);
    assert!(edges.warnings.iter().any(|item| item == "edge-adjusted"));
}

#[test]
fn moose_rings_score_and_the_placeholder_hit_area_does_not() {
    let moose = target_by_id("sjf-algfigur").unwrap();
    let score = score_holes(
        &[
            PlaneMm { x: 0.0, y: 0.0 },
            PlaneMm { x: 100.0, y: 0.0 },
            PlaneMm { x: 300.0, y: 100.0 },
        ],
        &moose,
        false,
    );
    assert!(score.available);
    assert_eq!(score.shots[0].label, "5¹");
    assert_eq!(score.shots[0].points, Some(5));
    assert_eq!(score.shots[1].points, Some(5));
    assert!(
        score.shots[2].miss,
        "a point outside ring 3 is not given the placeholder hit area"
    );
    assert!(score
        .notes
        .iter()
        .any(|item| item == "placeholder-not-scored"));
    assert_eq!(score.total, Some(10));
    let alt = score_holes(&[PlaneMm { x: 90.0, y: 0.0 }], &moose, true);
    assert_eq!(alt.shots[0].label, "4");
    assert_eq!(alt.shots[0].points, Some(4));
}

#[test]
fn an_unmeasured_target_is_not_scored() {
    let boar = target_by_id("sjf-vildsvin").unwrap();
    let score = score_holes(&[PlaneMm { x: 0.0, y: 0.0 }], &boar, false);
    assert!(!score.available);
    assert_eq!(score.reason, "no-published-zones");
    let bull = target_by_id("generic-bullseye").unwrap();
    let practice = score_holes(&[PlaneMm { x: 0.0, y: 0.0 }], &bull, false);
    assert!(
        !practice.available,
        "an unverified practice ring is not a score"
    );
}

#[test]
fn analyze_document_uses_the_rectangle_centre_as_the_aim_point() {
    let scene = holdover_core::synthetic_scene();
    let image_points: Vec<Vec<f64>> = scene
        .corners_px
        .iter()
        .map(|point| vec![point.x, point.y])
        .collect();
    let holes: Vec<_> = scene
        .holes_px
        .iter()
        .map(|point| serde_json::json!({"image": [point.x, point.y], "count": 1}))
        .collect();
    let input = serde_json::json!({
        "calibration": {
            "mode": "homography",
            "image_points": image_points,
            "width_mm": scene.width_mm,
            "height_mm": scene.height_mm,
        },
        "use_rectangle_centre": true,
        "holes": holes,
        "distance_m": 100.0,
        "expected_shots": 5,
        "measure": "centres",
        "ringing": "primary",
    });
    let moose = target_by_id("sjf-algfigur").unwrap();
    let report = analyze_document(&input, Some(&moose)).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["can_save"], true);
    assert!((report["stats"]["offset_x_mm"].as_f64().unwrap() - 22.0).abs() < 1e-6);
    assert!((report["stats"]["offset_y_mm"].as_f64().unwrap() + 16.0).abs() < 1e-6);
    assert!(report["score"]["available"].as_bool().unwrap());
}

#[test]
fn two_point_analysis_names_the_orthogonal_warning() {
    let input = serde_json::json!({
        "calibration": {
            "mode": "two-point",
            "image_points": [[0.0, 0.0], [200.0, 0.0]],
            "known_mm": 100.0,
        },
        "poa_image": [0.0, 0.0],
        "holes": [
            {"image": [0.0, 0.0], "count": 1},
            {"image": [40.0, 0.0], "count": 1}
        ],
        "distance_m": 100.0,
        "expected_shots": 2,
    });
    let report = analyze_document(&input, None).unwrap();
    let warnings = report["warnings"].as_array().unwrap();
    assert!(warnings.iter().any(|item| item == "orthogonal-photo"));
    assert!(warnings.iter().any(|item| item == "no-sub-mm"));
    assert_eq!(report["stats"]["precision_mm"], 1.0);
    assert_eq!(report["can_save"], true);
}

fn relative(measured: f64, expected: f64) -> f64 {
    (measured - expected).abs() / expected.abs()
}
