//! Holdover's own library file. JSON and CSV carry the same document.
//!
//! There is no importer for another application or device. A file that is not
//! this schema is refused before any row is written.

use serde_json::{json, Map, Value};

pub const SCHEMA_VERSION: i64 = 1;
pub const CSV_HEADER: &str =
    "format,schema_version,section,id,created_at,updated_at,revision,deleted_at,device_id,body";

pub const RECORD_SECTIONS: &[(&str, &str)] = &[
    ("rifles", "rifle"),
    ("scopes", "scope"),
    ("bullets", "bullet"),
    ("loads", "load"),
    ("atmospheres", "atmosphere"),
    ("profiles", "profile"),
    ("chronos", "chrono"),
    ("reticles", "reticle"),
    ("targets", "target"),
    ("documents", "document"),
    ("load_logs", "load_log"),
    ("groups", "group"),
    ("history", "history"),
];

const REQUIRED: &[&str] = &[
    "rifles",
    "scopes",
    "bullets",
    "loads",
    "atmospheres",
    "profiles",
];

pub fn document_to_csv(doc: &Value) -> Result<String, String> {
    validate_document(doc)?;
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    push_row(
        &mut out,
        &[
            "holdover-library",
            "1",
            "device",
            "",
            "",
            "",
            "",
            "",
            doc.get("device_id").and_then(|v| v.as_str()).unwrap_or(""),
            "",
        ],
    );
    if let Some(settings) = doc.get("settings") {
        push_row(
            &mut out,
            &[
                "holdover-library",
                "1",
                "settings",
                "",
                "",
                "",
                "",
                "",
                "",
                &settings.to_string(),
            ],
        );
    }
    for (key, section) in RECORD_SECTIONS {
        let Some(items) = doc.get(*key).and_then(|v| v.as_array()) else {
            continue;
        };
        for item in items {
            push_row(
                &mut out,
                &[
                    "holdover-library",
                    "1",
                    section,
                    item.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                    item.get("created_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    item.get("updated_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    &item
                        .get("revision")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0)
                        .to_string(),
                    item.get("deleted_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    item.get("device_id").and_then(|v| v.as_str()).unwrap_or(""),
                    &item.to_string(),
                ],
            );
        }
    }
    Ok(out)
}

pub fn csv_to_document(text: &str) -> Result<Value, String> {
    let rows = parse_csv(text)?;
    if rows.is_empty() || rows[0].len() != 10 || rows[0].join(",") != CSV_HEADER {
        return Err("not-holdover".into());
    }
    let mut doc = json!({
        "schema_version": SCHEMA_VERSION,
        "rifles": [],
        "scopes": [],
        "bullets": [],
        "loads": [],
        "atmospheres": [],
        "profiles": [],
        "chronos": [],
        "reticles": [],
        "targets": [],
        "documents": [],
        "load_logs": [],
        "groups": [],
        "history": [],
    });
    for row in rows.iter().skip(1) {
        if row.iter().all(|cell| cell.is_empty()) {
            continue;
        }
        if row.len() != 10 || row[0] != "holdover-library" {
            return Err("not-holdover".into());
        }
        let version: i64 = row[1].parse().map_err(|_| "not-holdover")?;
        if version > SCHEMA_VERSION {
            return Err("schema-newer".into());
        }
        if version != SCHEMA_VERSION {
            return Err("schema-older".into());
        }
        match row[2].as_str() {
            "device" => {
                doc["device_id"] = json!(row[8]);
            }
            "settings" => {
                let settings: Value =
                    serde_json::from_str(&row[9]).map_err(|_| "bad-row".to_string())?;
                if !settings.is_object() {
                    return Err("bad-row".into());
                }
                doc["settings"] = settings;
            }
            section => {
                let key = RECORD_SECTIONS
                    .iter()
                    .find(|(_, name)| *name == section)
                    .map(|(key, _)| *key)
                    .ok_or("not-holdover")?;
                let mut item: Value =
                    serde_json::from_str(&row[9]).map_err(|_| "bad-row".to_string())?;
                if let Some(object) = item.as_object_mut() {
                    object.insert("id".into(), json!(row[3]));
                    object.insert("created_at".into(), json!(row[4]));
                    object.insert("updated_at".into(), json!(row[5]));
                    let revision: i64 = row[6].parse().map_err(|_| "bad-row")?;
                    object.insert("revision".into(), json!(revision));
                    if row[7].is_empty() {
                        object.insert("deleted_at".into(), Value::Null);
                    } else {
                        object.insert("deleted_at".into(), json!(row[7]));
                    }
                    object.insert("device_id".into(), json!(row[8]));
                } else {
                    return Err("bad-row".into());
                }
                doc[key].as_array_mut().ok_or("bad-row")?.push(item);
            }
        }
    }
    validate_document(&doc)?;
    Ok(doc)
}

pub fn validate_document(doc: &Value) -> Result<(), String> {
    let version = doc.get("schema_version").and_then(|v| v.as_i64());
    match version {
        Some(SCHEMA_VERSION) => {}
        Some(other) if other > SCHEMA_VERSION => return Err("schema-newer".into()),
        Some(_) => return Err("schema-older".into()),
        None => return Err("not-holdover".into()),
    }
    for key in REQUIRED {
        if !doc.get(*key).map(|v| v.is_array()).unwrap_or(false) {
            return Err(format!("missing:{key}"));
        }
    }
    for (key, _) in RECORD_SECTIONS {
        if let Some(value) = doc.get(*key) {
            if !value.is_array() {
                return Err(format!("missing:{key}"));
            }
        }
    }
    let mut seen = Vec::new();
    for (key, _) in RECORD_SECTIONS {
        let Some(items) = doc.get(*key).and_then(|v| v.as_array()) else {
            continue;
        };
        for item in items {
            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() {
                return Err(format!("missing:{key}-id"));
            }
            if item
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty()
                || item
                    .get("updated_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .is_empty()
                || item.get("revision").and_then(|v| v.as_i64()).is_none()
            {
                return Err("bad-row".into());
            }
            if seen.iter().any(|existing: &String| existing == id) {
                return Err(format!("duplicate:{id}"));
            }
            seen.push(id.to_string());
        }
    }
    if let Some(settings) = doc.get("settings") {
        if !settings.is_null() && !settings.is_object() {
            return Err("bad-row".into());
        }
    }
    Ok(())
}

pub fn empty_counts() -> Map<String, Value> {
    let mut counts = Map::new();
    for (key, _) in RECORD_SECTIONS {
        counts.insert((*key).to_string(), json!(0));
    }
    counts
}

fn push_row(out: &mut String, cells: &[&str]) {
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&escape_cell(cell));
    }
    out.push('\n');
}

fn escape_cell(cell: &str) -> String {
    if cell.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cell.push('"');
                } else {
                    quoted = false;
                }
            } else {
                cell.push(ch);
            }
            continue;
        }
        match ch {
            '"' if cell.is_empty() => quoted = true,
            ',' => row.push(std::mem::take(&mut cell)),
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            '\r' => {}
            _ => cell.push(ch),
        }
    }
    if quoted {
        return Err("bad-row".into());
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        json!({
            "schema_version": 1,
            "device_id": "device-a",
            "exported_at": "2026-10-21T00:00:00Z",
            "settings": {"preset": "metric", "weather_place": "Home"},
            "rifles": [{
                "id": "rifle-1",
                "created_at": "2026-10-20T00:00:00Z",
                "updated_at": "2026-10-20T01:00:00Z",
                "revision": 2,
                "deleted_at": null,
                "device_id": "device-a",
                "label": "Rifle, \"one\"",
                "serial": ""
            }],
            "scopes": [],
            "bullets": [],
            "loads": [],
            "atmospheres": [],
            "profiles": [],
            "chronos": [],
            "reticles": [],
            "targets": []
        })
    }

    #[test]
    fn csv_round_trip_keeps_sync_fields_and_quotes() {
        let doc = sample();
        let csv = document_to_csv(&doc).unwrap();
        let again = csv_to_document(&csv).unwrap();
        assert_eq!(again["schema_version"], 1);
        assert_eq!(again["device_id"], "device-a");
        assert_eq!(again["settings"]["preset"], "metric");
        let rifle = &again["rifles"][0];
        assert_eq!(rifle["id"], "rifle-1");
        assert_eq!(rifle["created_at"], "2026-10-20T00:00:00Z");
        assert_eq!(rifle["updated_at"], "2026-10-20T01:00:00Z");
        assert_eq!(rifle["revision"], 2);
        assert!(rifle["deleted_at"].is_null());
        assert_eq!(rifle["device_id"], "device-a");
        assert_eq!(rifle["label"], "Rifle, \"one\"");
    }

    #[test]
    fn a_foreign_csv_is_refused() {
        let err = csv_to_document("Name,BC,Velocity\nHornady,0.243,800\n").unwrap_err();
        assert_eq!(err, "not-holdover");
    }

    #[test]
    fn a_newer_schema_is_refused_whole() {
        let mut doc = sample();
        doc["schema_version"] = json!(2);
        assert_eq!(validate_document(&doc).unwrap_err(), "schema-newer");
        let mut csv = document_to_csv(&sample()).unwrap();
        csv = csv.replacen("holdover-library,1,", "holdover-library,2,", 1);
        assert_eq!(csv_to_document(&csv).unwrap_err(), "schema-newer");
    }

    #[test]
    fn a_repeated_id_is_refused_whole() {
        let mut doc = sample();
        doc["scopes"] = json!([{
            "id": "rifle-1",
            "created_at": "2026-10-20T00:00:00Z",
            "updated_at": "2026-10-20T01:00:00Z",
            "revision": 1,
            "deleted_at": null,
            "device_id": "device-a"
        }]);
        assert_eq!(validate_document(&doc).unwrap_err(), "duplicate:rifle-1");
    }
}
