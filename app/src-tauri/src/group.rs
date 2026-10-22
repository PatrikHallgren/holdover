//! Saved shot groups. The geometry is computed in `holdover-core`. This module
//! only stores the photograph, the marked points, and the analysis.

use holdover_core::{analyze_document, detect_image_holes, target_by_id, Dimension, TargetDoc};
use serde_json::{json, Value};

use crate::photo::{self, PreparedPhoto};
use crate::store::{optional_live, text_or, Store};

impl Store {
    pub fn import_group_photo(
        &self,
        bytes: &[u8],
        filename: &str,
        keep_location: bool,
    ) -> Result<Value, String> {
        let prepared = photo::prepare_photo(bytes, filename)?;
        self.photo_value(&prepared, keep_location)
    }

    fn photo_value(&self, prepared: &PreparedPhoto, keep_location: bool) -> Result<Value, String> {
        let hash = self.put_blob(&prepared.png)?;
        let mut value = json!({
            "sha256": hash,
            "media": "image/png",
            "width": prepared.width,
            "height": prepared.height,
            "filename": prepared.filename,
        });
        if keep_location {
            if let Some(latitude) = prepared.latitude {
                value["latitude"] = json!(latitude);
            }
            if let Some(longitude) = prepared.longitude {
                value["longitude"] = json!(longitude);
            }
        }
        Ok(value)
    }

    pub fn group_photo_bytes(&self, hash: &str) -> Result<Vec<u8>, String> {
        let bytes = self
            .read_blob(hash)
            .map_err(|_| "photo-missing".to_string())?;
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Ok(bytes);
        }
        Err("photo-missing".into())
    }

    pub fn analyze_group(&self, body: &Value) -> Result<Value, String> {
        let input = analysis_input(body)?;
        let target = resolve_target(self, &text_or(body, "target_id"))?;
        analyze_document(&input, target.as_ref()).map_err(|err| err.to_string())
    }

    pub fn detect_group_holes(&self, body: &Value) -> Result<Value, String> {
        let hash = body
            .get("photo")
            .and_then(|photo| photo.get("sha256"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .ok_or("import a photograph first")?;
        let bytes = self.group_photo_bytes(hash)?;
        let raster = photo::raster_from_bytes(&bytes)?;
        let calibration = body.get("calibration").cloned().unwrap_or(Value::Null);
        let diameter = length_mm(body, "bullet_diameter", "bullet_diameter_mm")?;
        let threshold = body
            .get("threshold")
            .and_then(|value| value.as_u64())
            .unwrap_or(90)
            .min(255) as u8;
        detect_image_holes(&raster, &calibration, diameter, threshold)
            .map_err(|err| err.to_string())
    }

    pub fn save_group(&self, body: &Value) -> Result<Value, String> {
        let label = text_or(body, "label");
        if label.is_empty() {
            return Err("label is required".into());
        }
        if label.chars().count() > 200 {
            return Err("the label is too long".into());
        }
        let notes = text_or(body, "notes");
        if notes.chars().count() > 4000 {
            return Err("the notes are too long".into());
        }
        let photo = body
            .get("photo")
            .cloned()
            .filter(|value| value.is_object())
            .ok_or("import a photograph first")?;
        let hash = photo
            .get("sha256")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        self.group_photo_bytes(hash)?;
        let input = analysis_input(body)?;
        let target_id = resolve_target_id(self, body)?;
        let target = resolve_target(self, &target_id)?;
        let analysis = analyze_document(&input, target.as_ref()).map_err(|err| err.to_string())?;
        if analysis.get("ok").and_then(|value| value.as_bool()) != Some(true) {
            return Err(save_block(
                analysis
                    .get("reason")
                    .and_then(|value| value.as_str())
                    .unwrap_or(""),
            ));
        }
        if analysis.get("can_save").and_then(|value| value.as_bool()) != Some(true) {
            return Err(
                "the shot count has to match, and the distance has to be set, before saving".into(),
            );
        }
        let distance_m = input
            .get("distance_m")
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0);
        let keep_location = body
            .get("keep_location")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let mut record = json!({
            "label": label,
            "shot_at": text_or(body, "shot_at"),
            "distance_m": distance_m,
            "distance_text": distance_text(body, distance_m),
            "profile_id": optional_live(self, body, "profile_id", "profile")?,
            "load_id": optional_live(self, body, "load_id", "load")?,
            "chrono_id": optional_live(self, body, "chrono_id", "chrono")?,
            "target_id": target_id,
            "ringing": ringing(body),
            "notes": notes,
            "expected_shots": input.get("expected_shots").cloned().unwrap_or(json!(0)),
            "measure": input.get("measure").cloned().unwrap_or(json!("centres")),
            "bullet_diameter_mm": input.get("bullet_diameter_mm").cloned().unwrap_or(Value::Null),
            "bullet_diameter_text": text_or(body, "bullet_diameter_text"),
            "keep_location": keep_location,
            "photo": {
                "sha256": hash,
                "media": photo.get("media").and_then(|value| value.as_str()).unwrap_or("image/png"),
                "width": photo.get("width").cloned().unwrap_or(json!(0)),
                "height": photo.get("height").cloned().unwrap_or(json!(0)),
                "filename": photo.get("filename").and_then(|value| value.as_str()).unwrap_or("photo.png"),
            },
            "calibration": stored_calibration(body, &analysis),
            "poa_image": body.get("poa_image").cloned().unwrap_or(Value::Null),
            "use_rectangle_centre": body.get("use_rectangle_centre").and_then(|value| value.as_bool()).unwrap_or(false),
            "holes": body.get("holes").cloned().unwrap_or(json!([])),
            "atmosphere": self.current_air_raw().unwrap_or_else(|_| json!({})),
            "analysis": analysis,
        });
        if keep_location {
            if let Some(latitude) = body.get("latitude").and_then(|value| value.as_f64()) {
                record["latitude"] = json!(latitude);
            }
            if let Some(longitude) = body.get("longitude").and_then(|value| value.as_f64()) {
                record["longitude"] = json!(longitude);
            }
        }
        let id = self.write_record("group", body, record)?;
        self.record_value("group", &id)
    }
}

fn analysis_input(body: &Value) -> Result<Value, String> {
    let mut input = json!({
        "calibration": body.get("calibration").cloned().unwrap_or(Value::Null),
        "holes": body.get("holes").cloned().unwrap_or(json!([])),
        "expected_shots": body.get("expected_shots").and_then(|value| value.as_u64()).unwrap_or(0),
        "measure": body.get("measure").and_then(|value| value.as_str()).unwrap_or("centres"),
        "ringing": ringing(body),
        "use_rectangle_centre": body.get("use_rectangle_centre").and_then(|value| value.as_bool()).unwrap_or(false),
        "poa_image": body.get("poa_image").cloned().unwrap_or(Value::Null),
    });
    if let Some(distance) = length_m(body, "distance", "distance_m")? {
        input["distance_m"] = json!(distance);
    }
    if let Some(diameter) = length_mm(body, "bullet_diameter", "bullet_diameter_mm")? {
        input["bullet_diameter_mm"] = json!(diameter);
    }
    Ok(input)
}

fn stored_calibration(body: &Value, analysis: &Value) -> Value {
    let mut calibration = body.get("calibration").cloned().unwrap_or(json!({}));
    if let Some(object) = calibration.as_object_mut() {
        if let Some(matrix) = analysis.get("matrix") {
            object.insert("matrix".into(), matrix.clone());
        }
        if let Some(precision) = analysis.get("precision_mm") {
            object.insert("precision_mm".into(), precision.clone());
        }
        if let Some(warnings) = analysis.get("warnings") {
            object.insert("warnings".into(), warnings.clone());
        }
    }
    calibration
}

fn ringing(body: &Value) -> String {
    if text_or(body, "ringing") == "alt" {
        "alt".into()
    } else {
        "primary".into()
    }
}

fn distance_text(body: &Value, metres: f64) -> String {
    if let Some(text) = body.get("distance_text").and_then(|value| value.as_str()) {
        if !text.trim().is_empty() {
            return text.trim().to_string();
        }
    }
    if let Some(distance) = body.get("distance") {
        let text = distance
            .get("text")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let unit = distance
            .get("unit")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let joined = format!("{text} {unit}").trim().to_string();
        if !joined.is_empty() {
            return joined;
        }
    }
    format!("{metres} m")
}

fn length_m(body: &Value, object_key: &str, metres_key: &str) -> Result<Option<f64>, String> {
    if let Some(object) = body.get(object_key).filter(|value| value.is_object()) {
        let text = object
            .get("text")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if text.trim().is_empty() {
            return Ok(None);
        }
        let unit = object.get("unit").and_then(|value| value.as_str());
        let metres = holdover_core::parse_quantity(text, Dimension::Length, unit)
            .map_err(|err| err.to_string())?;
        return Ok(Some(metres));
    }
    Ok(body.get(metres_key).and_then(|value| value.as_f64()))
}

fn length_mm(body: &Value, object_key: &str, mm_key: &str) -> Result<Option<f64>, String> {
    if let Some(metres) = length_m(body, object_key, "")? {
        return Ok(Some(metres * 1000.0));
    }
    Ok(body
        .get(mm_key)
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite() && *value > 0.0))
}

fn resolve_target_id(store: &Store, body: &Value) -> Result<String, String> {
    let id = text_or(body, "target_id");
    if id.is_empty() || id == "none" {
        return Ok(String::new());
    }
    if target_by_id(&id).is_some() {
        return Ok(id);
    }
    store.require_live("target", &id)?;
    Ok(id)
}

fn resolve_target(store: &Store, id: &str) -> Result<Option<TargetDoc>, String> {
    if id.is_empty() {
        return Ok(None);
    }
    if let Some(doc) = target_by_id(id) {
        return Ok(Some(doc));
    }
    let body = store.body("target", id)?;
    serde_json::from_value(body)
        .map(Some)
        .map_err(|err| err.to_string())
}

fn save_block(reason: &str) -> String {
    match reason {
        "corners" => "mark the calibration points before saving".into(),
        "size" => "enter the reference size before saving".into(),
        "aim-point" => "mark the aim point, or use the centre of the rectangle".into(),
        "calibration" => "choose a calibration before saving".into(),
        "diameter" => "an outside-edge group needs the bullet diameter".into(),
        _ => "the group is not ready to save".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn temp_store() -> Store {
        let path = std::env::temp_dir().join(format!(
            "holdover-group-{}-{}.db",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        Store::open(&path).unwrap()
    }

    fn png() -> Vec<u8> {
        let image = RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 255]));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn a_group_saves_with_sync_fields_and_drops_location_unless_asked() {
        let store = temp_store();
        let photo = store.import_group_photo(&png(), "card.png", false).unwrap();
        assert!(photo.get("latitude").is_none());
        let saved = store
            .save_group(&json!({
                "label": "Bench",
                "distance": {"text": "100 m", "unit": "m"},
                "expected_shots": 1,
                "measure": "centres",
                "photo": photo,
                "calibration": {
                    "mode": "four",
                    "image_points": [
                        {"x": 0, "y": 0},
                        {"x": 100, "y": 0},
                        {"x": 100, "y": 100},
                        {"x": 0, "y": 100}
                    ],
                    "width_mm": 200,
                    "height_mm": 200
                },
                "use_rectangle_centre": true,
                "holes": [{"image": {"x": 40, "y": 60}, "count": 1}],
                "latitude": 59.3,
                "longitude": 18.0
            }))
            .unwrap();
        assert_eq!(saved["kind"], "group");
        assert_eq!(saved["revision"], 1);
        assert!(saved.get("latitude").is_none());
        assert!(saved["analysis"]["can_save"].as_bool().unwrap());
        let hash = saved["photo"]["sha256"].as_str().unwrap();
        assert_eq!(hash.len(), 64);
        assert!(store
            .group_photo_bytes(hash)
            .unwrap()
            .starts_with(b"\x89PNG"));
    }
}
