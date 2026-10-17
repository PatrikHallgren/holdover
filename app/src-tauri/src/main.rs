//! Tauri 2 spike shell. The solver stays in `holdover-core`.
//!
//! This is the window used to decide between Tauri and Qt on Omarchy 4.
//! It is not the Milestone 2 solve screen.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use holdover_core::{
    parse_quantity, parse_wind, Atmosphere, BcBand, BcReference, Bullet, Dimension, DragModel,
    DragSpec, Effects, Shot, Twist, TwistHand, DISCLAIMER,
};
use holdover_theme::{text_scale_factor, Palette, ThemeFollower, ThemePaths};
use tauri::Emitter;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![solve])
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || theme_loop(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Holdover window failed to start");
}

#[tauri::command]
fn solve(range: String, wind: Option<String>) -> Result<serde_json::Value, String> {
    let mut shot = spike_shot();
    if let Some(wind) = wind.filter(|wind| !wind.trim().is_empty()) {
        shot.wind = parse_wind(&wind).map_err(|err| err.to_string())?;
    }
    let range_m = parse_quantity(&range, Dimension::Length, None).map_err(|err| err.to_string())?;
    let solution = holdover_core::solve(&shot, range_m).map_err(|err| err.to_string())?;
    let point = solution
        .points
        .first()
        .ok_or("the solver returned no point")?;
    Ok(serde_json::json!({
        "range_m": point.range_m,
        "elevation_mrad": point.elevation.mrad,
        "elevation_moa": point.elevation.moa,
        "elevation_iphy": point.elevation.iphy,
        "elevation_cm_per_100m": point.elevation.cm_per_100m,
        "elevation_direction": direction_name(point.elevation.direction),
        "elevation_clicks": point.elevation.clicks,
        "windage_m": point.windage_m,
        "windage_mrad": point.windage.mrad,
        "windage_direction": direction_name(point.windage.direction),
        "velocity_mps": point.velocity_mps,
        "energy_j": point.energy_j,
        "tof_s": point.tof_s,
        "mach": point.mach,
        "disclaimer": DISCLAIMER,
    }))
}

fn direction_name(direction: holdover_core::Direction) -> &'static str {
    match direction {
        holdover_core::Direction::Up => "UP",
        holdover_core::Direction::Down => "DOWN",
        holdover_core::Direction::Left => "LEFT",
        holdover_core::Direction::Right => "RIGHT",
        holdover_core::Direction::None => "—",
    }
}

fn spike_shot() -> Shot {
    let atmo = Atmosphere::icao();
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
        atmo,
    );
    shot.sight_height_m = 0.045;
    shot.zero_range_m = Some(100.0);
    shot.zero_atmosphere = Some(atmo);
    shot.twist = Some(Twist {
        metres_per_turn: 10.0 * 0.0254,
        hand: TwistHand::Right,
    });
    shot.effects = Effects {
        spin_drift: false,
        ..Effects::default()
    };
    shot
}

fn theme_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("HOLDOVER_THEME_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/state/omarchy/current")
}

fn status_path() -> PathBuf {
    if let Ok(path) = std::env::var("HOLDOVER_PALETTE_STATUS") {
        return PathBuf::from(path);
    }
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join("holdover/palette.json")
}

fn theme_loop(handle: tauri::AppHandle) {
    let mut follower = ThemeFollower::new();
    let paths = ThemePaths::new(theme_dir());
    loop {
        let palette = follower.reload(&paths);
        let body = palette_body(&palette);
        let _ = write_status(&body);
        if handle.emit("theme", &body).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn palette_body(palette: &Palette) -> serde_json::Value {
    let tokens = &palette.tokens;
    serde_json::json!({
        "mode": tokens.mode,
        "background": tokens.background,
        "surface": tokens.surface,
        "text": tokens.text,
        "text_secondary": tokens.text_secondary,
        "border": tokens.border,
        "accent": tokens.accent,
        "source": format!("{:?}", palette.source),
        "text_scale": text_scale_factor(),
    })
}

fn write_status(body: &serde_json::Value) -> std::io::Result<()> {
    let path = status_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(body).unwrap_or_default())
}
