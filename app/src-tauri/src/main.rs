//! Holdover's Tauri 2 window. The solver stays in `holdover-core`.
//!
//! This is the Milestone 2 solve screen: profiles, a separate zero atmosphere,
//! current conditions, and the hold. SQLite lives in this crate because the
//! core library does no I/O.

mod session;
mod store;
mod strings;

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

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
            chart,
            export_library,
            import_library
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

#[tauri::command]
fn chart(state: tauri::State<'_, AppState>, body: Value) -> Result<Value, String> {
    with_store(state, |store| session::chart(store, &body))
}

#[tauri::command]
fn export_library(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    with_store(state, |store| {
        let doc = store.export_library()?;
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
fn import_library(
    state: tauri::State<'_, AppState>,
    document: Value,
    commit: bool,
) -> Result<Value, String> {
    with_store(state, |store| store.import_library(&document, commit))
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
