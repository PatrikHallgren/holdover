use std::process::Command;

fn bin() -> Command {
    let path = std::env::var("CARGO_BIN_EXE_holdover_cli")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_holdover-cli"))
        .expect("Cargo should expose the holdover-cli test binary");
    Command::new(path)
}

#[test]
fn vacuum_json_matches_the_parabola() {
    let profile = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/vacuum-flat.json"
    );
    let output = bin()
        .args(["solve", "--profile", profile, "--range", "100m", "--json"])
        .output()
        .expect("run cli");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let point = &json["points"][0];
    let path = point["path_m"].as_f64().unwrap();
    let expected = -0.5 * 9.80665 * (100.0 / 800.0_f64).powi(2);
    assert!((path - expected).abs() < 1e-6, "{path}");
    let text = json["disclaimer"].as_str().unwrap();
    assert!(text.contains("Confirm with live fire"));
}

#[test]
fn wind_flag_overrides_the_profile() {
    let profile = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/308-g7.json");
    let output = bin()
        .args([
            "solve",
            "--profile",
            profile,
            "--range",
            "300m",
            "--wind",
            "3mps@9oc",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["points"][0]["windage_direction"], "LEFT");
    assert!(json["points"][0]["windage_m"].as_f64().unwrap() > 0.0);
}

#[test]
fn chrono_keeps_the_outlier_and_recoil_names_its_convention() {
    let chrono = bin()
        .args([
            "chrono",
            "--shots",
            "800, 800, 800, 800, 800, 800, 800, 900 mps",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        chrono.status.success(),
        "{}",
        String::from_utf8_lossy(&chrono.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&chrono.stdout).unwrap();
    assert!((json["mean_mps"].as_f64().unwrap() - 812.5).abs() < 1e-6);
    assert_eq!(json["outliers"][0], 7);

    let recoil = bin()
        .args([
            "recoil", "--rifle", "8 lb", "--bullet", "150 gr", "--charge", "45 gr", "--mv",
            "2800 fps", "--json",
        ])
        .output()
        .unwrap();
    assert!(
        recoil.status.success(),
        "{}",
        String::from_utf8_lossy(&recoil.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&recoil.stdout).unwrap();
    let fps = json["velocity_fps"].as_f64().unwrap();
    assert!((fps - 600000.0 / 56000.0).abs() < 1e-4, "{fps}");
    assert!(json["convention"]
        .as_str()
        .unwrap()
        .contains("not been confirmed"));
}

#[test]
fn point_blank_json_reports_near_and_far() {
    let profile = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/308-g7.json");
    let output = bin()
        .args(["pbr", "--profile", profile, "--zone", "15cm", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let near = json["near_m"].as_f64().unwrap();
    let zero = json["optimal_zero_m"].as_f64().unwrap();
    let far = json["far_m"].as_f64().unwrap();
    assert!(near < zero && zero < far, "{near} {zero} {far}");
}

#[test]
fn table_without_a_unit_suffix_names_an_example() {
    let profile = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/308-g7.json");
    let output = bin()
        .args([
            "table",
            "--profile",
            profile,
            "--from",
            "100",
            "--to",
            "300m",
            "--step",
            "100m",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("100m"), "{err}");
}
