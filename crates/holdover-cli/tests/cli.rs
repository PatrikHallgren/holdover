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
