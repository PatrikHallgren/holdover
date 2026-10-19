//! SQLite library. Each record stores SI values beside the original text.
//!
//! Settings hold the unit preset. Changing that preset does not update record
//! bodies, so a save/reload after a switch keeps the same normalized values.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use holdover_core::{
    card_pages, format_number, parse_quantity, running_presets, Dimension, DEFAULT_EJECTA_MPS,
    EJECTA_CONVENTION, SJF_MOOSE_RING_DIAMETER_CM, SJF_ROE_BUCK_RING_DIAMETER_CM,
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

pub struct Store {
    conn: Connection,
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
        Ok(Self { conn })
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
            "disclaimer": holdover_core::DISCLAIMER,
            "spin_provenance": holdover_core::SPIN_DRIFT_PROVENANCE,
            "targets": {
                "moose_cm": SJF_MOOSE_RING_DIAMETER_CM,
                "roe_cm": SJF_ROE_BUCK_RING_DIAMETER_CM,
            },
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
        Ok(())
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
        let changed = self
            .conn
            .execute(
                "UPDATE records SET deleted_at = ?1, updated_at = ?1, revision = revision + 1
             WHERE id = ?2 AND kind = ?3 AND deleted_at IS NULL",
                params![now, id, kind],
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
        self.conn
            .execute(
                "UPDATE settings SET current_air_json = ?1, effects_json = ?2, last_range = ?3,
             last_wind_speed = ?4, last_wind_direction = ?5, last_inclination = ?6,
             angular = ?7, active_profile_id = ?8, range_step = ?9, wind_step = ?10 WHERE id = 1",
                params![
                    body.get("current_air")
                        .cloned()
                        .unwrap_or(default_air())
                        .to_string(),
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
        let mut settings = self.settings_json()?;
        if let Some(object) = settings.as_object_mut() {
            object.remove("onboarded");
        }
        let mut rifles = self.kind_export("rifle")?;
        for rifle in &mut rifles {
            if let Some(object) = rifle.as_object_mut() {
                object.insert("serial".into(), json!(""));
            }
        }
        Ok(json!({
            "schema_version": 1,
            "exported_at": now_iso(),
            "device_id": self.device_id()?,
            "settings": settings,
            "rifles": rifles,
            "scopes": self.kind_export("scope")?,
            "bullets": self.kind_export("bullet")?,
            "loads": self.kind_export("load")?,
            "atmospheres": self.kind_export("atmosphere")?,
            "profiles": self.kind_export("profile")?,
            "chronos": self.kind_export("chrono")?,
        }))
    }

    pub fn import_library(&self, doc: &Value, commit: bool) -> Result<Value, String> {
        let version = doc
            .get("schema_version")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if version != 1 {
            return Err("backup schema_version must be 1".into());
        }
        for key in [
            "rifles",
            "scopes",
            "bullets",
            "loads",
            "atmospheres",
            "profiles",
        ] {
            if !doc.get(key).map(|v| v.is_array()).unwrap_or(false) {
                return Err(format!("backup is missing {key}"));
            }
        }
        let preview = json!({
            "replaces": true,
            "rifles": doc["rifles"].as_array().map(|v| v.len()).unwrap_or(0),
            "scopes": doc["scopes"].as_array().map(|v| v.len()).unwrap_or(0),
            "bullets": doc["bullets"].as_array().map(|v| v.len()).unwrap_or(0),
            "loads": doc["loads"].as_array().map(|v| v.len()).unwrap_or(0),
            "atmospheres": doc["atmospheres"].as_array().map(|v| v.len()).unwrap_or(0),
            "profiles": doc["profiles"].as_array().map(|v| v.len()).unwrap_or(0),
            "chronos": doc.get("chronos").and_then(|v| v.as_array()).map(|v| v.len()).unwrap_or(0),
        });
        if !commit {
            return Ok(preview);
        }
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|err| err.to_string())?;
        tx.execute("DELETE FROM records", [])
            .map_err(|err| err.to_string())?;
        let device = self.device_id()?;
        for (key, kind) in [
            ("atmospheres", "atmosphere"),
            ("rifles", "rifle"),
            ("scopes", "scope"),
            ("bullets", "bullet"),
            ("loads", "load"),
            ("profiles", "profile"),
        ] {
            for item in doc[key].as_array().cloned().unwrap_or_default() {
                insert_raw(&tx, kind, &item, &device)?;
            }
        }
        if let Some(items) = doc.get("chronos").and_then(|v| v.as_array()) {
            for item in items {
                insert_raw(&tx, "chrono", item, &device)?;
            }
        }
        if let Some(settings) = doc.get("settings") {
            apply_settings(&tx, settings)?;
        }
        tx.commit().map_err(|err| err.to_string())?;
        Ok(preview)
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
        self.conn.query_row(
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
        ).map_err(|err| err.to_string())
    }

    fn equipment_json(&self, units: &UnitSet) -> Result<Value, String> {
        Ok(json!({
            "rifles": self.list_kind("rifle", units)?,
            "scopes": self.list_kind("scope", units)?,
            "bullets": self.list_kind("bullet", units)?,
            "loads": self.list_kind("load", units)?,
            "profiles": self.list_profiles(units)?,
            "chronos": self.list_kind("chrono", units)?,
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

    fn record_value(&self, kind: &str, id: &str) -> Result<Value, String> {
        let units = self.units()?;
        let rows = self.list_kind(kind, &units)?;
        rows.into_iter()
            .find(|row| row["id"] == id)
            .ok_or_else(|| format!("{kind} was not found"))
    }

    fn body(&self, kind: &str, id: &str) -> Result<Value, String> {
        let (raw, revision): (String, i64) = self.conn.query_row(
            "SELECT body, revision FROM records WHERE id = ?1 AND kind = ?2 AND deleted_at IS NULL",
            params![id, kind],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|_| format!("{kind} {id} was not found"))?;
        let mut value: Value = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
        stamp(&mut value, id, kind, revision);
        Ok(value)
    }

    fn write_record(&self, kind: &str, request: &Value, body: Value) -> Result<String, String> {
        let now = now_iso();
        let device = self.device_id()?;
        if let Some(id) = request
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let changed = self
                .conn
                .execute(
                    "UPDATE records SET body = ?1, updated_at = ?2, revision = revision + 1
                 WHERE id = ?3 AND kind = ?4 AND deleted_at IS NULL",
                    params![body.to_string(), now, id, kind],
                )
                .map_err(|err| err.to_string())?;
            if changed == 0 {
                return Err(format!("{kind} was not found"));
            }
            return Ok(id.to_string());
        }
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO records (id, kind, created_at, updated_at, revision, deleted_at, device_id, body)
             VALUES (?1, ?2, ?3, ?3, 1, NULL, ?4, ?5)",
            params![id, kind, now, device, body.to_string()],
        ).map_err(|err| err.to_string())?;
        Ok(id)
    }

    fn require_live(&self, kind: &str, id: &str) -> Result<(), String> {
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
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, updated_at, revision, body FROM records WHERE kind = ?1 AND deleted_at IS NULL",
        ).map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![kind], |row| {
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
            let (id, created, updated, revision, body) = row.map_err(|err| err.to_string())?;
            let mut value: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
            if let Some(object) = value.as_object_mut() {
                object.insert("id".into(), json!(id));
                object.insert("created_at".into(), json!(created));
                object.insert("updated_at".into(), json!(updated));
                object.insert("revision".into(), json!(revision));
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

fn stamp(value: &mut Value, id: &str, kind: &str, revision: i64) {
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
        return Err(format!("backup {kind} is missing an id"));
    }
    let created = item
        .get("created_at")
        .and_then(|v| v.as_str())
        .unwrap_or(&now_iso())
        .to_string();
    let updated = item
        .get("updated_at")
        .and_then(|v| v.as_str())
        .unwrap_or(&created)
        .to_string();
    let revision = item.get("revision").and_then(|v| v.as_i64()).unwrap_or(1);
    let mut body = item.clone();
    if let Some(object) = body.as_object_mut() {
        for key in ["id", "created_at", "updated_at", "revision", "kind"] {
            object.remove(key);
        }
        if kind == "rifle" {
            object.insert("serial".into(), json!(""));
        }
    }
    tx.execute(
        "INSERT INTO records (id, kind, created_at, updated_at, revision, deleted_at, device_id, body)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)",
        params![id, kind, created, updated, revision, device, body.to_string()],
    ).map_err(|err| err.to_string())?;
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

fn text_or(body: &Value, key: &str) -> String {
    body.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
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
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day) = civil_from_days(secs as i64 / 86_400);
    let tod = secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{hh:02}:{mm:02}:{ss:02}Z",
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
}
