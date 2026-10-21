//! Holdover's Tauri 2 window. The solver stays in `holdover-core`.
//!
//! This is the Milestone 2 solve screen: profiles, a separate zero atmosphere,
//! current conditions, and the hold. SQLite lives in this crate because the
//! core library does no I/O.

mod library;
mod session;
mod store;
mod strings;
mod weather;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use holdover_theme::{bundled_palette, text_scale_factor, Palette, ThemeFollower, ThemePaths};
use serde_json::{json, Value};
use store::Store;
use tauri::Emitter;
use tauri::Manager;

struct AppState {
    store: Mutex<Store>,
}

fn main() {
    let store = Store::open(&database_path()).expect("Holdover could not open its database");
    tauri::Builder::default()
        .manage(AppState {
            store: Mutex::new(store),
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            set_preferences,
            complete_onboarding,
            save_rifle,
            save_scope,
            save_bullet,
            save_load,
            save_profile,
            delete_record,
            set_active_profile,
            solve,
            range_card,
            card_pdf,
            chart,
            export_library,
            export_library_csv,
            import_library,
            fetch_weather,
            apply_weather,
            point_blank,
            apply_zero,
            truing,
            chrono_session,
            recoil_calc,
            impact,
            reticle_scene,
            reticle_save,
            target_save,
            target_pdf
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || theme_loop(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Holdover window failed to start");
}

fn database_path() -> PathBuf {
    if let Ok(path) = std::env::var("HOLDOVER_DB") {
        return PathBuf::from(path);
    }
    let base = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        format!("{home}/.local/share")
    });
    PathBuf::from(base).join("holdover/holdover.db")
}

fn with_store<T>(
    state: tauri::State<'_, AppState>,
    f: impl FnOnce(&Store) -> Result<T, String>,
) -> Result<T, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "the library is busy".to_string())?;
    f(&store)
}

#[tauri::command]
fn bootstrap(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    with_store(state, Store::bootstrap)
}

#[tauri::command]
fn set_preferences(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| {
        store.set_preferences(&body)?;
        store.bootstrap()
    })
}

#[tauri::command]
fn complete_onboarding(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| {
        store.complete_onboarding(&body)?;
        store.bootstrap()
    })
}

#[tauri::command]
fn save_rifle(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| store.save_rifle(&body))
}

#[tauri::command]
fn save_scope(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| store.save_scope(&body))
}

#[tauri::command]
fn save_bullet(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| store.save_bullet(&body))
}

#[tauri::command]
fn save_load(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| store.save_load(&body))
}

#[tauri::command]
fn save_profile(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| store.save_profile(&body))
}

#[tauri::command]
fn delete_record(
    state: tauri::State<'_, AppState>,
    kind: String,
    id: String,
) -> Result<(), String> {
    with_store(state, |store| store.delete_record(&kind, &id))
}

#[tauri::command]
fn set_active_profile(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    with_store(state, |store| store.set_active(&id))
}

#[tauri::command]
fn solve(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::solve(store, &body))
}

#[tauri::command]
fn range_card(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::range_card(store, &body))
}

/// Write the range card as a PDF at the physical page size and open it.
#[tauri::command]
fn card_pdf(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| {
        let bytes = session::card_pdf_bytes(store, &body)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("holdover-range-card-{stamp}.pdf"));
        fs::write(&path, &bytes).map_err(|err| err.to_string())?;
        let opened = open_pdf(&path);
        Ok(json!({
            "path": path,
            "opened": opened,
            "pdf_base64": base64_encode(&bytes),
        }))
    })
}

fn open_pdf(path: &Path) -> bool {
    Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let packed = ((bytes[index] as u32) << 16)
            | ((bytes[index + 1] as u32) << 8)
            | bytes[index + 2] as u32;
        out.push(TABLE[((packed >> 18) & 63) as usize] as char);
        out.push(TABLE[((packed >> 12) & 63) as usize] as char);
        out.push(TABLE[((packed >> 6) & 63) as usize] as char);
        out.push(TABLE[(packed & 63) as usize] as char);
        index += 3;
    }
    let rest = bytes.len() - index;
    if rest == 1 {
        let packed = (bytes[index] as u32) << 16;
        out.push(TABLE[((packed >> 18) & 63) as usize] as char);
        out.push(TABLE[((packed >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let packed = ((bytes[index] as u32) << 16) | ((bytes[index + 1] as u32) << 8);
        out.push(TABLE[((packed >> 18) & 63) as usize] as char);
        out.push(TABLE[((packed >> 12) & 63) as usize] as char);
        out.push(TABLE[((packed >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

#[tauri::command]
fn chart(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::chart(store, &body))
}

#[tauri::command]
fn export_library(
    state: tauri::State<'_, AppState>,
    include_location: Option<bool>,
) -> Result<Value, String> {
    with_store(state, |store| {
        let doc = if include_location.unwrap_or(false) {
            store.export_library_with(true)?
        } else {
            store.export_library()?
        };
        let path = backup_dir()?.join(format!(
            "holdover-{}.json",
            store::now_iso().replace(':', "")
        ));
        fs::write(
            &path,
            serde_json::to_vec_pretty(&doc).map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
        Ok(json!({ "path": path, "document": doc }))
    })
}

#[tauri::command]
fn export_library_csv(
    state: tauri::State<'_, AppState>,
    include_location: Option<bool>,
) -> Result<Value, String> {
    with_store(state, |store| {
        let csv = store.export_library_csv(include_location.unwrap_or(false))?;
        let path = backup_dir()?.join(format!(
            "holdover-{}.csv",
            store::now_iso().replace(':', "")
        ));
        fs::write(&path, &csv).map_err(|err| err.to_string())?;
        Ok(json!({ "path": path, "csv": csv }))
    })
}

/// Open-Meteo lookup. The webview stays responsive: this command does not run
/// on the UI thread, and the request gives up after a few seconds.
#[tauri::command]
async fn fetch_weather(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let (latitude, longitude, temperature, pressure, length, velocity) = {
        let store = state
            .store
            .lock()
            .map_err(|_| "the library is busy".to_string())?;
        let place = store.weather_place()?;
        let units = store.units()?;
        (
            place.latitude,
            place.longitude,
            units.temperature,
            units.pressure,
            units.length,
            units.velocity,
        )
    };
    let (Some(latitude), Some(longitude)) = (latitude, longitude) else {
        return Ok(weather::place_missing());
    };
    let fetched = tauri::async_runtime::spawn_blocking(move || {
        weather::fetch(
            latitude,
            longitude,
            weather::FETCH_TIMEOUT,
            weather::FORECAST_URL,
        )
    })
    .await
    .map_err(|err| err.to_string())?;
    Ok(match fetched {
        Ok(value) => value.presented(weather::PresentUnits {
            temperature: &temperature,
            pressure: &pressure,
            length: &length,
            velocity: &velocity,
        }),
        Err(err) => err.body(),
    })
}

#[tauri::command]
fn apply_weather(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| {
        store.apply_weather(&body)?;
        store.bootstrap()
    })
}

#[tauri::command]
fn point_blank(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::point_blank(store, &body))
}

#[tauri::command]
fn apply_zero(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::apply_zero(store, &body))
}

#[tauri::command]
fn truing(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::truing(store, &body))
}

#[tauri::command]
fn chrono_session(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::chrono_session(store, &body))
}

#[tauri::command]
fn recoil_calc(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::recoil_calc(store, &body))
}

#[tauri::command]
fn impact(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::impact(store, &body))
}

#[tauri::command]
fn reticle_scene(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::reticle_scene(store, &body))
}

#[tauri::command]
fn reticle_save(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| store.save_reticle(&body))
}

#[tauri::command]
fn target_save(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| store.save_target_record(&body))
}

/// Write a target sheet as a PDF. The scale bar is true paper size. The figure is 1:1 or 1:N.
#[tauri::command]
fn target_pdf(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| {
        let printed = session::target_pdf_bytes(store, &body)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("holdover-target-{stamp}.pdf"));
        fs::write(&path, &printed.pdf).map_err(|err| err.to_string())?;
        let opened = open_pdf(&path);
        Ok(json!({
            "path": path,
            "opened": opened,
            "pdf_base64": base64_encode(&printed.pdf),
            "scale_denominator": printed.scale_denominator,
            "scale_bar_mm": printed.scale_bar_mm,
        }))
    })
}

#[tauri::command]
fn import_library(
    state: tauri::State<'_, AppState>,
    document: Value,
    commit: bool,
    policy: Option<String>,
) -> Result<Value, String> {
    with_store(state, |store| {
        let policy = policy.unwrap_or_else(|| "replace".into());
        if let Some(csv) = document.get("csv").and_then(|value| value.as_str()) {
            store.import_text(csv, commit, &policy)
        } else if policy == "replace" {
            store.import_library(&document, commit)
        } else {
            store.import_prepared(&document, commit, &policy, None)
        }
    })
}

fn backup_dir() -> Result<PathBuf, String> {
    let dir = database_path()
        .parent()
        .unwrap_or(std::path::Path::new("/tmp"))
        .join("backups");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir)
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
        let appearance = if let Some(state) = handle.try_state::<AppState>() {
            match state.store.lock() {
                Ok(store) => store.appearance().unwrap_or_else(|_| "follow".into()),
                Err(_) => "follow".into(),
            }
        } else {
            "follow".into()
        };
        let palette = if appearance == "light" || appearance == "dark" {
            bundled_palette(&appearance)
        } else {
            follower.reload(&paths)
        };
        let body = palette_body(&palette);
        let _ = write_status(&body);
        if handle.emit("theme", &body).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn palette_body(palette: &Palette) -> Value {
    let tokens = &palette.tokens;
    json!({
        "mode": tokens.mode,
        "background": tokens.background,
        "surface": tokens.surface,
        "text": tokens.text,
        "text_secondary": tokens.text_secondary,
        "border": tokens.border,
        "accent": tokens.accent,
        "focus": tokens.focus,
        "warning": tokens.warning,
        "error": tokens.error,
        "chart": tokens.chart,
        "source": format!("{:?}", palette.source),
        "text_scale": text_scale_factor(),
    })
}

fn write_status(body: &Value) -> std::io::Result<()> {
    let path = status_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(body).unwrap_or_default())
}
