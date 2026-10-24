//! SQLite library. Each record stores SI values beside the original text.
//!
//! Settings hold the unit preset. Changing that preset does not update record
//! bodies, so a save/reload after a switch keeps the same normalized values.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use holdover_core::{
    card_page, card_pages, default_card_page, format_number, format_quantity, parse_quantity,
    running_presets, Dimension, DEFAULT_EJECTA_MPS, EJECTA_CONVENTION, SJF_MOOSE_RING_DIAMETER_CM,
    SJF_ROE_BUCK_RING_DIAMETER_CM,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use uuid::Uuid;

const SCHEMA: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE records (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  revision INTEGER NOT NULL,
  deleted_at TEXT,
  device_id TEXT NOT NULL,
  body TEXT NOT NULL
);
CREATE INDEX records_kind ON records(kind);
CREATE TABLE settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  onboarded INTEGER NOT NULL,
  preset TEXT NOT NULL,
  units_json TEXT NOT NULL,
  appearance TEXT NOT NULL,
  large_text INTEGER NOT NULL,
  reduced_motion INTEGER NOT NULL,
  card_pinned INTEGER NOT NULL,
  angular TEXT NOT NULL,
  active_profile_id TEXT NOT NULL,
  current_air_json TEXT NOT NULL,
  effects_json TEXT NOT NULL,
  range_step TEXT NOT NULL,
  wind_step TEXT NOT NULL,
  last_range TEXT NOT NULL,
  last_wind_speed TEXT NOT NULL,
  last_wind_direction TEXT NOT NULL,
  last_inclination TEXT NOT NULL
);
";

#[derive(Clone, Debug)]
pub struct UnitSet {
    pub preset: String,
    pub length: String,
    pub small: String,
    pub angle: String,
    pub temperature: String,
    pub pressure: String,
    pub velocity: String,
    pub mass: String,
    pub energy: String,
    pub angular: String,
}

impl UnitSet {
    pub fn metric() -> Self {
        Self {
            preset: "metric".into(),
            length: "m".into(),
            small: "cm".into(),
            angle: "mrad".into(),
            temperature: "C".into(),
            pressure: "hPa".into(),
            velocity: "mps".into(),
            mass: "g".into(),
            energy: "J".into(),
            angular: "mrad".into(),
        }
    }

    pub fn us() -> Self {
        Self {
            preset: "us".into(),
            length: "yd".into(),
            small: "in".into(),
            angle: "MOA".into(),
            temperature: "F".into(),
            pressure: "inHg".into(),
            velocity: "mph".into(),
            mass: "gr".into(),
            energy: "ftlb".into(),
            angular: "MOA".into(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "length": self.length,
            "small": self.small,
            "angle": self.angle,
            "temperature": self.temperature,
            "pressure": self.pressure,
            "velocity": self.velocity,
            "mass": self.mass,
            "energy": self.energy,
            "angular": self.angular,
        })
    }

    fn resolved(preset: &str, custom: &Value) -> Self {
        let base = if preset == "us" {
            Self::us()
        } else {
            Self::metric()
        };
        if preset != "custom" {
            return Self {
                preset: preset.to_string(),
                ..base
            };
        }
        let pick = |key: &str, fallback: &str| {
            custom
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or(fallback)
                .to_string()
        };
        Self {
            preset: "custom".into(),
            length: pick("length", &base.length),
            small: pick("small", &base.small),
            angle: pick("angle", &base.angle),
            temperature: pick("temperature", &base.temperature),
            pressure: pick("pressure", &base.pressure),
            velocity: pick("velocity", &base.velocity),
            mass: pick("mass", &base.mass),
            energy: pick("energy", &base.energy),
            angular: pick("angular", &base.angular),
        }
    }
}

pub struct WeatherPlace {
    pub name: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub stale_minutes: u64,
}

pub struct Store {
    pub(crate) conn: Connection,
    db_path: PathBuf,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let conn = Connection::open(path).map_err(|err| err.to_string())?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|err| err.to_string())?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap_or(0);
        if version < 1 {
            conn.execute_batch(SCHEMA).map_err(|err| err.to_string())?;
            conn.pragma_update(None, "user_version", 1)
                .map_err(|err| err.to_string())?;
            conn.execute(
                "INSERT INTO meta (key, value) VALUES ('device_id', ?1)",
                params![Uuid::new_v4().to_string()],
            )
            .map_err(|err| err.to_string())?;
            let units = UnitSet::metric();
            conn.execute(
                "INSERT INTO settings (
                    id, onboarded, preset, units_json, appearance, large_text, reduced_motion,
                    card_pinned, angular, active_profile_id, current_air_json, effects_json,
                    range_step, wind_step, last_range, last_wind_speed, last_wind_direction, last_inclination
                 ) VALUES (1, 0, 'metric', ?1, 'follow', 0, 0, 0, 'mrad', '', ?2, ?3,
                    '10 m', '1 mps', '300 m', '0 mps', '12oc', '0 deg')",
                params![units.to_json().to_string(), default_air().to_string(), default_effects().to_string()],
            ).map_err(|err| err.to_string())?;
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sync_base (
                id TEXT PRIMARY KEY,
                device_id TEXT NOT NULL,
                revision INTEGER NOT NULL,
                updated_at TEXT NOT NULL,
                deleted INTEGER NOT NULL,
                suppress TEXT NOT NULL DEFAULT ''
            );",
        )
        .map_err(|err| err.to_string())?;
        Ok(Self {
            conn,
            db_path: path.to_path_buf(),
        })
    }

    pub fn device_id(&self) -> Result<String, String> {
        self.conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'device_id'",
                [],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())
    }

    pub fn appearance(&self) -> Result<String, String> {
        self.conn
            .query_row("SELECT appearance FROM settings WHERE id = 1", [], |row| {
                row.get(0)
            })
            .map_err(|err| err.to_string())
    }

    pub fn units(&self) -> Result<UnitSet, String> {
        let (preset, raw): (String, String) = self
            .conn
            .query_row(
                "SELECT preset, units_json FROM settings WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|err| err.to_string())?;
        let custom: Value = serde_json::from_str(&raw).unwrap_or(json!({}));
        let mut units = UnitSet::resolved(&preset, &custom);
        if let Some(angular) = custom.get("angular").and_then(|v| v.as_str()) {
            if preset == "custom" || !angular.is_empty() {
                units.angular = angular.to_string();
            }
        }
        Ok(units)
    }

    pub fn bootstrap(&self) -> Result<Value, String> {
        let units = self.units()?;
        let mut settings = self.settings_json()?;
        decorate_settings(&mut settings, &units);
        Ok(json!({
            "onboarded": self.flag("onboarded")?,
            "settings": settings,
            "equipment": self.equipment_json(&units)?,
            "strings": crate::strings::english_strings(),
            "version": env!("CARGO_PKG_VERSION"),
            "licence": include_str!("../../../LICENSE"),
            "disclaimer": holdover_core::DISCLAIMER,
            "spin_provenance": holdover_core::SPIN_DRIFT_PROVENANCE,
            "targets": {
                "moose_cm": SJF_MOOSE_RING_DIAMETER_CM,
                "roe_cm": SJF_ROE_BUCK_RING_DIAMETER_CM,
                "library": holdover_core::library_value(),
            },
            "reticles": holdover_core::generic_reticles()
                .iter()
                .map(holdover_core::reticle_json)
                .collect::<Vec<_>>(),
            "running": running_presets()
                .iter()
                .map(|preset| {
                    json!({
                        "id": preset.id,
                        "name": preset.name,
                        "range_m": preset.range_m,
                        "speed_mps": preset.speed_mps,
                        "speed_known": preset.speed_known,
                        "note": preset.note,
                    })
                })
                .collect::<Vec<_>>(),
            "card_pages": card_pages()
                .iter()
                .map(|page| {
                    json!({
                        "id": page.id,
                        "name": page.name,
                        "width_mm": page.width_mm,
                        "height_mm": page.height_mm,
                    })
                })
                .collect::<Vec<_>>(),
            "ejecta_mps": DEFAULT_EJECTA_MPS,
            "ejecta_convention": EJECTA_CONVENTION,
            "paper": self.paper_id()?,
        }))
    }

    pub fn set_preferences(&self, body: &Value) -> Result<(), String> {
        let preset = body
            .get("preset")
            .and_then(|v| v.as_str())
            .unwrap_or("metric");
        if !matches!(preset, "metric" | "us" | "custom") {
            return Err("preset must be metric, us, or custom".into());
        }
        let custom = body.get("units").cloned().unwrap_or(json!({}));
        let mut units = UnitSet::resolved(preset, &custom);
        if let Some(angular) = body.get("angular").and_then(|v| v.as_str()) {
            units.angular = angular.to_string();
        }
        let appearance = body
            .get("appearance")
            .and_then(|v| v.as_str())
            .unwrap_or("follow");
        if !matches!(appearance, "follow" | "light" | "dark") {
            return Err("appearance must be follow, light, or dark".into());
        }
        if let Some(paper) = body.get("paper").and_then(|value| value.as_str()) {
            if card_page(paper).is_none() {
                return Err(format!("unknown card size {paper}"));
            }
        }
        self.conn.execute(
            "UPDATE settings SET preset = ?1, units_json = ?2, appearance = ?3, large_text = ?4,
             reduced_motion = ?5, card_pinned = ?6, angular = ?7 WHERE id = 1",
            params![
                units.preset,
                units.to_json().to_string(),
                appearance,
                bool_or(body, "large_text", false) as i64,
                bool_or(body, "reduced_motion", false) as i64,
                bool_or(body, "card_pinned", false) as i64,
                units.angular
            ],
        ).map_err(|err| err.to_string())?;
        if let Some(paper) = body.get("paper").and_then(|value| value.as_str()) {
            self.remember_paper(paper)?;
        }
        if let Some(step) = body.get("range_step").and_then(|value| value.as_str()) {
            let step = step.trim();
            if step.is_empty() {
                return Err("range step is empty".into());
            }
            self.conn
                .execute(
                    "UPDATE settings SET range_step = ?1 WHERE id = 1",
                    params![step],
                )
                .map_err(|err| err.to_string())?;
        }
        if let Some(step) = body.get("wind_step").and_then(|value| value.as_str()) {
            let step = step.trim();
            if step.is_empty() {
                return Err("wind step is empty".into());
            }
            self.conn
                .execute(
                    "UPDATE settings SET wind_step = ?1 WHERE id = 1",
                    params![step],
                )
                .map_err(|err| err.to_string())?;
        }
        if body.get("weather_place").is_some()
            || body.get("weather_latitude").is_some()
            || body.get("weather_longitude").is_some()
            || body.get("weather_stale_minutes").is_some()
        {
            self.save_weather_place(body)?;
        }
        Ok(())
    }

    pub fn save_weather_place(&self, body: &Value) -> Result<(), String> {
        if let Some(name) = body.get("weather_place").and_then(|v| v.as_str()) {
            self.meta_put("weather_place", name.trim())?;
        }
        if body.get("weather_latitude").is_some() {
            match parse_coord(&coord_text(body, "weather_latitude"), -90.0, 90.0) {
                Ok(Some(value)) => self.meta_put("weather_latitude", &value.to_string())?,
                Ok(None) => self.meta_del("weather_latitude")?,
                Err(_) => return Err("place-latitude".into()),
            }
        }
        if body.get("weather_longitude").is_some() {
            match parse_coord(&coord_text(body, "weather_longitude"), -180.0, 180.0) {
                Ok(Some(value)) => self.meta_put("weather_longitude", &value.to_string())?,
                Ok(None) => self.meta_del("weather_longitude")?,
                Err(_) => return Err("place-longitude".into()),
            }
        }
        if body.get("weather_stale_minutes").is_some() {
            let minutes = stale_minutes(body.get("weather_stale_minutes"))?;
            self.meta_put("weather_stale_minutes", &minutes.to_string())?;
        }
        Ok(())
    }

    pub fn weather_place(&self) -> Result<WeatherPlace, String> {
        let name = self.meta_get("weather_place")?.unwrap_or_default();
        let latitude = self
            .meta_get("weather_latitude")?
            .and_then(|text| text.parse().ok());
        let longitude = self
            .meta_get("weather_longitude")?
            .and_then(|text| text.parse().ok());
        let stale_minutes = self
            .meta_get("weather_stale_minutes")?
            .and_then(|text| text.parse().ok())
            .unwrap_or(60);
        Ok(WeatherPlace {
            name,
            latitude,
            longitude,
            stale_minutes,
        })
    }

    /// Remember the range-card paper size. Unknown ids are refused.
    pub fn remember_paper(&self, page_id: &str) -> Result<(), String> {
        if card_page(page_id).is_none() {
            return Err(format!("unknown card size {page_id}"));
        }
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES ('paper', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![page_id],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    /// Saved paper size, or Letter for a US locale or yard unit, otherwise A4.
    pub fn paper_id(&self) -> Result<String, String> {
        let saved: Option<String> = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = 'paper'", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|err| err.to_string())?;
        if let Some(saved) = saved {
            if card_page(&saved).is_some() {
                return Ok(saved);
            }
        }
        let units = self.units()?;
        let locale = std::env::var("LC_PAPER")
            .or_else(|_| std::env::var("LC_ALL"))
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default();
        Ok(default_card_page(&units.length, &locale).to_string())
    }

    pub fn complete_onboarding(&self, body: &Value) -> Result<(), String> {
        self.set_preferences(body)?;
        if body
            .get("example")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let id = self.seed_example()?;
            self.set_active(&id)?;
        }
        self.conn
            .execute("UPDATE settings SET onboarded = 1 WHERE id = 1", [])
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn save_rifle(&self, body: &Value) -> Result<Value, String> {
        let mut record = json!({
            "label": required_text(body, "label")?,
            "make_model": text_or(body, "make_model"),
            "twist_text": text_field(body, "twist"),
            "twist_hand": hand(body)?,
            "serial": text_or(body, "serial"),
        });
        put_qty(
            &mut record,
            "barrel_length",
            optional_qty(body, "barrel_length", Dimension::Length, "in")?,
        )?;
        put_required(
            &mut record,
            "sight_height",
            required_qty(body, "sight_height", Dimension::Length, "mm")?,
        )?;
        put_qty(
            &mut record,
            "weight",
            optional_qty(body, "weight", Dimension::Mass, "kg")?,
        )?;
        let id = self.write_record("rifle", body, record)?;
        self.record_value("rifle", &id)
    }

    pub fn save_scope(&self, body: &Value) -> Result<Value, String> {
        let focal = body
            .get("focal_plane")
            .and_then(|v| v.as_str())
            .unwrap_or("ffp");
        if !matches!(focal, "ffp" | "sfp") {
            return Err("focal plane must be ffp or sfp".into());
        }
        let (click_rad, click_text) = required_qty(body, "click", Dimension::Angle, "mrad")?;
        if click_rad <= 0.0 {
            return Err("click value must be positive".into());
        }
        let tracking = body
            .get("tracking_factor")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        if !(0.5..=1.5).contains(&tracking) {
            return Err("tracking factor must be between 0.5 and 1.5".into());
        }
        let mut record = json!({
            "label": required_text(body, "label")?,
            "make_model": text_or(body, "make_model"),
            "focal_plane": focal,
            "mag_min": body.get("mag_min").and_then(|v| v.as_f64()),
            "mag_max": body.get("mag_max").and_then(|v| v.as_f64()),
            "calibrated_mag": body.get("calibrated_mag").and_then(|v| v.as_f64()),
            "click_rad": click_rad,
            "click_text": click_text,
            "tracking_factor": tracking,
        });
        put_qty(
            &mut record,
            "travel_up",
            optional_qty(body, "travel_up", Dimension::Angle, "mrad")?,
        )?;
        put_qty(
            &mut record,
            "travel_down",
            optional_qty(body, "travel_down", Dimension::Angle, "mrad")?,
        )?;
        put_qty(
            &mut record,
            "travel_left",
            optional_qty(body, "travel_left", Dimension::Angle, "mrad")?,
        )?;
        put_qty(
            &mut record,
            "travel_right",
            optional_qty(body, "travel_right", Dimension::Angle, "mrad")?,
        )?;
        let id = self.write_record("scope", body, record)?;
        self.record_value("scope", &id)
    }

    pub fn save_bullet(&self, body: &Value) -> Result<Value, String> {
        let model = body
            .get("drag_model")
            .and_then(|v| v.as_str())
            .unwrap_or("G7")
            .to_ascii_uppercase();
        holdover_core::DragModel::parse(&model).map_err(|err| err.to_string())?;
        let reference = body
            .get("bc_reference")
            .and_then(|v| v.as_str())
            .unwrap_or("ICAO");
        if !matches!(reference, "ICAO" | "Army") {
            return Err("BC reference must be ICAO or Army".into());
        }
        let bc = body
            .get("bc")
            .and_then(|v| v.as_f64())
            .ok_or("a BC is required")?;
        if bc <= 0.0 {
            return Err("BC must be positive".into());
        }
        let mut warning = bc_warning(&model, bc);
        let mut bands = vec![json!({"bc": bc, "min_mps": 0.0, "min_text": "0 mps"})];
        if body.get("bc2").and_then(|v| v.as_f64()).is_some() {
            let bc2 = body["bc2"].as_f64().unwrap();
            if bc2 <= 0.0 {
                return Err("the second BC must be positive".into());
            }
            let (min_mps, min_text) = required_qty(body, "bc2_min", Dimension::Velocity, "mps")?;
            bands.push(json!({"bc": bc2, "min_mps": min_mps, "min_text": min_text}));
            if let Some(extra) = bc_warning(&model, bc2) {
                warning = Some(extra);
            }
        }
        let (diameter_m, diameter_text) = required_qty(body, "diameter", Dimension::Length, "in")?;
        let (weight_kg, weight_text) = required_qty(body, "weight", Dimension::Mass, "gr")?;
        let mut record = json!({
            "label": required_text(body, "label")?,
            "make_model": text_or(body, "make_model"),
            "diameter_m": diameter_m,
            "diameter_text": diameter_text,
            "weight_kg": weight_kg,
            "weight_text": weight_text,
            "drag_model": model,
            "bc_reference": reference,
            "source_note": text_or(body, "source_note"),
            "bands": bands,
        });
        put_qty(
            &mut record,
            "length",
            optional_qty(body, "length", Dimension::Length, "in")?,
        )?;
        let id = self.write_record("bullet", body, record)?;
        let mut value = self.record_value("bullet", &id)?;
        if let Some(warning) = warning {
            value["warning"] = json!(warning);
        }
        Ok(value)
    }

    pub fn save_load(&self, body: &Value) -> Result<Value, String> {
        let bullet_id = required_text(body, "bullet_id")?;
        self.require_live("bullet", &bullet_id)?;
        let (mv, mv_text) = required_qty(body, "mv", Dimension::Velocity, "mps")?;
        let mut record = json!({
            "label": required_text(body, "label")?,
            "bullet_id": bullet_id,
            "mv_mps": mv,
            "mv_text": mv_text,
            "source_note": text_or(body, "source_note"),
        });
        put_qty(
            &mut record,
            "mv_temperature",
            optional_qty(body, "mv_temperature", Dimension::Temperature, "C")?,
        )?;
        put_qty(
            &mut record,
            "sensitivity",
            optional_qty(body, "sensitivity", Dimension::Velocity, "mps")?,
        )?;
        let id = self.write_record("load", body, record)?;
        self.record_value("load", &id)
    }

    pub fn save_profile(&self, body: &Value) -> Result<Value, String> {
        let rifle_id = required_text(body, "rifle_id")?;
        let scope_id = required_text(body, "scope_id")?;
        let load_id = required_text(body, "load_id")?;
        self.require_live("rifle", &rifle_id)?;
        self.require_live("scope", &scope_id)?;
        self.require_live("load", &load_id)?;
        let (range_m, range_text) = required_qty(body, "zero_range", Dimension::Length, "m")?;
        let (offset_m, offset_text) = required_qty(body, "zero_offset", Dimension::Length, "cm")?;
        let air = parse_air(body, "zero_")?;
        let existing = self.profile_air_id(body.get("id").and_then(|v| v.as_str()).unwrap_or(""));
        let air_id = if let Some(id) = existing {
            let mut air_body = air_body(&air);
            air_body["label"] = json!("Zero atmosphere");
            self.write_record("atmosphere", &json!({"id": id}), air_body)?;
            id
        } else {
            let mut air_body = air_body(&air);
            air_body["label"] = json!("Zero atmosphere");
            self.write_record("atmosphere", &json!({}), air_body)?
        };
        let mut record = json!({
            "label": required_text(body, "label")?,
            "rifle_id": rifle_id,
            "scope_id": scope_id,
            "load_id": load_id,
            "zero_range_m": range_m,
            "zero_range_text": range_text,
            "zero_offset_m": offset_m,
            "zero_offset_text": offset_text,
            "zero_atmosphere_id": air_id,
        });
        put_qty(
            &mut record,
            "zero_powder",
            optional_qty(body, "zero_powder", Dimension::Temperature, "C")?,
        )?;
        if let Some(id) = body
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            if let Ok(existing) = self.body("profile", id) {
                if let Some(truing) = existing.get("truing") {
                    record["truing"] = truing.clone();
                }
            }
        }
        let id = self.write_record("profile", body, record)?;
        if self.active_id()?.is_empty() {
            self.set_active(&id)?;
        }
        self.profile_value(&id)
    }

    pub fn delete_record(&self, kind: &str, id: &str) -> Result<(), String> {
        self.ensure_unlinked(kind, id)?;
        let now = now_iso();
        let device = self.device_id()?;
        let changed = self
            .conn
            .execute(
                "UPDATE records SET deleted_at = ?1, updated_at = ?1, revision = revision + 1,
                    device_id = ?4
             WHERE id = ?2 AND kind = ?3 AND deleted_at IS NULL",
                params![now, id, kind, device],
            )
            .map_err(|err| err.to_string())?;
        if changed == 0 {
            return Err("record not found".into());
        }
        if kind == "profile" {
            self.conn
                .execute(
                    "UPDATE settings SET active_profile_id = '' WHERE active_profile_id = ?1",
                    params![id],
                )
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    /// Bring back a soft-deleted record. The body is unchanged.
    pub fn restore_record(&self, kind: &str, id: &str) -> Result<Value, String> {
        if kind == "history" {
            return Err("use restore history for an older version".into());
        }
        let now = now_iso();
        let device = self.device_id()?;
        let changed = self
            .conn
            .execute(
                "UPDATE records SET deleted_at = NULL, updated_at = ?1, revision = revision + 1,
                    device_id = ?4
             WHERE id = ?2 AND kind = ?3 AND deleted_at IS NOT NULL",
                params![now, id, kind, device],
            )
            .map_err(|err| err.to_string())?;
        if changed == 0 {
            return Err("archived record not found".into());
        }
        self.record_value(kind, id)
    }

    /// Write the snapshot back onto the live record. The current body is kept as history first.
    pub fn restore_history(&self, id: &str) -> Result<Value, String> {
        let row = self.body("history", id)?;
        let kind = row
            .get("subject_kind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let subject = row.get("subject_id").and_then(|v| v.as_str()).unwrap_or("");
        let snapshot = row.get("snapshot").cloned().unwrap_or(Value::Null);
        if kind.is_empty() || subject.is_empty() || !snapshot.is_object() {
            return Err("this history row has no snapshot".into());
        }
        if self.require_live(kind, subject).is_err() {
            if self.is_archived(kind, subject)? {
                return Err("restore the archived record before an older version".into());
            }
            return Err(format!("{kind} was not found"));
        }
        self.write_record(kind, &json!({ "id": subject }), snapshot)?;
        self.record_value(kind, subject)
    }

    pub fn save_load_log(&self, body: &Value) -> Result<Value, String> {
        let load_id = required_text(body, "load_id")?;
        self.require_live("load", &load_id)?;
        let mut record = json!({
            "label": required_text(body, "label")?,
            "load_id": load_id,
            "powder": text_or(body, "powder"),
            "powder_lot": text_or(body, "powder_lot"),
            "primer": text_or(body, "primer"),
            "primer_lot": text_or(body, "primer_lot"),
            "case_brand": text_or(body, "case_brand"),
            "case_lot": text_or(body, "case_lot"),
            "bullet_lot": text_or(body, "bullet_lot"),
            "notes": text_or(body, "notes"),
            "chrono_ids": self.live_id_list(body, "chrono_ids", "chrono")?,
            "document_ids": self.live_id_list(body, "document_ids", "document")?,
        });
        put_qty(
            &mut record,
            "charge",
            optional_qty(body, "charge", Dimension::Mass, "gr")?,
        )?;
        put_qty(
            &mut record,
            "coal",
            optional_qty(body, "coal", Dimension::Length, "mm")?,
        )?;
        let id = self.write_record("load_log", body, record)?;
        self.record_value("load_log", &id)
    }

    pub fn save_document(&self, body: &Value) -> Result<Value, String> {
        let title = required_text(body, "title")?;
        if title.chars().count() > 300 {
            return Err("the title is too long".into());
        }
        let link = text_or(body, "link_url");
        if !link.is_empty() && !is_http_url(&link) {
            return Err("a link has to start with http:// or https://".into());
        }
        let notes = text_or(body, "notes");
        if notes.chars().count() > 8000 {
            return Err("the notes are too long".into());
        }
        let existing_id = body
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let previous = if let Some(id) = &existing_id {
            Some(self.body("document", id)?)
        } else {
            None
        };
        let (sha, filename, bytes) = if let Some(file) = body.get("file").filter(|v| !v.is_null()) {
            let stored = self.store_pdf(file)?;
            (stored.0, stored.1, stored.2)
        } else if let Some(previous) = &previous {
            (
                previous
                    .get("sha256")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                previous
                    .get("filename")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                previous.get("bytes").and_then(|v| v.as_u64()).unwrap_or(0),
            )
        } else {
            (String::new(), String::new(), 0)
        };
        if link.is_empty() && sha.is_empty() {
            return Err("add a link or a PDF".into());
        }
        let load_id = optional_live(self, body, "load_id", "load")?;
        let rifle_id = optional_live(self, body, "rifle_id", "rifle")?;
        let bullet_id = optional_live(self, body, "bullet_id", "bullet")?;
        let record = json!({
            "label": title,
            "title": title,
            "edition": text_or(body, "edition"),
            "edition_date": text_or(body, "edition_date"),
            "notes": notes,
            "link_url": link,
            "sha256": sha,
            "filename": filename,
            "bytes": bytes,
            "load_id": load_id,
            "rifle_id": rifle_id,
            "bullet_id": bullet_id,
        });
        let id = self.write_record("document", body, record)?;
        self.record_value("document", &id)
    }

    /// Path or URL the shell can hand to the system opener. The bytes are never executed.
    pub fn document_target(&self, id: &str) -> Result<Value, String> {
        let doc = self.body("document", id)?;
        if let Some(hash) = doc
            .get("sha256")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let path = self.blob_path(hash)?;
            if !path.is_file() {
                return Err("the PDF is not on this computer".into());
            }
            let dest = std::env::temp_dir().join(format!("holdover-{}.pdf", &hash[..12]));
            fs::copy(&path, &dest).map_err(|err| err.to_string())?;
            return Ok(json!({
                "kind": "file",
                "path": dest.display().to_string(),
            }));
        }
        if let Some(url) = doc
            .get("link_url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            if !is_http_url(url) {
                return Err("a link has to start with http:// or https://".into());
            }
            return Ok(json!({ "kind": "link", "path": url }));
        }
        Err("this document has no file and no link".into())
    }

    pub fn set_active(&self, id: &str) -> Result<(), String> {
        if !id.is_empty() {
            self.require_live("profile", id)?;
        }
        self.conn
            .execute(
                "UPDATE settings SET active_profile_id = ?1 WHERE id = 1",
                params![id],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn remember_solve(&self, body: &Value) -> Result<(), String> {
        let air = self.merged_air(body.get("current_air").unwrap_or(&default_air()))?;
        self.conn
            .execute(
                "UPDATE settings SET current_air_json = ?1, effects_json = ?2, last_range = ?3,
             last_wind_speed = ?4, last_wind_direction = ?5, last_inclination = ?6,
             angular = ?7, active_profile_id = ?8, range_step = ?9, wind_step = ?10 WHERE id = 1",
                params![
                    air.to_string(),
                    body.get("effects")
                        .cloned()
                        .unwrap_or(default_effects())
                        .to_string(),
                    text_any(body, "range"),
                    text_any(body, "wind_speed"),
                    text_or(body, "wind_direction"),
                    text_any(body, "inclination"),
                    body.get("angular")
                        .and_then(|v| v.as_str())
                        .unwrap_or("mrad"),
                    text_or(body, "profile_id"),
                    {
                        let step = text_any(body, "range_step");
                        if step.is_empty() {
                            "10 m".into()
                        } else {
                            step
                        }
                    },
                    {
                        let step = text_any(body, "wind_step");
                        if step.is_empty() {
                            "1 mps".into()
                        } else {
                            step
                        }
                    }
                ],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn profile_bundle(&self, id: &str) -> Result<Value, String> {
        let profile = self.body("profile", id)?;
        let rifle = self.body("rifle", profile["rifle_id"].as_str().unwrap_or(""))?;
        let scope = self.body("scope", profile["scope_id"].as_str().unwrap_or(""))?;
        let load = self.body("load", profile["load_id"].as_str().unwrap_or(""))?;
        let bullet = self.body("bullet", load["bullet_id"].as_str().unwrap_or(""))?;
        let zero_air = self.body(
            "atmosphere",
            profile["zero_atmosphere_id"].as_str().unwrap_or(""),
        )?;
        Ok(json!({
            "profile": profile,
            "rifle": rifle,
            "scope": scope,
            "load": load,
            "bullet": bullet,
            "zero_air": zero_air,
        }))
    }

    pub fn export_library(&self) -> Result<Value, String> {
        self.export_library_with(false)
    }

    /// Library JSON. Rifle serials are cleared. Precise coordinates are omitted
    /// unless `include_location` is set. Sync fields on each record stay.
    pub fn export_library_with(&self, include_location: bool) -> Result<Value, String> {
        let mut settings = self.settings_json()?;
        if let Some(object) = settings.as_object_mut() {
            object.remove("onboarded");
            // Age is computed when settings are read. A backup must not freeze it.
            object.remove("weather_age_secs");
            object.remove("weather_stale");
            if !include_location {
                object.remove("weather_latitude");
                object.remove("weather_longitude");
            }
        }
        if !include_location {
            if let Some(air) = settings.get_mut("current_air") {
                strip_coordinates(air);
            }
        }
        let mut rifles = self.kind_export("rifle")?;
        for rifle in &mut rifles {
            if let Some(object) = rifle.as_object_mut() {
                object.insert("serial".into(), json!(""));
            }
        }
        let mut doc = json!({
            "schema_version": crate::library::SCHEMA_VERSION,
            "exported_at": now_iso(),
            "device_id": self.device_id()?,
            "settings": settings,
            "rifles": rifles,
        });
        for (key, kind) in crate::library::RECORD_SECTIONS.iter().skip(1) {
            doc[*key] = json!(self.kind_export(kind)?);
        }
        Ok(doc)
    }

    pub fn export_library_csv(&self, include_location: bool) -> Result<String, String> {
        crate::library::document_to_csv(&self.export_library_with(include_location)?)
    }

    pub fn import_library(&self, doc: &Value, commit: bool) -> Result<Value, String> {
        self.import_prepared(doc, commit, "replace", None)
    }

    pub fn import_prepared(
        &self,
        doc: &Value,
        commit: bool,
        policy: &str,
        abort_after: Option<usize>,
    ) -> Result<Value, String> {
        if policy != "replace" && policy != "merge" {
            return Err("policy".into());
        }
        crate::library::validate_document(doc)?;
        let preview = self.import_preview(doc, policy)?;
        if !commit {
            return Ok(preview);
        }
        self.apply_import(doc, policy, abort_after)?;
        let mut written = preview;
        if let Some(object) = written.as_object_mut() {
            object.insert("written".into(), json!(true));
        }
        Ok(written)
    }

    /// Read a Holdover CSV or JSON document. Nothing is written when `commit` is false.
    pub fn import_text(&self, text: &str, commit: bool, policy: &str) -> Result<Value, String> {
        let trimmed = text.trim_start();
        let doc = if trimmed.starts_with('{') {
            serde_json::from_str(trimmed).map_err(|_| "not-holdover".to_string())?
        } else {
            crate::library::csv_to_document(trimmed)?
        };
        self.import_prepared(&doc, commit, policy, None)
    }

    pub fn seed_example(&self) -> Result<String, String> {
        let rifle = self.save_rifle(&json!({
            "label": "Example .308 rifle",
            "make_model": "Example",
            "twist": "1:10in",
            "twist_hand": "right",
            "sight_height": {"text": "45 mm", "unit": "mm"}
        }))?;
        let scope = self.save_scope(&json!({
            "label": "Example scope",
            "focal_plane": "ffp",
            "click": {"text": "0.1 mrad", "unit": "mrad"},
            "tracking_factor": 1.0
        }))?;
        let bullet = self.save_bullet(&json!({
            "label": "175 gr .308 example",
            "diameter": {"text": "0.308 in", "unit": "in"},
            "weight": {"text": "175 gr", "unit": "gr"},
            "length": {"text": "1.24 in", "unit": "in"},
            "drag_model": "G7",
            "bc": 0.243,
            "bc_reference": "ICAO",
            "source_note": "examples/308-g7.json in this repository. Not a manufacturer catalogue."
        }))?;
        let load = self.save_load(&json!({
            "label": "Example 800 m/s",
            "bullet_id": bullet["id"].as_str().unwrap_or(""),
            "mv": {"text": "800 mps", "unit": "mps"},
            "source_note": "examples/308-g7.json"
        }))?;
        let profile = self.save_profile(&json!({
            "label": "Example .308",
            "rifle_id": rifle["id"].as_str().unwrap_or(""),
            "scope_id": scope["id"].as_str().unwrap_or(""),
            "load_id": load["id"].as_str().unwrap_or(""),
            "zero_range": {"text": "100 m", "unit": "m"},
            "zero_offset": {"text": "0 cm", "unit": "cm"},
            "zero_temperature": {"text": "15 C", "unit": "C"},
            "zero_pressure": {"text": "1013.25 hPa", "unit": "hPa"},
            "zero_pressure_kind": "station",
            "zero_humidity": "0%",
            "zero_altitude": {"text": "0 m", "unit": "m"}
        }))?;
        Ok(profile["id"].as_str().unwrap_or("").to_string())
    }

    fn settings_json(&self) -> Result<Value, String> {
        let mut value = self.conn.query_row(
            "SELECT onboarded, preset, units_json, appearance, large_text, reduced_motion, card_pinned,
                    angular, active_profile_id, current_air_json, effects_json, range_step, wind_step,
                    last_range, last_wind_speed, last_wind_direction, last_inclination
             FROM settings WHERE id = 1",
            [],
            |row| {
                let units_json: String = row.get(2)?;
                let air: String = row.get(9)?;
                let effects: String = row.get(10)?;
                let units: Value = serde_json::from_str(&units_json).unwrap_or(json!({}));
                let mut units = units;
                if let Some(object) = units.as_object_mut() {
                    let angular: String = row.get(7)?;
                    object.insert("angular".into(), json!(angular));
                }
                Ok(json!({
                    "onboarded": row.get::<_, i64>(0)? == 1,
                    "preset": row.get::<_, String>(1)?,
                    "units": units,
                    "appearance": row.get::<_, String>(3)?,
                    "large_text": row.get::<_, i64>(4)? == 1,
                    "reduced_motion": row.get::<_, i64>(5)? == 1,
                    "card_pinned": row.get::<_, i64>(6)? == 1,
                    "angular": row.get::<_, String>(7)?,
                    "active_profile_id": row.get::<_, String>(8)?,
                    "current_air": serde_json::from_str::<Value>(&air).unwrap_or(default_air()),
                    "effects": serde_json::from_str::<Value>(&effects).unwrap_or(default_effects()),
                    "range_step": row.get::<_, String>(11)?,
                    "wind_step": row.get::<_, String>(12)?,
                    "last_range": row.get::<_, String>(13)?,
                    "last_wind_speed": row.get::<_, String>(14)?,
                    "last_wind_direction": row.get::<_, String>(15)?,
                    "last_inclination": row.get::<_, String>(16)?,
                }))
            },
        ).map_err(|err| err.to_string())?;
        let place = self.weather_place()?;
        if let Some(object) = value.as_object_mut() {
            object.insert("weather_place".into(), json!(place.name));
            object.insert(
                "weather_latitude".into(),
                match place.latitude {
                    Some(latitude) => json!(latitude),
                    None => Value::Null,
                },
            );
            object.insert(
                "weather_longitude".into(),
                match place.longitude {
                    Some(longitude) => json!(longitude),
                    None => Value::Null,
                },
            );
            object.insert("weather_stale_minutes".into(), json!(place.stale_minutes));
        }
        if let Some(fetched) = value
            .pointer("/current_air/provenance/fetched_at")
            .and_then(|item| item.as_str())
        {
            if let Ok((age, stale)) = crate::weather::weather_age(
                fetched,
                &now_iso(),
                place.stale_minutes.saturating_mul(60),
            ) {
                if let Some(object) = value.as_object_mut() {
                    object.insert("weather_age_secs".into(), json!(age));
                    object.insert("weather_stale".into(), json!(stale));
                }
            }
        }
        Ok(value)
    }

    fn equipment_json(&self, units: &UnitSet) -> Result<Value, String> {
        Ok(json!({
            "rifles": self.list_kind("rifle", units)?,
            "scopes": self.list_kind("scope", units)?,
            "bullets": self.list_kind("bullet", units)?,
            "loads": self.list_kind("load", units)?,
            "profiles": self.list_profiles(units)?,
            "chronos": self.list_kind("chrono", units)?,
            "reticles": self.list_kind("reticle", units)?,
            "targets": self.list_kind("target", units)?,
            "documents": self.list_kind("document", units)?,
            "load_logs": self.list_kind("load_log", units)?,
            "groups": self.list_kind("group", units)?,
            "archived": self.list_archived()?,
            "history": self.history_json()?,
        }))
    }

    fn list_kind(&self, kind: &str, units: &UnitSet) -> Result<Vec<Value>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, body, revision FROM records WHERE kind = ?1 AND deleted_at IS NULL",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![kind], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            let (id, body, revision) = row.map_err(|err| err.to_string())?;
            let mut value: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
            stamp(&mut value, &id, kind, revision);
            decorate(&mut value, units);
            out.push(value);
        }
        out.sort_by(|left, right| {
            left["label"]
                .as_str()
                .unwrap_or("")
                .cmp(right["label"].as_str().unwrap_or(""))
        });
        Ok(out)
    }

    fn list_profiles(&self, units: &UnitSet) -> Result<Vec<Value>, String> {
        let mut profiles = self.list_kind("profile", units)?;
        for profile in &mut profiles {
            let id = profile["id"].as_str().unwrap_or("").to_string();
            if let Ok(bundle) = self.profile_bundle(&id) {
                profile["zero_air"] = bundle["zero_air"].clone();
                decorate_air(profile.get_mut("zero_air").unwrap_or(&mut json!({})), units);
                let rifle = bundle["rifle"]["label"].as_str().unwrap_or("");
                let load = bundle["load"]["label"].as_str().unwrap_or("");
                profile["summary"] = json!(format!(
                    "{} · {} {} · {}",
                    profile["label"].as_str().unwrap_or("Profile"),
                    profile["zero_range_display"].as_str().unwrap_or(""),
                    units.length,
                    rifle
                ));
                profile["load_label"] = json!(load);
                profile["rifle_label"] = json!(rifle);
            }
        }
        Ok(profiles)
    }

    fn profile_value(&self, id: &str) -> Result<Value, String> {
        let units = self.units()?;
        let profiles = self.list_profiles(&units)?;
        profiles
            .into_iter()
            .find(|profile| profile["id"] == id)
            .ok_or_else(|| "profile was not found".into())
    }

    pub(crate) fn record_value(&self, kind: &str, id: &str) -> Result<Value, String> {
        let units = self.units()?;
        let rows = self.list_kind(kind, &units)?;
        rows.into_iter()
            .find(|row| row["id"] == id)
            .ok_or_else(|| format!("{kind} was not found"))
    }

    pub(crate) fn body(&self, kind: &str, id: &str) -> Result<Value, String> {
        let (raw, revision): (String, i64) = self.conn.query_row(
            "SELECT body, revision FROM records WHERE id = ?1 AND kind = ?2 AND deleted_at IS NULL",
            params![id, kind],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|_| format!("{kind} {id} was not found"))?;
        let mut value: Value = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
        stamp(&mut value, id, kind, revision);
        Ok(value)
    }

    pub(crate) fn write_record(
        &self,
        kind: &str,
        request: &Value,
        body: Value,
    ) -> Result<String, String> {
        let now = now_iso();
        if let Some(id) = request
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            if kind != "history" {
                self.snapshot_previous(kind, id, &body)?;
            }
            let device = self.device_id()?;
            let changed = self
                .conn
                .execute(
                    "UPDATE records SET body = ?1, updated_at = ?2, revision = revision + 1,
                    device_id = ?5
                 WHERE id = ?3 AND kind = ?4 AND deleted_at IS NULL",
                    params![body.to_string(), now, id, kind, device],
                )
                .map_err(|err| err.to_string())?;
            if changed == 0 {
                return Err(format!("{kind} was not found"));
            }
            return Ok(id.to_string());
        }
        self.insert_record(kind, &body)
    }

    fn insert_record(&self, kind: &str, body: &Value) -> Result<String, String> {
        let now = now_iso();
        let device = self.device_id()?;
        let id = Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO records (id, kind, created_at, updated_at, revision, deleted_at, device_id, body)
                 VALUES (?1, ?2, ?3, ?3, 1, NULL, ?4, ?5)",
                params![id, kind, now, device, body.to_string()],
            )
            .map_err(|err| err.to_string())?;
        Ok(id)
    }

    fn snapshot_previous(&self, kind: &str, id: &str, incoming: &Value) -> Result<(), String> {
        let found: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT body, revision FROM records WHERE id = ?1 AND kind = ?2 AND deleted_at IS NULL",
                params![id, kind],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|err| err.to_string())?;
        let Some((raw, revision)) = found else {
            return Ok(());
        };
        let old: Value = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
        if &old == incoming {
            return Ok(());
        }
        let label = old
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let snap = json!({
            "label": format!("{kind} {label}"),
            "subject_kind": kind,
            "subject_id": id,
            "subject_revision": revision,
            "subject_label": label,
            "snapshot": old,
        });
        self.insert_record("history", &snap)?;
        Ok(())
    }

    fn is_archived(&self, kind: &str, id: &str) -> Result<bool, String> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM records WHERE id = ?1 AND kind = ?2 AND deleted_at IS NOT NULL",
                params![id, kind],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())?;
        Ok(found.is_some())
    }

    fn list_archived(&self) -> Result<Vec<Value>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, kind, body, revision, deleted_at FROM records
             WHERE deleted_at IS NOT NULL AND kind != 'history'
             ORDER BY deleted_at DESC",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            let (id, kind, body, revision, deleted_at) = row.map_err(|err| err.to_string())?;
            let value: Value = serde_json::from_str(&body).unwrap_or(json!({}));
            out.push(json!({
                "id": id,
                "kind": kind,
                "label": value.get("label").and_then(|v| v.as_str()).unwrap_or(""),
                "revision": revision,
                "deleted_at": deleted_at,
            }));
        }
        Ok(out)
    }

    fn history_json(&self) -> Result<Value, String> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM records WHERE kind = 'history' AND deleted_at IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())?;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, created_at, body FROM records
             WHERE kind = 'history' AND deleted_at IS NULL
             ORDER BY created_at DESC LIMIT 100",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            let (id, created_at, body) = row.map_err(|err| err.to_string())?;
            let value: Value = serde_json::from_str(&body).unwrap_or(json!({}));
            out.push(json!({
                "id": id,
                "created_at": created_at,
                "subject_kind": value.get("subject_kind").cloned().unwrap_or(Value::Null),
                "subject_id": value.get("subject_id").cloned().unwrap_or(Value::Null),
                "subject_label": value.get("subject_label").cloned().unwrap_or(Value::Null),
                "subject_revision": value.get("subject_revision").cloned().unwrap_or(Value::Null),
            }));
        }
        Ok(json!({ "count": count, "rows": out }))
    }

    fn live_id_list(&self, body: &Value, key: &str, kind: &str) -> Result<Vec<String>, String> {
        let Some(items) = body.get(key).and_then(|v| v.as_array()) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for item in items {
            let id = item.as_str().unwrap_or("").trim();
            if id.is_empty() {
                continue;
            }
            self.require_live(kind, id)?;
            if !out.iter().any(|existing: &String| existing == id) {
                out.push(id.to_string());
            }
        }
        Ok(out)
    }

    fn store_pdf(&self, file: &Value) -> Result<(String, String, u64), String> {
        let filename = pdf_filename(
            file.get("filename")
                .and_then(|v| v.as_str())
                .unwrap_or("document.pdf"),
        )?;
        let encoded = file
            .get("base64")
            .and_then(|v| v.as_str())
            .ok_or("the PDF data is missing")?;
        let bytes = decode_base64(encoded)?;
        if bytes.is_empty() {
            return Err("the PDF is empty".into());
        }
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("a PDF has to be 32 MB or smaller".into());
        }
        if !bytes.starts_with(b"%PDF") {
            return Err("Holdover stores a PDF, and this file is not one".into());
        }
        let hash = self.write_blob(&bytes)?;
        Ok((hash, filename, bytes.len() as u64))
    }

    pub(crate) fn read_blob(&self, hash: &str) -> Result<Vec<u8>, String> {
        let path = self.blob_path(hash)?;
        fs::read(&path).map_err(|_| "bundle-missing-pdf".to_string())
    }

    pub(crate) fn put_blob(&self, bytes: &[u8]) -> Result<String, String> {
        self.write_blob(bytes)
    }

    fn write_blob(&self, bytes: &[u8]) -> Result<String, String> {
        let hash = hex_encode(&Sha256::digest(bytes));
        let dir = self.blob_dir();
        fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let path = self.blob_path(&hash)?;
        if !path.is_file() {
            let tmp = dir.join(format!(".{hash}.tmp"));
            fs::write(&tmp, bytes).map_err(|err| err.to_string())?;
            fs::rename(&tmp, &path).map_err(|err| err.to_string())?;
        }
        Ok(hash)
    }

    fn blob_dir(&self) -> PathBuf {
        let mut name = self
            .db_path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "holdover".into());
        name.push_str(".blobs");
        self.db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(name)
    }

    fn blob_path(&self, hash: &str) -> Result<PathBuf, String> {
        if hash.len() != 64 || !hash.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err("the stored file name is not a hash".into());
        }
        Ok(self.blob_dir().join(hash))
    }

    pub(crate) fn require_live(&self, kind: &str, id: &str) -> Result<(), String> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM records WHERE id = ?1 AND kind = ?2 AND deleted_at IS NULL",
                params![id, kind],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())?;
        if found.is_none() {
            return Err(format!("{kind} was not found"));
        }
        Ok(())
    }

    fn ensure_unlinked(&self, kind: &str, id: &str) -> Result<(), String> {
        let (sql, message): (&str, &str) = match kind {
            "rifle" => ("SELECT COUNT(*) FROM records WHERE kind = 'profile' AND deleted_at IS NULL AND json_extract(body, '$.rifle_id') = ?1", "still used by a profile"),
            "scope" => ("SELECT COUNT(*) FROM records WHERE kind = 'profile' AND deleted_at IS NULL AND json_extract(body, '$.scope_id') = ?1", "still used by a profile"),
            "bullet" => ("SELECT COUNT(*) FROM records WHERE kind = 'load' AND deleted_at IS NULL AND json_extract(body, '$.bullet_id') = ?1", "still used by a load"),
            "load" => ("SELECT COUNT(*) FROM records WHERE kind = 'profile' AND deleted_at IS NULL AND json_extract(body, '$.load_id') = ?1", "still used by a profile"),
            _ => return Ok(()),
        };
        let count: i64 = self
            .conn
            .query_row(sql, params![id], |row| row.get(0))
            .map_err(|err| err.to_string())?;
        if count > 0 {
            return Err(message.into());
        }
        Ok(())
    }

    fn profile_air_id(&self, id: &str) -> Option<String> {
        if id.is_empty() {
            return None;
        }
        self.body("profile", id)
            .ok()
            .and_then(|body| body["zero_atmosphere_id"].as_str().map(|s| s.to_string()))
    }

    fn active_id(&self) -> Result<String, String> {
        self.conn
            .query_row(
                "SELECT active_profile_id FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())
    }

    fn flag(&self, _name: &str) -> Result<bool, String> {
        let onboarded: i64 = self
            .conn
            .query_row("SELECT onboarded FROM settings WHERE id = 1", [], |row| {
                row.get(0)
            })
            .map_err(|err| err.to_string())?;
        Ok(onboarded == 1)
    }

    fn kind_export(&self, kind: &str) -> Result<Vec<Value>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, created_at, updated_at, revision, deleted_at, device_id, body
             FROM records WHERE kind = ?1 ORDER BY id",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![kind], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            let (id, created, updated, revision, deleted, device, body) =
                row.map_err(|err| err.to_string())?;
            let mut value: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
            if let Some(object) = value.as_object_mut() {
                object.insert("id".into(), json!(id));
                object.insert("created_at".into(), json!(created));
                object.insert("updated_at".into(), json!(updated));
                object.insert("revision".into(), json!(revision));
                object.insert(
                    "deleted_at".into(),
                    match deleted {
                        Some(text) => json!(text),
                        None => Value::Null,
                    },
                );
                object.insert("device_id".into(), json!(device));
            }
            out.push(value);
        }
        Ok(out)
    }

    pub fn mutate_body(
        &self,
        kind: &str,
        id: &str,
        mutate: impl FnOnce(&mut serde_json::Map<String, Value>) -> Result<(), String>,
    ) -> Result<(), String> {
        let raw: String = self
            .conn
            .query_row(
                "SELECT body FROM records WHERE id = ?1 AND kind = ?2 AND deleted_at IS NULL",
                params![id, kind],
                |row| row.get(0),
            )
            .map_err(|_| format!("{kind} {id} was not found"))?;
        let mut value: Value = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
        {
            let object = value.as_object_mut().ok_or("record")?;
            mutate(object)?;
        }
        self.write_record(kind, &json!({ "id": id }), value)?;
        Ok(())
    }

    pub fn save_reticle(&self, body: &Value) -> Result<Value, String> {
        let mut reticle = holdover_core::parse_reticle(body).map_err(|err| err.to_string())?;
        reticle.licence = "user-drawn".into();
        if reticle.source.trim().is_empty() {
            reticle.source = "User drawn in Holdover.".into();
        }
        let mut request = body.clone();
        if holdover_core::generic_by_id(&reticle.id).is_some() {
            if let Some(object) = request.as_object_mut() {
                object.remove("id");
            }
        }
        reticle.id = request
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let mut stored = holdover_core::reticle_json(&reticle);
        if let Some(object) = stored.as_object_mut() {
            object.remove("id");
        }
        let id = self.write_record("reticle", &request, stored)?;
        self.record_value("reticle", &id)
    }

    pub fn save_target_record(&self, body: &Value) -> Result<Value, String> {
        let doc = holdover_core::apply_target_edit(body).map_err(|err| err.to_string())?;
        let mut request = json!({});
        if let Some(id) = body.get("id").and_then(|value| value.as_str()) {
            if holdover_core::target_by_id(id).is_none() {
                request["id"] = json!(id);
            }
        }
        let mut stored = holdover_core::target_json(&doc);
        if let Some(object) = stored.as_object_mut() {
            object.insert(
                "base_id".into(),
                json!(body.get("base_id").and_then(|v| v.as_str()).unwrap_or("")),
            );
            object.remove("id");
        }
        let id = self.write_record("target", &request, stored)?;
        self.record_value("target", &id)
    }

    pub fn saved_target(&self, id: &str) -> Result<Value, String> {
        self.body("target", id)
    }

    pub fn save_chrono_record(&self, body: &Value) -> Result<String, String> {
        let load_id = body
            .get("load_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        self.require_live("load", load_id)?;
        self.write_record("chrono", &json!({}), body.clone())
    }

    pub fn live_bodies(&self, kind: &str) -> Result<Vec<Value>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, body FROM records WHERE kind = ?1 AND deleted_at IS NULL")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![kind], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            let (id, raw) = row.map_err(|err| err.to_string())?;
            let mut value: Value = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
            if let Some(object) = value.as_object_mut() {
                object.insert("id".into(), json!(id));
            }
            out.push(value);
        }
        Ok(out)
    }

    fn import_preview(&self, doc: &Value, policy: &str) -> Result<Value, String> {
        let mut counts = crate::library::empty_counts();
        let mut duplicates = Vec::new();
        let mut incoming = Vec::new();
        for (key, kind) in crate::library::RECORD_SECTIONS {
            let items = doc
                .get(*key)
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if let Some(count) = counts.get_mut(*key) {
                *count = json!(items.len());
            }
            for item in items {
                let id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                incoming.push(id.clone());
                let live = self.record_is_live(&id)?;
                if live && item.get("deleted_at").and_then(|v| v.as_str()).is_none() {
                    duplicates.push(json!({
                        "kind": kind,
                        "id": id,
                        "label": item.get("label").and_then(|v| v.as_str()).unwrap_or(""),
                    }));
                }
            }
        }
        let local_only = self
            .live_ids()?
            .into_iter()
            .filter(|id| !incoming.contains(id))
            .count();
        Ok(json!({
            "written": false,
            "schema_version": doc.get("schema_version").and_then(|v| v.as_i64()).unwrap_or(0),
            "policy": policy,
            "counts": counts,
            "duplicates": duplicates,
            "local_only": local_only,
            "coordinates_omitted": doc
                .pointer("/settings/weather_latitude")
                .and_then(|v| v.as_f64())
                .is_none(),
        }))
    }

    fn apply_import(
        &self,
        doc: &Value,
        policy: &str,
        abort_after: Option<usize>,
    ) -> Result<(), String> {
        let local_device = self.device_id()?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|err| err.to_string())?;
        if policy == "replace" {
            tx.execute("DELETE FROM records", [])
                .map_err(|err| err.to_string())?;
        }
        let mut written = 0usize;
        for (key, kind) in crate::library::RECORD_SECTIONS {
            let Some(items) = doc.get(*key).and_then(|v| v.as_array()) else {
                continue;
            };
            for item in items {
                if abort_after == Some(written) {
                    return Err("aborted".into());
                }
                if policy == "merge" {
                    let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    tx.execute("DELETE FROM records WHERE id = ?1", params![id])
                        .map_err(|err| err.to_string())?;
                }
                insert_raw(&tx, kind, item, &local_device)?;
                written += 1;
            }
        }
        if let Some(settings) = doc.get("settings") {
            if abort_after == Some(written) {
                return Err("aborted".into());
            }
            apply_settings(&tx, settings)?;
            apply_weather_meta(&tx, settings)?;
        }
        tx.commit().map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn apply_weather(&self, body: &Value) -> Result<(), String> {
        if bool_or(body, "accept_wind", false) {
            let speed = body
                .get("wind_speed_mps")
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite())
                .ok_or("wind")?;
            let units = self.units()?;
            let text = quantity_text(speed, &units.velocity);
            self.conn
                .execute(
                    "UPDATE settings SET last_wind_speed = ?1 WHERE id = 1",
                    params![text],
                )
                .map_err(|err| err.to_string())?;
            let mut air = self.current_air_raw()?;
            if let Some(provenance) = air.get_mut("provenance").and_then(|v| v.as_object_mut()) {
                provenance.insert("wind_accepted".into(), json!(true));
                provenance.insert("wind_speed_mps".into(), json!(speed));
            }
            self.write_current_air(&air)?;
            return Ok(());
        }
        let temperature_c = body
            .get("temperature_c")
            .and_then(|v| v.as_f64())
            .ok_or("temperature")?;
        let humidity = body
            .get("humidity")
            .and_then(|v| v.as_f64())
            .ok_or("humidity")?;
        let elevation_m = body
            .get("elevation_m")
            .and_then(|v| v.as_f64())
            .ok_or("elevation")?;
        let surface = body
            .get("surface_pressure_pa")
            .and_then(|v| v.as_f64())
            .ok_or("surface_pressure")?;
        let sea = body
            .get("sea_level_pressure_pa")
            .and_then(|v| v.as_f64())
            .ok_or("pressure_msl")?;
        let use_sea = body.get("pressure_used").and_then(|v| v.as_str()) == Some("sea-level");
        let units = self.units()?;
        let pressure_pa = if use_sea { sea } else { surface };
        let kind = if use_sea { "sea-level" } else { "station" };
        let field = if use_sea {
            "pressure_msl"
        } else {
            "surface_pressure"
        };
        let humidity_text = format!("{}%", format_number(humidity * 100.0));
        let air = json!({
            "temperature": {"text": quantity_text(temperature_c, &units.temperature), "unit": units.temperature},
            "pressure": {"text": quantity_text(pressure_pa, &units.pressure), "unit": units.pressure},
            "pressure_kind": kind,
            "humidity": humidity_text,
            "altitude": {"text": quantity_text(elevation_m, &units.length), "unit": units.length},
            "provenance": {
                "source": "fetched",
                "provider": "open-meteo",
                "fetched_at": text_or(body, "fetched_at"),
                "valid_time": text_or(body, "valid_time"),
                "pressure_used": kind,
                "pressure_field": field,
                "surface_pressure_pa": surface,
                "sea_level_pressure_pa": sea,
                "elevation_m": elevation_m,
                "grid_latitude": body.get("grid_latitude").cloned().unwrap_or(Value::Null),
                "grid_longitude": body.get("grid_longitude").cloned().unwrap_or(Value::Null),
                "place_name": text_or(body, "place_name"),
                "wind_speed_mps": body.get("wind_speed_mps").cloned().unwrap_or(Value::Null),
                "wind_from_deg": body.get("wind_from_deg").cloned().unwrap_or(Value::Null),
                "wind_gust_mps": body.get("wind_gust_mps").cloned().unwrap_or(Value::Null),
                "wind_accepted": false
            }
        });
        atmosphere_from_form(&air)?;
        self.write_current_air(&air)?;
        Ok(())
    }

    fn merged_air(&self, incoming: &Value) -> Result<Value, String> {
        let mut air = incoming.clone();
        if air.get("provenance").is_some() {
            return Ok(air);
        }
        let previous = self.current_air_raw().unwrap_or_else(|_| default_air());
        if same_air(&previous, &air) {
            if let Some(provenance) = previous.get("provenance") {
                air["provenance"] = provenance.clone();
            }
        } else {
            air["provenance"] = json!({ "source": "manual" });
        }
        Ok(air)
    }

    pub(crate) fn current_air_raw(&self) -> Result<Value, String> {
        let raw: String = self
            .conn
            .query_row(
                "SELECT current_air_json FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())?;
        serde_json::from_str(&raw).map_err(|err| err.to_string())
    }

    fn write_current_air(&self, air: &Value) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE settings SET current_air_json = ?1 WHERE id = 1",
                params![air.to_string()],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn meta_get(&self, key: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())
    }

    fn meta_put(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn meta_del(&self, key: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM meta WHERE key = ?1", params![key])
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn record_is_live(&self, id: &str) -> Result<bool, String> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM records WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())?;
        Ok(found.is_some())
    }

    fn live_ids(&self) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM records WHERE deleted_at IS NULL")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|err| err.to_string())?);
        }
        Ok(out)
    }
}

struct ParsedAir {
    temperature_c: f64,
    temperature_text: String,
    pressure_pa: f64,
    pressure_text: String,
    pressure_kind: String,
    humidity: f64,
    humidity_text: String,
    altitude_m: f64,
    altitude_text: String,
}

fn air_body(air: &ParsedAir) -> Value {
    json!({
        "temperature_c": air.temperature_c,
        "temperature_text": air.temperature_text,
        "pressure_pa": air.pressure_pa,
        "pressure_text": air.pressure_text,
        "pressure_kind": air.pressure_kind,
        "humidity": air.humidity,
        "humidity_text": air.humidity_text,
        "altitude_m": air.altitude_m,
        "altitude_text": air.altitude_text,
        "source": "manual",
    })
}

fn parse_air(body: &Value, prefix: &str) -> Result<ParsedAir, String> {
    let (temperature_c, temperature_text) = required_qty(
        body,
        &format!("{prefix}temperature"),
        Dimension::Temperature,
        "C",
    )?;
    let (pressure_pa, pressure_text) = required_qty(
        body,
        &format!("{prefix}pressure"),
        Dimension::Pressure,
        "hPa",
    )?;
    let (altitude_m, altitude_text) =
        required_qty(body, &format!("{prefix}altitude"), Dimension::Length, "m")?;
    let kind = body
        .get(format!("{prefix}pressure_kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("station");
    if !matches!(kind, "station" | "sea-level") {
        return Err("pressure kind must be station or sea-level".into());
    }
    let humidity_text = body
        .get(format!("{prefix}humidity"))
        .and_then(|v| v.as_str())
        .unwrap_or("0%")
        .to_string();
    let (humidity, humidity_text) = parse_humidity(&humidity_text)?;
    Ok(ParsedAir {
        temperature_c,
        temperature_text,
        pressure_pa,
        pressure_text,
        pressure_kind: kind.into(),
        humidity,
        humidity_text,
        altitude_m,
        altitude_text,
    })
}

fn quantity_text(si: f64, unit: &str) -> String {
    match format_quantity(si, unit) {
        Ok(number) => format!("{number} {unit}"),
        Err(_) => format!("{} {unit}", format_number(si)),
    }
}

fn same_air(left: &Value, right: &Value) -> bool {
    let Ok(before) = atmosphere_from_form(left) else {
        return false;
    };
    let Ok(after) = atmosphere_from_form(right) else {
        return false;
    };
    (before.temperature_c - after.temperature_c).abs() < 0.05
        && (before.pressure_pa - after.pressure_pa).abs() < 50.0
        && (before.altitude_m - after.altitude_m).abs() < 0.5
        && (before.humidity - after.humidity).abs() < 0.005
        && before.pressure_kind == after.pressure_kind
}

fn strip_coordinates(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for key in ["grid_latitude", "grid_longitude", "latitude", "longitude"] {
        object.remove(key);
    }
    if let Some(provenance) = object.get_mut("provenance") {
        if let Some(nested) = provenance.as_object_mut() {
            for key in ["grid_latitude", "grid_longitude", "latitude", "longitude"] {
                nested.remove(key);
            }
        }
    }
}

fn coord_text(body: &Value, key: &str) -> String {
    match body.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => String::new(),
    }
}

fn parse_coord(text: &str, min: f64, max: f64) -> Result<Option<f64>, ()> {
    let trimmed = text.trim().replace(',', ".");
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value: f64 = trimmed.parse().map_err(|_| ())?;
    if !value.is_finite() || !(min..=max).contains(&value) {
        return Err(());
    }
    Ok(Some(value))
}

fn stale_minutes(value: Option<&Value>) -> Result<u64, String> {
    let Some(value) = value else {
        return Ok(60);
    };
    let minutes = match value {
        Value::Number(number) => number
            .as_u64()
            .or_else(|| number.as_f64().map(|v| v as u64)),
        Value::String(text) => text
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .map(|v| v as u64),
        _ => None,
    };
    match minutes {
        Some(minutes) if (1..=7 * 24 * 60).contains(&minutes) => Ok(minutes),
        _ => Err("place-stale".into()),
    }
}

fn default_air() -> Value {
    json!({
        "temperature": {"text": "15 C", "unit": "C"},
        "pressure": {"text": "1013.25 hPa", "unit": "hPa"},
        "pressure_kind": "station",
        "humidity": "0%",
        "altitude": {"text": "0 m", "unit": "m"}
    })
}

fn default_effects() -> Value {
    json!({
        "incline": true,
        "spin_drift": true,
        "coriolis": false,
        "cant": false,
        "aerodynamic_jump": false
    })
}

fn put_required(record: &mut Value, name: &str, qty: (f64, String)) -> Result<(), String> {
    put_qty(record, name, Some(qty))
}

fn put_qty(record: &mut Value, name: &str, qty: Option<(f64, String)>) -> Result<(), String> {
    let object = record.as_object_mut().ok_or("record")?;
    match qty {
        Some((si, text)) => {
            object.insert(
                format!("{name}_m").replace("sight_height_m", "sight_height_m"),
                json!(si),
            );
            object.insert(format!("{name}_si"), json!(si));
            object.insert(format!("{name}_text"), json!(text));
        }
        None => {
            object.insert(format!("{name}_si"), Value::Null);
            object.insert(format!("{name}_text"), json!(""));
        }
    }
    Ok(())
}

pub(crate) fn stamp(value: &mut Value, id: &str, kind: &str, revision: i64) {
    if let Some(object) = value.as_object_mut() {
        object.insert("id".into(), json!(id));
        object.insert("kind".into(), json!(kind));
        object.insert("revision".into(), json!(revision));
    }
}

fn decorate(value: &mut Value, units: &UnitSet) {
    copy_si(value, "mv_mps", "mv_si");
    copy_si(value, "click_rad", "click_si");
    copy_si(value, "zero_range_m", "zero_range_si");
    copy_si(value, "zero_offset_m", "zero_offset_si");
    copy_si(value, "diameter_m", "diameter_si");
    copy_si(value, "weight_kg", "weight_si");
    copy_si(value, "length_m", "length_si");
    copy_si(value, "barrel_length_m", "barrel_length_si");
    copy_si(value, "sight_height_m", "sight_height_si");
    let pairs = [
        ("sight_height", &units.small),
        ("barrel_length", &units.small),
        ("weight", &units.mass),
        ("diameter", &units.small),
        ("length", &units.small),
        ("mv", &units.velocity),
        ("mv_temperature", &units.temperature),
        ("sensitivity", &units.velocity),
        ("zero_range", &units.length),
        ("zero_offset", &units.small),
        ("zero_powder", &units.temperature),
        ("click", &units.angle),
        ("travel_up", &units.angle),
        ("travel_down", &units.angle),
        ("travel_left", &units.angle),
        ("travel_right", &units.angle),
        ("temperature", &units.temperature),
        ("pressure", &units.pressure),
        ("altitude", &units.length),
        ("charge", &units.mass),
        ("coal", &units.small),
    ];
    for (name, unit) in pairs {
        let si = value.get(format!("{name}_si")).and_then(|v| v.as_f64());
        if let Some(si) = si {
            value[format!("{name}_display")] = json!(shown(si, unit));
            value[format!("{name}_unit")] = json!(unit);
        }
    }
    decorate_air(value, units);
}

fn copy_si(value: &mut Value, from: &str, to: &str) {
    if value.get(to).and_then(|v| v.as_f64()).is_none() {
        if let Some(si) = value.get(from).and_then(|v| v.as_f64()) {
            value[to] = json!(si);
        }
    }
}

pub(crate) fn atmosphere_from_form(value: &Value) -> Result<holdover_core::Atmosphere, String> {
    let wrapped = json!({
        "zero_temperature": value.get("temperature").cloned().unwrap_or(Value::Null),
        "zero_pressure": value.get("pressure").cloned().unwrap_or(Value::Null),
        "zero_pressure_kind": value.get("pressure_kind").cloned().unwrap_or(json!("station")),
        "zero_humidity": value.get("humidity").cloned().unwrap_or(json!("0%")),
        "zero_altitude": value.get("altitude").cloned().unwrap_or(Value::Null),
    });
    let air = parse_air(&wrapped, "zero_")?;
    stored_atmosphere(&air_body(&air))
}

pub(crate) fn stored_atmosphere(value: &Value) -> Result<holdover_core::Atmosphere, String> {
    let kind = value
        .get("pressure_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("station");
    Ok(holdover_core::Atmosphere {
        temperature_c: value
            .get("temperature_c")
            .and_then(|v| v.as_f64())
            .ok_or("temperature is missing")?,
        pressure_pa: value
            .get("pressure_pa")
            .and_then(|v| v.as_f64())
            .ok_or("pressure is missing")?,
        pressure_kind: if kind == "sea-level" {
            holdover_core::PressureKind::SeaLevel
        } else {
            holdover_core::PressureKind::Station
        },
        humidity: value
            .get("humidity")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        altitude_m: value
            .get("altitude_m")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
    })
}

fn decorate_settings(value: &mut Value, units: &UnitSet) {
    decorate_text_setting(value, "last_range", Dimension::Length, &units.length);
    decorate_text_setting(
        value,
        "last_wind_speed",
        Dimension::Velocity,
        &units.velocity,
    );
    decorate_text_setting(value, "last_inclination", Dimension::Angle, "deg");
    if let Some(air) = value.get_mut("current_air") {
        decorate_form_quantity(
            air,
            "temperature",
            Dimension::Temperature,
            &units.temperature,
        );
        decorate_form_quantity(air, "pressure", Dimension::Pressure, &units.pressure);
        decorate_form_quantity(air, "altitude", Dimension::Length, &units.length);
    }
}

fn decorate_text_setting(value: &mut Value, key: &str, dim: Dimension, unit: &str) {
    let Some(text) = value.get(key).and_then(|v| v.as_str()).map(str::to_string) else {
        return;
    };
    if let Ok(si) = parse_quantity(&text, dim, Some(unit)) {
        value[format!("{key}_si")] = json!(si);
        value[format!("{key}_display")] = json!(shown(si, unit));
        value[format!("{key}_unit")] = json!(unit);
    }
}

fn decorate_form_quantity(air: &mut Value, key: &str, dim: Dimension, unit: &str) {
    let Some(node) = air.get_mut(key) else {
        return;
    };
    let (text, fallback) = match &node {
        Value::String(text) => (text.clone(), unit.to_string()),
        Value::Object(map) => (
            map.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            map.get("unit")
                .and_then(|v| v.as_str())
                .unwrap_or(unit)
                .to_string(),
        ),
        _ => return,
    };
    if text.trim().is_empty() {
        return;
    }
    let Ok(si) = parse_quantity(&text, dim, Some(&fallback)) else {
        return;
    };
    let display = shown(si, unit);
    match node {
        Value::Object(map) => {
            map.insert("si".into(), json!(si));
            map.insert("display".into(), json!(display));
            map.insert("display_unit".into(), json!(unit));
        }
        other => {
            *other = json!({
                "text": text,
                "unit": fallback,
                "si": si,
                "display": display,
                "display_unit": unit,
            });
        }
    }
}

fn decorate_air(value: &mut Value, units: &UnitSet) {
    if let Some(si) = value.get("temperature_c").and_then(|v| v.as_f64()) {
        value["temperature_display"] = json!(shown(si, &units.temperature));
        value["temperature_unit"] = json!(units.temperature);
    }
    if let Some(si) = value.get("pressure_pa").and_then(|v| v.as_f64()) {
        value["pressure_display"] = json!(shown(si, &units.pressure));
        value["pressure_unit"] = json!(units.pressure);
    }
    if let Some(si) = value.get("altitude_m").and_then(|v| v.as_f64()) {
        value["altitude_display"] = json!(shown(si, &units.length));
        value["altitude_unit"] = json!(units.length);
    }
    if let Some(humidity) = value.get("humidity").and_then(|v| v.as_f64()) {
        value["humidity_display"] = json!(shown(humidity * 100.0, "%"));
    }
}

fn shown(si: f64, unit: &str) -> String {
    if unit == "%" {
        return format_number(si);
    }
    holdover_core::format_quantity(si, unit).unwrap_or_else(|_| format_number(si))
}

fn insert_raw(
    tx: &rusqlite::Transaction<'_>,
    kind: &str,
    item: &Value,
    device: &str,
) -> Result<(), String> {
    let id = item
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if id.is_empty() {
        return Err(format!("missing:{kind}-id"));
    }
    let created = item
        .get("created_at")
        .and_then(|v| v.as_str())
        .filter(|text| !text.is_empty())
        .ok_or("bad-row")?
        .to_string();
    let updated = item
        .get("updated_at")
        .and_then(|v| v.as_str())
        .filter(|text| !text.is_empty())
        .unwrap_or(&created)
        .to_string();
    let revision = item
        .get("revision")
        .and_then(|v| v.as_i64())
        .ok_or("bad-row")?;
    let record_device = item
        .get("device_id")
        .and_then(|v| v.as_str())
        .filter(|text| !text.is_empty())
        .unwrap_or(device)
        .to_string();
    let deleted_at = item
        .get("deleted_at")
        .and_then(|v| v.as_str())
        .filter(|text| !text.is_empty())
        .map(|text| text.to_string());
    let mut body = item.clone();
    if let Some(object) = body.as_object_mut() {
        for key in [
            "id",
            "created_at",
            "updated_at",
            "revision",
            "deleted_at",
            "device_id",
            "kind",
        ] {
            object.remove(key);
        }
        if kind == "rifle" {
            object.insert("serial".into(), json!(""));
        }
    }
    tx.execute(
        "INSERT INTO records (id, kind, created_at, updated_at, revision, deleted_at, device_id, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, kind, created, updated, revision, deleted_at, record_device, body.to_string()],
    ).map_err(|err| err.to_string())?;
    Ok(())
}

fn apply_weather_meta(tx: &rusqlite::Transaction<'_>, settings: &Value) -> Result<(), String> {
    if let Some(name) = settings.get("weather_place").and_then(|v| v.as_str()) {
        meta_put_tx(tx, "weather_place", name)?;
    }
    if settings.get("weather_latitude").is_some() {
        match settings.get("weather_latitude").and_then(|v| v.as_f64()) {
            Some(value) => meta_put_tx(tx, "weather_latitude", &value.to_string())?,
            None => meta_del_tx(tx, "weather_latitude")?,
        }
    }
    if settings.get("weather_longitude").is_some() {
        match settings.get("weather_longitude").and_then(|v| v.as_f64()) {
            Some(value) => meta_put_tx(tx, "weather_longitude", &value.to_string())?,
            None => meta_del_tx(tx, "weather_longitude")?,
        }
    }
    if let Some(minutes) = settings
        .get("weather_stale_minutes")
        .and_then(|v| v.as_u64())
    {
        meta_put_tx(tx, "weather_stale_minutes", &minutes.to_string())?;
    }
    Ok(())
}

fn meta_put_tx(tx: &rusqlite::Transaction<'_>, key: &str, value: &str) -> Result<(), String> {
    tx.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn meta_del_tx(tx: &rusqlite::Transaction<'_>, key: &str) -> Result<(), String> {
    tx.execute("DELETE FROM meta WHERE key = ?1", params![key])
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn apply_settings(tx: &rusqlite::Transaction<'_>, settings: &Value) -> Result<(), String> {
    tx.execute(
        "UPDATE settings SET preset = ?1, units_json = ?2, appearance = ?3, large_text = ?4,
         reduced_motion = ?5, card_pinned = ?6, angular = ?7, active_profile_id = ?8,
         current_air_json = ?9, effects_json = ?10, range_step = ?11, wind_step = ?12,
         last_range = ?13, last_wind_speed = ?14, last_wind_direction = ?15, last_inclination = ?16,
         onboarded = 1 WHERE id = 1",
        params![
            settings
                .get("preset")
                .and_then(|v| v.as_str())
                .unwrap_or("metric"),
            settings
                .get("units")
                .cloned()
                .unwrap_or(UnitSet::metric().to_json())
                .to_string(),
            settings
                .get("appearance")
                .and_then(|v| v.as_str())
                .unwrap_or("follow"),
            bool_or(settings, "large_text", false) as i64,
            bool_or(settings, "reduced_motion", false) as i64,
            bool_or(settings, "card_pinned", false) as i64,
            settings
                .get("angular")
                .and_then(|v| v.as_str())
                .unwrap_or("mrad"),
            settings
                .get("active_profile_id")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            settings
                .get("current_air")
                .cloned()
                .unwrap_or(default_air())
                .to_string(),
            settings
                .get("effects")
                .cloned()
                .unwrap_or(default_effects())
                .to_string(),
            settings
                .get("range_step")
                .and_then(|v| v.as_str())
                .unwrap_or("10 m"),
            settings
                .get("wind_step")
                .and_then(|v| v.as_str())
                .unwrap_or("1 mps"),
            settings
                .get("last_range")
                .and_then(|v| v.as_str())
                .unwrap_or("300 m"),
            settings
                .get("last_wind_speed")
                .and_then(|v| v.as_str())
                .unwrap_or("0 mps"),
            settings
                .get("last_wind_direction")
                .and_then(|v| v.as_str())
                .unwrap_or("12oc"),
            settings
                .get("last_inclination")
                .and_then(|v| v.as_str())
                .unwrap_or("0 deg")
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn required_text(body: &Value, key: &str) -> Result<String, String> {
    let text = text_or(body, key);
    if text.is_empty() {
        return Err(format!("{key} is required"));
    }
    Ok(text)
}

pub(crate) fn text_or(body: &Value, key: &str) -> String {
    body.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(crate) fn optional_live(
    store: &Store,
    body: &Value,
    key: &str,
    kind: &str,
) -> Result<String, String> {
    let id = text_or(body, key);
    if id.is_empty() {
        return Ok(String::new());
    }
    store.require_live(kind, &id)?;
    Ok(id)
}

fn is_http_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
}

fn pdf_filename(raw: &str) -> Result<String, String> {
    let name = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(raw)
        .trim()
        .to_string();
    if name.is_empty() || name == "." || name == ".." || name.contains('\0') {
        return Err("the PDF needs a file name".into());
    }
    if !name.to_ascii_lowercase().ends_with(".pdf") {
        return Err("Holdover stores a PDF, and this file is not one".into());
    }
    if name.chars().count() > 180 {
        return Err("the file name is too long".into());
    }
    Ok(name)
}

pub(crate) fn decode_base64(text: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    if cleaned.len() % 4 != 0 {
        return Err("the PDF data is not valid".into());
    }
    let bytes = cleaned.as_bytes();
    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    let mut index = 0;
    while index < bytes.len() {
        let chunk = &bytes[index..index + 4];
        let pad = chunk.iter().filter(|byte| **byte == b'=').count();
        if pad > 2 || (pad > 0 && index + 4 != bytes.len()) {
            return Err("the PDF data is not valid".into());
        }
        let mut packed = 0u32;
        for (place, byte) in chunk.iter().enumerate() {
            let value = if *byte == b'=' {
                0
            } else {
                base64_value(*byte).ok_or("the PDF data is not valid")?
            };
            packed |= value << (18 - 6 * place);
        }
        out.push((packed >> 16) as u8);
        if pad < 2 {
            out.push((packed >> 8) as u8);
        }
        if pad < 1 {
            out.push(packed as u8);
        }
        index += 4;
    }
    Ok(out)
}

fn base64_value(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some((byte - b'A') as u32),
        b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
        b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn text_any(body: &Value, key: &str) -> String {
    match body.get(key) {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Object(map)) => {
            let text = map
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let unit = map
                .get("unit")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if text.is_empty() {
                String::new()
            } else if unit.is_empty() || text_ends_with_unit(text) {
                text.to_string()
            } else {
                format!("{text} {unit}")
            }
        }
        _ => String::new(),
    }
}

fn text_ends_with_unit(text: &str) -> bool {
    text.chars()
        .last()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '%')
}

fn text_field(body: &Value, key: &str) -> String {
    match body.get(key) {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Object(map)) => map
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

fn hand(body: &Value) -> Result<String, String> {
    let hand = body
        .get("twist_hand")
        .and_then(|v| v.as_str())
        .unwrap_or("right");
    if matches!(hand, "right" | "left") {
        Ok(hand.into())
    } else {
        Err("twist hand must be right or left".into())
    }
}

fn bool_or(body: &Value, key: &str, default: bool) -> bool {
    body.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn required_qty(
    body: &Value,
    key: &str,
    dim: Dimension,
    fallback: &str,
) -> Result<(f64, String), String> {
    optional_qty(body, key, dim, fallback)?.ok_or_else(|| format!("{key} is required"))
}

fn optional_qty(
    body: &Value,
    key: &str,
    dim: Dimension,
    fallback: &str,
) -> Result<Option<(f64, String)>, String> {
    let Some(value) = body.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let (text, unit) = match value {
        Value::String(text) => (text.clone(), fallback.to_string()),
        Value::Object(map) => (
            map.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            map.get("unit")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(fallback)
                .to_string(),
        ),
        _ => return Err(format!("{key} must be a quantity")),
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    store_text(&text, dim, &unit)
        .map(Some)
        .map_err(|err| format!("{key}: {err}"))
}

fn store_text(text: &str, dim: Dimension, unit: &str) -> Result<(f64, String), String> {
    match parse_quantity(text, dim, None) {
        Ok(si) => Ok((si, text.trim().to_string())),
        Err(holdover_core::Error::Parse { reason, .. }) if reason.starts_with("a unit suffix") => {
            let si = parse_quantity(text, dim, Some(unit)).map_err(|err| err.to_string())?;
            Ok((si, format!("{} {unit}", text.trim())))
        }
        Err(err) => Err(err.to_string()),
    }
}

fn parse_humidity(text: &str) -> Result<(f64, String), String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok((0.0, "0%".into()));
    }
    let percent = trimmed.ends_with('%');
    let number = trimmed.trim_end_matches('%').trim().replace(',', ".");
    let value: f64 = number
        .parse()
        .map_err(|_| "humidity must be a number".to_string())?;
    let fraction = if percent || value > 1.0 {
        value / 100.0
    } else {
        value
    };
    if !(0.0..=1.0).contains(&fraction) {
        return Err("humidity must be from 0 to 100%".into());
    }
    Ok((fraction, trimmed.to_string()))
}

fn bc_warning(model: &str, bc: f64) -> Option<String> {
    if model == "G7" && !(0.1..=0.5).contains(&bc) {
        Some("G7 BC is usually between 0.1 and 0.5. Check it is not a G1 value.".into())
    } else if model == "G1" && !(0.1..=1.0).contains(&bc) {
        Some("G1 BC is usually between 0.1 and 1.0.".into())
    } else {
        None
    }
}

pub fn now_iso() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = elapsed.as_secs();
    let millis = elapsed.subsec_millis();
    let (year, month, day) = civil_from_days(secs as i64 / 86_400);
    let tod = secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{hh:02}:{mm:02}:{ss:02}.{millis:03}Z",
        hh = tod / 3600,
        mm = (tod / 60) % 60,
        ss = tod % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> Store {
        let path = std::env::temp_dir().join(format!(
            "holdover-{}-{}.db",
            std::process::id(),
            Uuid::new_v4()
        ));
        Store::open(&path).unwrap()
    }

    #[test]
    fn epoch_formats_as_1970() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn unit_switch_save_reload_keeps_si() {
        let store = temp_store();
        let rifle = store
            .save_rifle(&json!({
                "label": "Test",
                "sight_height": {"text": "45 mm", "unit": "mm"},
                "weight": {"text": "175 gr", "unit": "gr"}
            }))
            .unwrap();
        let id = rifle["id"].as_str().unwrap().to_string();
        let sight = rifle["sight_height_si"].as_f64().unwrap();
        let weight = rifle["weight_si"].as_f64().unwrap();
        store.set_preferences(&json!({"preset": "us"})).unwrap();
        let again = store.record_value("rifle", &id).unwrap();
        assert_eq!(
            again["sight_height_si"].as_f64().unwrap().to_bits(),
            sight.to_bits()
        );
        assert_eq!(
            again["weight_si"].as_f64().unwrap().to_bits(),
            weight.to_bits()
        );
        assert_eq!(again["sight_height_text"], "45 mm");
        assert_eq!(again["weight_text"], "175 gr");
        assert_eq!(again["sight_height_unit"], "in");
        store.set_preferences(&json!({"preset": "metric"})).unwrap();
        let metric = store.record_value("rifle", &id).unwrap();
        assert_eq!(
            metric["sight_height_si"].as_f64().unwrap().to_bits(),
            sight.to_bits()
        );
        assert_eq!(metric["sight_height_display"], "4.5");
        let doc = store.export_library().unwrap();
        assert_eq!(doc["rifles"][0]["serial"], "");
        let other = temp_store();
        other.import_library(&doc, true).unwrap();
        let imported = other.record_value("rifle", &id).unwrap();
        assert_eq!(
            imported["sight_height_si"].as_f64().unwrap().to_bits(),
            sight.to_bits()
        );
        assert_eq!(imported["sight_height_text"], "45 mm");
    }

    #[test]
    fn preset_switches_do_not_rewrite_several_dimensions() {
        let store = temp_store();
        let bullet = store
            .save_bullet(&json!({
                "label": "B",
                "diameter": {"text": "0.308 in", "unit": "in"},
                "weight": {"text": "175 gr", "unit": "gr"},
                "length": {"text": "1.24 in", "unit": "in"},
                "drag_model": "G7",
                "bc": 0.243,
                "bc_reference": "ICAO"
            }))
            .unwrap();
        let before = bullet["weight_si"].as_f64().unwrap().to_bits();
        for preset in ["us", "metric", "custom", "metric"] {
            store
                .set_preferences(&json!({"preset": preset, "units": UnitSet::metric().to_json()}))
                .unwrap();
            let again = store
                .record_value("bullet", bullet["id"].as_str().unwrap())
                .unwrap();
            assert_eq!(again["weight_si"].as_f64().unwrap().to_bits(), before);
            assert_eq!(again["weight_text"], "175 gr");
        }
    }

    fn rifle_body(label: &str, serial: &str) -> Value {
        json!({
            "label": label,
            "serial": serial,
            "sight_height": {"text": "45 mm", "unit": "mm"}
        })
    }

    fn sync_of(record: &Value) -> Value {
        json!({
            "id": record["id"],
            "created_at": record["created_at"],
            "updated_at": record["updated_at"],
            "revision": record["revision"],
            "deleted_at": record["deleted_at"],
            "device_id": record["device_id"],
            "label": record.get("label").cloned().unwrap_or(Value::Null),
            "serial": record.get("serial").cloned().unwrap_or(Value::Null),
        })
    }

    #[test]
    fn json_and_csv_round_trip_keep_sync_fields_and_schema() {
        let store = temp_store();
        let rifle = store.save_rifle(&rifle_body("Kept", "SN-SECRET")).unwrap();
        let rifle_id = rifle["id"].as_str().unwrap();
        assert_eq!(
            store.record_value("rifle", rifle_id).unwrap()["serial"],
            "SN-SECRET"
        );
        let scope = store
            .save_scope(&json!({
                "label": "Gone",
                "focal_plane": "ffp",
                "click": {"text": "0.1 mrad", "unit": "mrad"}
            }))
            .unwrap();
        store
            .delete_record("scope", scope["id"].as_str().unwrap())
            .unwrap();
        store
            .save_weather_place(&json!({
                "weather_place": "Home",
                "weather_latitude": "59.33",
                "weather_longitude": "18.07",
                "weather_stale_minutes": 30
            }))
            .unwrap();
        let doc = store.export_library().unwrap();
        assert_eq!(doc["schema_version"], 1);
        assert_eq!(doc["rifles"][0]["serial"], "");
        assert!(doc["settings"].get("weather_latitude").is_none());
        assert!(doc["settings"].get("weather_longitude").is_none());
        assert_eq!(doc["settings"]["weather_place"], "Home");
        assert_eq!(doc["rifles"][0]["device_id"], store.device_id().unwrap());
        let deleted = doc["scopes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["label"] == "Gone")
            .unwrap();
        assert!(deleted["deleted_at"].as_str().is_some());

        let other = temp_store();
        let other_device = other.device_id().unwrap();
        let preview = other.import_library(&doc, false).unwrap();
        assert_eq!(preview["written"], false);
        assert!(other.export_library().unwrap()["rifles"]
            .as_array()
            .unwrap()
            .is_empty());
        other.import_library(&doc, true).unwrap();
        assert_eq!(other.device_id().unwrap(), other_device);
        let again = other.export_library().unwrap();
        assert_eq!(again["schema_version"], 1);
        assert_eq!(again["device_id"], other_device);
        assert_eq!(sync_of(&doc["rifles"][0]), sync_of(&again["rifles"][0]));
        assert_eq!(sync_of(deleted), sync_of(&again["scopes"][0]));
        assert_eq!(
            other.weather_place().unwrap().latitude,
            None,
            "a backup without coordinates does not invent any"
        );

        let csv = store.export_library_csv(false).unwrap();
        let parsed = crate::library::csv_to_document(&csv).unwrap();
        let third = temp_store();
        third.import_library(&parsed, true).unwrap();
        let from_csv = third.export_library().unwrap();
        assert_eq!(from_csv["schema_version"], 1);
        assert_eq!(sync_of(&doc["rifles"][0]), sync_of(&from_csv["rifles"][0]));
        assert_eq!(sync_of(deleted), sync_of(&from_csv["scopes"][0]));

        let with_place = store.export_library_with(true).unwrap();
        assert_eq!(with_place["settings"]["weather_latitude"], 59.33);
        let placed = temp_store();
        placed.import_library(&with_place, true).unwrap();
        let latitude = placed.weather_place().unwrap().latitude.unwrap();
        assert!((latitude - 59.33).abs() < 1e-6);
    }

    #[test]
    fn import_preview_and_refusal_write_nothing() {
        let store = temp_store();
        let rifle = store.save_rifle(&rifle_body("Local", "SN")).unwrap();
        let id = rifle["id"].as_str().unwrap().to_string();
        let before = store.export_library().unwrap();
        let mut newer = before.clone();
        newer["schema_version"] = json!(2);
        assert_eq!(
            store.import_library(&newer, true).unwrap_err(),
            "schema-newer"
        );
        assert_eq!(store.record_value("rifle", &id).unwrap()["serial"], "SN");
        let err = store
            .import_text("Name,BC\nHornady,0.2\n", true, "replace")
            .unwrap_err();
        assert_eq!(err, "not-holdover");
        assert_eq!(store.record_value("rifle", &id).unwrap()["label"], "Local");
        let mut duplicate = store.export_library().unwrap();
        duplicate["scopes"] = duplicate["rifles"].clone();
        assert!(store
            .import_library(&duplicate, true)
            .unwrap_err()
            .starts_with("duplicate:"));
        assert_eq!(store.record_value("rifle", &id).unwrap()["serial"], "SN");
    }

    #[test]
    fn import_failure_rolls_back_the_whole_library() {
        let store = temp_store();
        let rifle = store.save_rifle(&rifle_body("Local", "SN-SECRET")).unwrap();
        let id = rifle["id"].as_str().unwrap().to_string();
        store.set_preferences(&json!({"preset": "us"})).unwrap();
        let incoming = {
            let other = temp_store();
            other.save_rifle(&rifle_body("Incoming", "")).unwrap();
            other.export_library().unwrap()
        };
        let err = store
            .import_prepared(&incoming, true, "replace", Some(0))
            .unwrap_err();
        assert_eq!(err, "aborted");
        let kept = store.record_value("rifle", &id).unwrap();
        assert_eq!(kept["label"], "Local");
        assert_eq!(kept["serial"], "SN-SECRET");
        assert_eq!(store.export_library().unwrap()["settings"]["preset"], "us");
        assert_eq!(
            store.export_library().unwrap()["rifles"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn merge_replaces_duplicates_and_keeps_local_only_rows() {
        let store = temp_store();
        let local = store.save_rifle(&rifle_body("Stay", "")).unwrap();
        let shared = store.save_rifle(&rifle_body("Old", "")).unwrap();
        let mut doc = store.export_library().unwrap();
        doc["rifles"]
            .as_array_mut()
            .unwrap()
            .retain(|item| item["id"] == shared["id"]);
        doc["rifles"][0]["label"] = json!("New");
        let preview = store.import_prepared(&doc, false, "merge", None).unwrap();
        assert_eq!(preview["written"], false);
        assert_eq!(preview["duplicates"].as_array().unwrap().len(), 1);
        assert_eq!(preview["local_only"], 1);
        store.import_prepared(&doc, true, "merge", None).unwrap();
        assert_eq!(
            store
                .record_value("rifle", local["id"].as_str().unwrap())
                .unwrap()["label"],
            "Stay"
        );
        assert_eq!(
            store
                .record_value("rifle", shared["id"].as_str().unwrap())
                .unwrap()["label"],
            "New"
        );
    }

    #[test]
    fn fetched_weather_uses_surface_pressure_and_manual_edit_clears_it() {
        let store = temp_store();
        store.set_preferences(&json!({"preset": "us"})).unwrap();
        store
            .apply_weather(&json!({
                "temperature_c": 7.4,
                "humidity": 0.86,
                "elevation_m": 28.0,
                "surface_pressure_pa": 100420.0,
                "sea_level_pressure_pa": 100780.0,
                "pressure_used": "station",
                "fetched_at": "2026-10-21T01:00:12Z",
                "valid_time": "2026-10-21T01:00",
                "grid_latitude": 59.3294,
                "grid_longitude": 18.0686,
                "place_name": "Home",
                "wind_speed_mps": 4.5,
                "wind_from_deg": 210
            }))
            .unwrap();
        let air = store.export_library_with(true).unwrap()["settings"]["current_air"].clone();
        assert_eq!(air["pressure_kind"], "station");
        assert_eq!(air["provenance"]["pressure_field"], "surface_pressure");
        assert_eq!(air["provenance"]["source"], "fetched");
        assert!(air["pressure"]["text"].as_str().unwrap().ends_with(" inHg"));
        assert!(air["temperature"]["text"].as_str().unwrap().ends_with(" F"));
        assert!(air["altitude"]["text"].as_str().unwrap().ends_with(" yd"));
        assert!(air["provenance"]["grid_latitude"].as_f64().is_some());
        let stripped = store.export_library().unwrap();
        assert!(stripped["settings"]["current_air"]["provenance"]
            .get("grid_latitude")
            .is_none());
        assert!(stripped["settings"]["current_air"]["provenance"]
            .get("grid_longitude")
            .is_none());
        assert!(stripped["settings"].get("weather_age_secs").is_none());
        assert!(stripped["settings"].get("weather_stale").is_none());
        assert!(stripped["settings"].get("weather_stale_minutes").is_some());

        let (age, stale) =
            crate::weather::weather_age("2026-10-21T01:00:12Z", "2026-10-21T03:00:12Z", 30 * 60)
                .unwrap();
        assert_eq!(age, 7200);
        assert!(stale);

        store
            .remember_solve(&json!({
                "current_air": air,
                "wind_direction": "9oc",
                "wind_speed": "0 mph"
            }))
            .unwrap();
        let kept = store.export_library_with(true).unwrap()["settings"]["current_air"].clone();
        assert_eq!(kept["provenance"]["fetched_at"], "2026-10-21T01:00:12Z");

        let mut edited = air.clone();
        edited["temperature"] = json!({"text": "90 F", "unit": "F"});
        edited.as_object_mut().unwrap().remove("provenance");
        store
            .remember_solve(&json!({
                "current_air": edited,
                "wind_direction": "9oc"
            }))
            .unwrap();
        let manual = store.export_library().unwrap()["settings"]["current_air"].clone();
        assert_eq!(manual["provenance"]["source"], "manual");

        store
            .apply_weather(&json!({
                "accept_wind": true,
                "wind_speed_mps": 4.5
            }))
            .unwrap();
        let settings = store.export_library().unwrap()["settings"].clone();
        assert!(settings["last_wind_speed"]
            .as_str()
            .unwrap()
            .ends_with(" mph"));
        assert_eq!(settings["last_wind_direction"], "9oc");
    }

    #[test]
    fn load_log_stores_a_charge_and_leaves_muzzle_velocity_alone() {
        let store = temp_store();
        let id = store.seed_example().unwrap();
        let bundle = store.profile_bundle(&id).unwrap();
        let load_id = bundle["load"]["id"].as_str().unwrap().to_string();
        let before = bundle["load"]["mv_mps"].as_f64().unwrap();
        let log = store
            .save_load_log(&json!({
                "label": "Range day",
                "load_id": load_id,
                "powder": "a powder I wrote down",
                "powder_lot": "lot 1",
                "charge": {"text": "45 gr", "unit": "gr"},
                "coal": {"text": "71 mm", "unit": "mm"},
                "notes": "from my notebook"
            }))
            .unwrap();
        assert!(log["charge_si"].as_f64().unwrap() > 0.0);
        assert!(log["coal_si"].as_f64().unwrap() > 0.0);
        let after = store.profile_bundle(&id).unwrap()["load"]["mv_mps"]
            .as_f64()
            .unwrap();
        assert_eq!(before, after);
        store.set_preferences(&json!({"preset": "us"})).unwrap();
        let shown = store
            .record_value("load_log", log["id"].as_str().unwrap())
            .unwrap();
        assert_eq!(log["charge_unit"], "g");
        assert_eq!(
            shown["charge_si"].as_f64().unwrap(),
            log["charge_si"].as_f64().unwrap()
        );
        assert_eq!(shown["charge_unit"], "gr");
        assert_eq!(shown["coal_unit"], "in");
    }

    #[test]
    fn archive_and_history_round_trip() {
        let store = temp_store();
        let rifle = store
            .save_rifle(&json!({
                "label": "First",
                "sight_height": {"text": "45 mm", "unit": "mm"}
            }))
            .unwrap();
        let id = rifle["id"].as_str().unwrap().to_string();
        store
            .save_rifle(&json!({
                "id": id,
                "label": "Second",
                "sight_height": {"text": "45 mm", "unit": "mm"}
            }))
            .unwrap();
        let history = store.bootstrap().unwrap()["equipment"]["history"].clone();
        assert_eq!(history["count"], 1);
        assert_eq!(history["rows"][0]["subject_label"], "First");
        store.delete_record("rifle", &id).unwrap();
        assert!(store.record_value("rifle", &id).is_err());
        let archived = store.bootstrap().unwrap()["equipment"]["archived"].clone();
        assert_eq!(archived[0]["label"], "Second");
        store.restore_record("rifle", &id).unwrap();
        assert_eq!(store.record_value("rifle", &id).unwrap()["label"], "Second");
        let history_id = history["rows"][0]["id"].as_str().unwrap();
        let restored = store.restore_history(history_id).unwrap();
        assert_eq!(restored["label"], "First");
        let again = store.bootstrap().unwrap()["equipment"]["history"]["count"]
            .as_i64()
            .unwrap();
        assert!(again >= 2);
    }

    #[test]
    fn a_document_link_and_pdf_are_stored_without_executing_them() {
        let store = temp_store();
        let link = store
            .save_document(&json!({
                "title": "Load data",
                "edition": "2026",
                "edition_date": "2026 printing",
                "link_url": "https://example.com/load-data",
                "notes": "my own pointer"
            }))
            .unwrap();
        assert_eq!(link["link_url"], "https://example.com/load-data");
        assert!(store
            .save_document(&json!({"title": "Bad", "link_url": "file:///tmp/x"}))
            .is_err());
        let pdf = b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\n%%EOF\n";
        let encoded = base64_std(pdf);
        let saved = store
            .save_document(&json!({
                "title": "Printed manual",
                "file": {"filename": "manual.pdf", "base64": encoded}
            }))
            .unwrap();
        let hash = saved["sha256"].as_str().unwrap();
        assert_eq!(hash.len(), 64);
        let path = store.blob_path(hash).unwrap();
        assert_eq!(fs::read(&path).unwrap(), pdf);
        let again = store
            .save_document(&json!({
                "title": "Same bytes",
                "file": {"filename": "other.pdf", "base64": encoded}
            }))
            .unwrap();
        assert_eq!(again["sha256"], saved["sha256"]);
        let target = store
            .document_target(saved["id"].as_str().unwrap())
            .unwrap();
        assert_eq!(target["kind"], "file");
        assert!(target["path"].as_str().unwrap().ends_with(".pdf"));
        let exported = store.export_library().unwrap();
        let documents = exported["documents"].as_array().unwrap();
        assert_eq!(documents.len(), 3);
        assert!(documents.iter().all(|row| row.get("base64").is_none()));
        let mut older = exported.clone();
        older.as_object_mut().unwrap().remove("documents");
        older.as_object_mut().unwrap().remove("load_logs");
        older.as_object_mut().unwrap().remove("history");
        let other = temp_store();
        other.import_library(&older, true).unwrap();
    }

    fn base64_std(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
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
}
