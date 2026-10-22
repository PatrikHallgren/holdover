//! Record merge for a Holdover bundle.
//!
//! This module does not read or write files. The desktop shell packs the zip.
//! The same decisions are what a later phone companion and a LAN sync can call:
//! [`classify_pair`] is the field-level rule, and [`classify`] applies it to a
//! whole record using the sync base from the last export or import.

use serde_json::{json, Map, Value};

pub const BUNDLE_FORMAT: &str = "holdover-bundle";
pub const BUNDLE_SCHEMA_VERSION: i64 = 1;

pub const RECORD_KINDS: &[&str] = &[
    "rifle",
    "scope",
    "bullet",
    "load",
    "atmosphere",
    "profile",
    "chrono",
    "reticle",
    "target",
    "document",
    "load_log",
    "group",
    "history",
];

const COORD_KEYS: &[&str] = &[
    "latitude",
    "longitude",
    "grid_latitude",
    "grid_longitude",
    "weather_latitude",
    "weather_longitude",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncError {
    Unresolved,
    KeepBothUnavailable,
    SchemaNewer,
    SchemaOlder,
    Foreign,
    Corrupt,
    DuplicateId(String),
    BadRecord(&'static str),
}

impl SyncError {
    pub fn code(&self) -> String {
        match self {
            Self::Unresolved => "bundle-unresolved".into(),
            Self::KeepBothUnavailable => "bundle-keep-both".into(),
            Self::SchemaNewer => "bundle-schema-newer".into(),
            Self::SchemaOlder => "bundle-schema-older".into(),
            Self::Foreign => "bundle-foreign".into(),
            Self::Corrupt => "bundle-corrupt".into(),
            Self::DuplicateId(id) => format!("bundle-duplicate:{id}"),
            Self::BadRecord(detail) => format!("bundle-bad:{detail}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyncRecord {
    pub id: String,
    pub kind: String,
    pub created_at: String,
    pub updated_at: String,
    pub revision: i64,
    pub deleted_at: Option<String>,
    pub device_id: String,
    pub body: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeenRevision {
    pub device_id: String,
    pub revision: i64,
    pub updated_at: String,
    pub deleted: bool,
}

/// Last reconciled sync identity for one record, plus versions the user rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncBase {
    pub device_id: String,
    pub revision: i64,
    pub updated_at: String,
    pub deleted: bool,
    /// Other devices whose later edits stay conflicts after Keep mine.
    pub devices: Vec<String>,
    /// Exact remote revisions already resolved, so the same bundle is unchanged.
    pub seen: Vec<SeenRevision>,
}

impl SyncBase {
    pub fn from_record(record: &SyncRecord) -> Self {
        Self {
            device_id: record.device_id.clone(),
            revision: record.revision,
            updated_at: record.updated_at.clone(),
            deleted: is_deleted(record),
            devices: Vec::new(),
            seen: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeStatus {
    New,
    Updated,
    Deleted,
    Unchanged,
    Conflict,
}

impl MergeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Updated => "updated",
            Self::Deleted => "deleted",
            Self::Unchanged => "unchanged",
            Self::Conflict => "conflict",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    KeepMine,
    TakeTheirs,
    KeepBoth,
}

impl Resolution {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "keep-mine" => Some(Self::KeepMine),
            "take-theirs" => Some(Self::TakeTheirs),
            "keep-both" => Some(Self::KeepBoth),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeepMine => "keep-mine",
            Self::TakeTheirs => "take-theirs",
            Self::KeepBoth => "keep-both",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MergeRow {
    pub id: String,
    pub kind: String,
    pub status: MergeStatus,
    pub local: Option<SyncRecord>,
    pub remote: SyncRecord,
    pub keep_both: bool,
    pub label: String,
    pub local_label: String,
    pub remote_label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub write: Option<SyncRecord>,
    pub bases: Vec<(String, SyncBase)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyStamp {
    pub id: String,
    pub device_id: String,
    pub now: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BundleDoc {
    pub schema_version: i64,
    pub exported_at: String,
    pub source_device_id: String,
    pub include_location: bool,
    pub place_name: String,
    pub place_latitude: Option<f64>,
    pub place_longitude: Option<f64>,
    pub stale_minutes: Option<u64>,
    pub records: Vec<SyncRecord>,
}

pub fn is_deleted(record: &SyncRecord) -> bool {
    record
        .deleted_at
        .as_ref()
        .map(|text| !text.is_empty())
        .unwrap_or(false)
}

pub fn record_label(record: &SyncRecord) -> String {
    for key in ["label", "title", "name"] {
        if let Some(text) = record.body.get(key).and_then(|value| value.as_str()) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    record.id.chars().take(8).collect()
}

/// Field-level rule shared with a later LAN sync.
///
/// `local_changed` and `remote_changed` are relative to the last reconciled
/// value. A delete on one side and an edit on the other is a conflict when
/// both changed. A one-sided change is an update, or a tombstone when the
/// remote change is a delete.
pub fn classify_pair(
    local_changed: bool,
    remote_changed: bool,
    same: bool,
    local_deleted: bool,
    remote_deleted: bool,
) -> MergeStatus {
    if same || (local_deleted && remote_deleted) {
        return MergeStatus::Unchanged;
    }
    if local_changed && remote_changed {
        return MergeStatus::Conflict;
    }
    if remote_changed {
        if remote_deleted && !local_deleted {
            MergeStatus::Deleted
        } else {
            MergeStatus::Updated
        }
    } else {
        MergeStatus::Unchanged
    }
}

pub fn classify(
    local: Option<&SyncRecord>,
    remote: &SyncRecord,
    base: Option<&SyncBase>,
    strip_location: bool,
) -> MergeStatus {
    let Some(local) = local else {
        return if is_deleted(remote) {
            MergeStatus::Deleted
        } else {
            MergeStatus::New
        };
    };
    if is_deleted(local) && is_deleted(remote) {
        return MergeStatus::Unchanged;
    }
    if same_payload(local, remote, strip_location) {
        return MergeStatus::Unchanged;
    }
    if identity_eq(local, remote) {
        return MergeStatus::Conflict;
    }
    if let Some(base) = base {
        let local_changed = !matches_base(local, base);
        let remote_changed = !matches_base(remote, base);
        if !local_changed && remote_changed {
            if seen_hit(base, remote) {
                return MergeStatus::Unchanged;
            }
            if device_suppressed(base, remote) {
                return MergeStatus::Conflict;
            }
            // A stale copy must not replace the reconciled record.
            if remote.device_id == local.device_id {
                return later_same_device(local, remote);
            }
            // This computer has not edited since the ancestor, so the other
            // computer's copy is the change. An equal clock is still that change.
            if time_before(&remote.updated_at, &local.updated_at) {
                return MergeStatus::Unchanged;
            }
            return classify_pair(false, true, false, is_deleted(local), is_deleted(remote));
        }
        if local_changed && !remote_changed {
            return MergeStatus::Unchanged;
        }
        if local_changed && remote_changed && local.device_id == remote.device_id {
            return later_same_device(local, remote);
        }
        return classify_pair(
            local_changed,
            remote_changed,
            false,
            is_deleted(local),
            is_deleted(remote),
        );
    }
    if local.device_id == remote.device_id {
        return later_same_device(local, remote);
    }
    MergeStatus::Conflict
}

pub fn merge_plan(
    local: &[SyncRecord],
    remote: &[SyncRecord],
    bases: &std::collections::BTreeMap<String, SyncBase>,
    strip_location: bool,
) -> Vec<MergeRow> {
    let local_by_id: std::collections::BTreeMap<&str, &SyncRecord> = local
        .iter()
        .map(|record| (record.id.as_str(), record))
        .collect();
    let mut rows = Vec::new();
    for remote_record in remote {
        let local_record = local_by_id.get(remote_record.id.as_str()).copied();
        let base = bases.get(&remote_record.id);
        let status = classify(local_record, remote_record, base, strip_location);
        let keep_both = status == MergeStatus::Conflict
            && local_record
                .map(|record| keep_both_allowed(record, remote_record))
                .unwrap_or(false);
        let local_label = local_record.map(record_label).unwrap_or_default();
        let remote_label = record_label(remote_record);
        rows.push(MergeRow {
            id: remote_record.id.clone(),
            kind: remote_record.kind.clone(),
            status,
            label: if remote_label.is_empty() {
                local_label.clone()
            } else {
                remote_label.clone()
            },
            local_label,
            remote_label,
            local: local_record.cloned(),
            remote: remote_record.clone(),
            keep_both,
        });
    }
    rows.sort_by(|left, right| {
        status_rank(left.status)
            .cmp(&status_rank(right.status))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });
    rows
}

pub fn choices_for(row: &MergeRow) -> Vec<Resolution> {
    if row.status != MergeStatus::Conflict {
        return Vec::new();
    }
    let mut choices = vec![Resolution::KeepMine, Resolution::TakeTheirs];
    if row.keep_both {
        choices.push(Resolution::KeepBoth);
    }
    choices
}

pub fn effect(
    row: &MergeRow,
    choice: Option<Resolution>,
    copy: Option<&CopyStamp>,
    existing: Option<&SyncBase>,
) -> Result<Effect, SyncError> {
    if matches!(choice, Some(Resolution::KeepBoth)) && !row.keep_both {
        return Err(SyncError::KeepBothUnavailable);
    }
    let resolved = match (row.status, choice) {
        (MergeStatus::Conflict, None) => return Err(SyncError::Unresolved),
        (MergeStatus::Conflict, Some(choice)) => choice,
        (MergeStatus::Unchanged, _) => Resolution::KeepMine,
        (_, Some(Resolution::KeepMine)) => Resolution::KeepMine,
        (_, _) => Resolution::TakeTheirs,
    };
    match resolved {
        Resolution::KeepMine if row.status == MergeStatus::Unchanged => Ok(Effect {
            write: None,
            bases: match existing {
                Some(base) => vec![(row.id.clone(), base.clone())],
                None => anchor_base(row),
            },
        }),
        Resolution::KeepMine => Ok(Effect {
            write: None,
            bases: keep_mine_bases(row),
        }),
        Resolution::KeepBoth => {
            let stamp = copy.ok_or(SyncError::BadRecord("copy"))?;
            let copied = copied_record(&row.remote, stamp);
            let mut bases = keep_mine_bases(row);
            bases.push((copied.id.clone(), SyncBase::from_record(&copied)));
            Ok(Effect {
                write: Some(copied),
                bases,
            })
        }
        Resolution::TakeTheirs => {
            let base = SyncBase::from_record(&row.remote);
            Ok(Effect {
                write: Some(row.remote.clone()),
                bases: vec![(row.id.clone(), base)],
            })
        }
    }
}

pub fn copied_record(remote: &SyncRecord, stamp: &CopyStamp) -> SyncRecord {
    SyncRecord {
        id: stamp.id.clone(),
        kind: remote.kind.clone(),
        created_at: stamp.now.clone(),
        updated_at: stamp.now.clone(),
        revision: 1,
        deleted_at: None,
        device_id: stamp.device_id.clone(),
        body: remote.body.clone(),
    }
}

/// Rifle serials stay on the computer that already has them. Coordinates that
/// were left out of the bundle stay on the local record too.
pub fn merge_private(
    kind: &str,
    local: Option<&Value>,
    incoming: &Value,
    include_location: bool,
) -> Value {
    let mut body = incoming.clone();
    let Some(local) = local else {
        return body;
    };
    if kind == "rifle" {
        let incoming_serial = body
            .get("serial")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let local_serial = local
            .get("serial")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if incoming_serial.is_empty() && !local_serial.is_empty() {
            if let Some(object) = body.as_object_mut() {
                object.insert("serial".into(), json!(local_serial));
            }
        }
    }
    if !include_location {
        restore_coords(local, &mut body);
    }
    body
}

pub fn comparable_body(kind: &str, body: &Value, strip_location: bool) -> Value {
    let mut copy = body.clone();
    if kind == "rifle" {
        if let Some(object) = copy.as_object_mut() {
            object.insert("serial".into(), json!(""));
        }
    }
    if strip_location {
        strip_coords(&mut copy);
    }
    copy
}

pub fn strip_private(kind: &str, body: &mut Value, include_location: bool) {
    if kind == "rifle" {
        if let Some(object) = body.as_object_mut() {
            object.insert("serial".into(), json!(""));
        }
    }
    if !include_location {
        strip_coords(body);
    }
}

pub fn referenced_blobs(records: &[SyncRecord]) -> Vec<String> {
    let mut found = Vec::new();
    for record in records {
        collect_hashes(&record.body, &mut found);
    }
    found.sort();
    found.dedup();
    found
}

pub fn parse_bundle_doc(value: &Value) -> Result<BundleDoc, SyncError> {
    let object = value.as_object().ok_or(SyncError::Foreign)?;
    let format = object
        .get("format")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    if format != BUNDLE_FORMAT {
        return Err(SyncError::Foreign);
    }
    let version = object
        .get("schema_version")
        .and_then(|item| item.as_i64())
        .ok_or(SyncError::Foreign)?;
    if version > BUNDLE_SCHEMA_VERSION {
        return Err(SyncError::SchemaNewer);
    }
    if version < BUNDLE_SCHEMA_VERSION {
        return Err(SyncError::SchemaOlder);
    }
    let records_value = object.get("records").ok_or(SyncError::Corrupt)?;
    let records_array = records_value.as_array().ok_or(SyncError::Corrupt)?;
    let mut records = Vec::new();
    let mut seen_ids = Vec::new();
    for item in records_array {
        let record = parse_record(item)?;
        if seen_ids.iter().any(|id: &String| id == &record.id) {
            return Err(SyncError::DuplicateId(record.id));
        }
        seen_ids.push(record.id.clone());
        records.push(record);
    }
    let place = object.get("place").cloned().unwrap_or(Value::Null);
    let (place_name, place_latitude, place_longitude, stale_minutes) = if place.is_null() {
        (String::new(), None, None, None)
    } else {
        let place_object = place.as_object().ok_or(SyncError::Corrupt)?;
        (
            place_object
                .get("name")
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string(),
            finite_f64(place_object.get("latitude")),
            finite_f64(place_object.get("longitude")),
            place_object
                .get("stale_minutes")
                .and_then(|item| item.as_u64()),
        )
    };
    Ok(BundleDoc {
        schema_version: version,
        exported_at: object
            .get("exported_at")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
        source_device_id: object
            .get("source_device_id")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string(),
        include_location: object
            .get("include_location")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        place_name,
        place_latitude,
        place_longitude,
        stale_minutes,
        records,
    })
}

pub fn bundle_doc_value(doc: &BundleDoc) -> Value {
    let mut place = Map::new();
    place.insert("name".into(), json!(doc.place_name));
    if let Some(minutes) = doc.stale_minutes {
        place.insert("stale_minutes".into(), json!(minutes));
    }
    if doc.include_location {
        place.insert(
            "latitude".into(),
            match doc.place_latitude {
                Some(value) => json!(value),
                None => Value::Null,
            },
        );
        place.insert(
            "longitude".into(),
            match doc.place_longitude {
                Some(value) => json!(value),
                None => Value::Null,
            },
        );
    }
    let records: Vec<Value> = doc.records.iter().map(record_value).collect();
    json!({
        "format": BUNDLE_FORMAT,
        "schema_version": doc.schema_version,
        "exported_at": doc.exported_at,
        "source_device_id": doc.source_device_id,
        "include_location": doc.include_location,
        "place": Value::Object(place),
        "records": records,
    })
}

pub fn record_value(record: &SyncRecord) -> Value {
    json!({
        "id": record.id,
        "kind": record.kind,
        "created_at": record.created_at,
        "updated_at": record.updated_at,
        "revision": record.revision,
        "deleted_at": match &record.deleted_at {
            Some(text) if !text.is_empty() => json!(text),
            _ => Value::Null,
        },
        "device_id": record.device_id,
        "body": record.body,
    })
}

fn parse_record(value: &Value) -> Result<SyncRecord, SyncError> {
    let object = value.as_object().ok_or(SyncError::Corrupt)?;
    let id = required_text(object, "id")?;
    let kind = required_text(object, "kind")?;
    if !RECORD_KINDS.contains(&kind.as_str()) {
        return Err(SyncError::BadRecord("kind"));
    }
    let created_at = required_text(object, "created_at")?;
    let updated_at = required_text(object, "updated_at")?;
    let device_id = required_text(object, "device_id")?;
    let revision = object
        .get("revision")
        .and_then(|item| item.as_i64())
        .filter(|revision| *revision >= 1)
        .ok_or(SyncError::BadRecord("revision"))?;
    let deleted_at = match object.get("deleted_at") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) if text.is_empty() => None,
        Some(Value::String(text)) => Some(text.clone()),
        _ => return Err(SyncError::BadRecord("deleted_at")),
    };
    let body = object
        .get("body")
        .filter(|item| item.is_object())
        .cloned()
        .ok_or(SyncError::BadRecord("body"))?;
    Ok(SyncRecord {
        id,
        kind,
        created_at,
        updated_at,
        revision,
        deleted_at,
        device_id,
        body,
    })
}

fn required_text(object: &Map<String, Value>, key: &'static str) -> Result<String, SyncError> {
    object
        .get(key)
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or(SyncError::BadRecord(key))
}

fn finite_f64(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(|item| item.as_f64())
        .filter(|number| number.is_finite())
}

fn same_payload(local: &SyncRecord, remote: &SyncRecord, strip_location: bool) -> bool {
    local.kind == remote.kind
        && is_deleted(local) == is_deleted(remote)
        && comparable_body(&local.kind, &local.body, strip_location)
            == comparable_body(&remote.kind, &remote.body, strip_location)
}

fn identity_eq(local: &SyncRecord, remote: &SyncRecord) -> bool {
    local.device_id == remote.device_id
        && local.revision == remote.revision
        && local.updated_at == remote.updated_at
        && is_deleted(local) == is_deleted(remote)
}

fn matches_base(record: &SyncRecord, base: &SyncBase) -> bool {
    record.device_id == base.device_id
        && record.revision == base.revision
        && record.updated_at == base.updated_at
        && is_deleted(record) == base.deleted
}

fn seen_hit(base: &SyncBase, remote: &SyncRecord) -> bool {
    base.seen.iter().any(|seen| {
        seen.device_id == remote.device_id
            && seen.revision == remote.revision
            && seen.updated_at == remote.updated_at
            && seen.deleted == is_deleted(remote)
    })
}

fn device_suppressed(base: &SyncBase, remote: &SyncRecord) -> bool {
    remote.device_id != base.device_id && base.devices.iter().any(|id| id == &remote.device_id)
}

fn later_same_device(local: &SyncRecord, remote: &SyncRecord) -> MergeStatus {
    let remote_newer = remote.revision > local.revision
        || (remote.revision == local.revision && time_after(&remote.updated_at, &local.updated_at));
    let local_newer = local.revision > remote.revision
        || (local.revision == remote.revision && time_after(&local.updated_at, &remote.updated_at));
    if remote_newer {
        classify_pair(false, true, false, is_deleted(local), is_deleted(remote))
    } else if local_newer {
        MergeStatus::Unchanged
    } else {
        MergeStatus::Conflict
    }
}

fn keep_both_allowed(local: &SyncRecord, remote: &SyncRecord) -> bool {
    local.kind != "history"
        && remote.kind != "history"
        && local.kind == remote.kind
        && !is_deleted(local)
        && !is_deleted(remote)
}

fn anchor_base(row: &MergeRow) -> Vec<(String, SyncBase)> {
    let record = row.local.as_ref().unwrap_or(&row.remote);
    vec![(row.id.clone(), SyncBase::from_record(record))]
}

fn keep_mine_bases(row: &MergeRow) -> Vec<(String, SyncBase)> {
    let Some(local) = row.local.as_ref() else {
        return Vec::new();
    };
    let mut base = SyncBase::from_record(local);
    if local.device_id != row.remote.device_id {
        base.devices.push(row.remote.device_id.clone());
    }
    base.seen.push(SeenRevision {
        device_id: row.remote.device_id.clone(),
        revision: row.remote.revision,
        updated_at: row.remote.updated_at.clone(),
        deleted: is_deleted(&row.remote),
    });
    vec![(row.id.clone(), base)]
}

fn time_before(left: &str, right: &str) -> bool {
    clock_key(left) < clock_key(right)
}

fn time_after(left: &str, right: &str) -> bool {
    clock_key(left) > clock_key(right)
}

/// Second-precision stamps and millisecond stamps from the same second order
/// with the fraction. A missing fraction is `.000`.
fn clock_key(text: &str) -> String {
    if text.contains('.') {
        return text.to_string();
    }
    match text.strip_suffix('Z') {
        Some(prefix) => format!("{prefix}.000Z"),
        None => format!("{text}.000"),
    }
}

fn status_rank(status: MergeStatus) -> u8 {
    match status {
        MergeStatus::Conflict => 0,
        MergeStatus::Deleted => 1,
        MergeStatus::Updated => 2,
        MergeStatus::New => 3,
        MergeStatus::Unchanged => 4,
    }
}

fn strip_coords(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, _| !COORD_KEYS.contains(&key.as_str()));
            for nested in map.values_mut() {
                strip_coords(nested);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_coords(item);
            }
        }
        _ => {}
    }
}

fn restore_coords(local: &Value, incoming: &mut Value) {
    let (Some(local_map), Some(incoming_map)) = (local.as_object(), incoming.as_object_mut())
    else {
        return;
    };
    for key in COORD_KEYS {
        if !incoming_map.contains_key(*key) {
            if let Some(value) = local_map.get(*key) {
                incoming_map.insert((*key).to_string(), value.clone());
            }
        }
    }
    let keys: Vec<String> = incoming_map.keys().cloned().collect();
    for key in keys {
        if COORD_KEYS.contains(&key.as_str()) {
            continue;
        }
        let Some(local_child) = local_map.get(&key).filter(|value| value.is_object()) else {
            continue;
        };
        if let Some(incoming_child) = incoming_map.get_mut(&key) {
            restore_coords(local_child, incoming_child);
        }
    }
}

fn collect_hashes(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(hash) = map.get("sha256").and_then(|item| item.as_str()) {
                if is_sha256(hash) {
                    found.push(hash.to_ascii_lowercase());
                }
            }
            for nested in map.values() {
                collect_hashes(nested, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_hashes(item, found);
            }
        }
        _ => {}
    }
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        id: &str,
        device: &str,
        revision: i64,
        updated: &str,
        deleted: bool,
        label: &str,
    ) -> SyncRecord {
        SyncRecord {
            id: id.into(),
            kind: "rifle".into(),
            created_at: "2026-10-21T00:00:00Z".into(),
            updated_at: updated.into(),
            revision,
            deleted_at: deleted.then(|| updated.to_string()),
            device_id: device.into(),
            body: json!({"label": label, "serial": "SN"}),
        }
    }

    #[test]
    fn empty_local_is_new_or_a_tombstone() {
        let live = record("1", "a", 1, "2026-10-21T00:00:01Z", false, "Alpha");
        assert_eq!(classify(None, &live, None, true), MergeStatus::New);
        let dead = record("1", "a", 2, "2026-10-21T00:00:02Z", true, "Alpha");
        assert_eq!(classify(None, &dead, None, true), MergeStatus::Deleted);
    }

    #[test]
    fn same_body_is_unchanged_even_when_the_serial_was_stripped() {
        let mut local = record("1", "a", 1, "2026-10-21T00:00:01Z", false, "Alpha");
        local.body["serial"] = json!("SECRET");
        let mut remote = local.clone();
        remote.body["serial"] = json!("");
        assert_eq!(
            classify(Some(&local), &remote, None, true),
            MergeStatus::Unchanged
        );
    }

    #[test]
    fn one_sided_edit_after_a_base_is_an_update() {
        let original = record("1", "a", 1, "2026-10-21T00:00:01Z", false, "Alpha");
        let base = SyncBase::from_record(&original);
        let mut remote = original.clone();
        remote.device_id = "b".into();
        remote.revision = 2;
        remote.updated_at = "2026-10-21T00:00:02Z".into();
        remote.body["label"] = json!("From B");
        assert_eq!(
            classify(Some(&original), &remote, Some(&base), true),
            MergeStatus::Updated
        );
    }

    #[test]
    fn both_sides_editing_is_a_conflict_with_keep_both() {
        let original = record("1", "a", 1, "2026-10-21T00:00:01Z", false, "Alpha");
        let base = SyncBase::from_record(&original);
        let mut local = original.clone();
        local.revision = 2;
        local.updated_at = "2026-10-21T00:00:03Z".into();
        local.body["label"] = json!("Mine");
        let mut remote = original.clone();
        remote.device_id = "b".into();
        remote.revision = 2;
        remote.updated_at = "2026-10-21T00:00:04Z".into();
        remote.body["label"] = json!("Theirs");
        let rows = merge_plan(
            &[local],
            &[remote],
            &std::collections::BTreeMap::from([("1".into(), base)]),
            true,
        );
        assert_eq!(rows[0].status, MergeStatus::Conflict);
        assert!(rows[0].keep_both);
        assert_eq!(
            choices_for(&rows[0]),
            vec![
                Resolution::KeepMine,
                Resolution::TakeTheirs,
                Resolution::KeepBoth
            ]
        );
        assert!(effect(&rows[0], None, None, None).is_err());
    }

    #[test]
    fn newer_tombstone_from_the_same_line_is_deleted() {
        let local = record("1", "a", 1, "2026-10-21T00:00:01Z", false, "Alpha");
        let base = SyncBase::from_record(&local);
        let remote = record("1", "a", 2, "2026-10-21T00:00:05Z", true, "Alpha");
        assert_eq!(
            classify(Some(&local), &remote, Some(&base), true),
            MergeStatus::Deleted
        );
    }

    #[test]
    fn history_conflict_cannot_keep_both() {
        let mut local = record("1", "a", 2, "2026-10-21T00:00:03Z", false, "Mine");
        local.kind = "history".into();
        let mut remote = record("1", "b", 2, "2026-10-21T00:00:04Z", false, "Theirs");
        remote.kind = "history".into();
        let rows = merge_plan(
            &[local],
            &[remote],
            &std::collections::BTreeMap::new(),
            true,
        );
        assert_eq!(rows[0].status, MergeStatus::Conflict);
        assert!(!rows[0].keep_both);
        let stamp = CopyStamp {
            id: "copy".into(),
            device_id: "a".into(),
            now: "2026-10-21T00:00:09Z".into(),
        };
        assert_eq!(
            effect(&rows[0], Some(Resolution::KeepBoth), Some(&stamp), None),
            Err(SyncError::KeepBothUnavailable)
        );
    }

    #[test]
    fn keep_mine_then_the_same_bundle_is_unchanged() {
        let original = record("1", "a", 1, "2026-10-21T00:00:01Z", false, "Alpha");
        let base = SyncBase::from_record(&original);
        let mut local = original.clone();
        local.revision = 2;
        local.updated_at = "2026-10-21T00:00:03Z".into();
        local.body["label"] = json!("Mine");
        let mut remote = original;
        remote.device_id = "b".into();
        remote.revision = 2;
        remote.updated_at = "2026-10-21T00:00:04Z".into();
        remote.body["label"] = json!("Theirs");
        let rows = merge_plan(
            std::slice::from_ref(&local),
            std::slice::from_ref(&remote),
            &std::collections::BTreeMap::from([("1".into(), base)]),
            true,
        );
        let kept = effect(&rows[0], Some(Resolution::KeepMine), None, None).unwrap();
        let next = classify(Some(&local), &remote, Some(&kept.bases[0].1), true);
        assert_eq!(next, MergeStatus::Unchanged);
        let mut later = remote.clone();
        later.revision = 3;
        later.updated_at = "2026-10-21T00:00:06Z".into();
        later.body["label"] = json!("Theirs again");
        assert_eq!(
            classify(Some(&local), &later, Some(&kept.bases[0].1), true),
            MergeStatus::Conflict
        );
    }

    #[test]
    fn a_newer_schema_and_a_foreign_document_are_refused() {
        let mut doc = json!({
            "format": BUNDLE_FORMAT,
            "schema_version": 2,
            "records": []
        });
        assert_eq!(parse_bundle_doc(&doc).unwrap_err(), SyncError::SchemaNewer);
        doc["schema_version"] = json!(1);
        doc["format"] = json!("holdover-library");
        assert_eq!(parse_bundle_doc(&doc).unwrap_err(), SyncError::Foreign);
        doc["format"] = json!(BUNDLE_FORMAT);
        doc["records"] = json!([{"id": "1"}]);
        assert!(matches!(
            parse_bundle_doc(&doc).unwrap_err(),
            SyncError::BadRecord(_)
        ));
    }

    #[test]
    fn a_stale_bundle_does_not_replace_the_reconciled_record() {
        let current = record("1", "a", 2, "2026-10-21T00:00:04Z", false, "Current");
        let base = SyncBase::from_record(&current);
        let stale = record("1", "a", 1, "2026-10-21T00:00:01Z", false, "Old");
        assert_eq!(
            classify(Some(&current), &stale, Some(&base), true),
            MergeStatus::Unchanged
        );
        let mut other = stale.clone();
        other.device_id = "b".into();
        other.updated_at = "2026-10-21T00:00:02Z".into();
        assert_eq!(
            classify(Some(&current), &other, Some(&base), true),
            MergeStatus::Unchanged
        );
    }

    #[test]
    fn private_fields_survive_an_update() {
        let local = json!({"label": "Old", "serial": "SECRET", "grid_latitude": 59.3});
        let incoming = json!({"label": "New", "serial": ""});
        let merged = merge_private("rifle", Some(&local), &incoming, false);
        assert_eq!(merged["label"], "New");
        assert_eq!(merged["serial"], "SECRET");
        assert_eq!(merged["grid_latitude"], 59.3);
        let with_place = merge_private("rifle", Some(&local), &incoming, true);
        assert!(with_place.get("grid_latitude").is_none());
    }
}
