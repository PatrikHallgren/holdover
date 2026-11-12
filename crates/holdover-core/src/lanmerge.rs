//! Field-level merge for LAN sync.
//!
//! The bundle path still merges a whole record. This module is the rule for
//! two devices that have per-field stamps. An empty stamp map means the record
//! was last saved before those stamps existed, so a record that moved is
//! treated as every field moving together.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::sync::{
    is_deleted, later_time, record_label, FieldStamp, MergeStatus, SyncBase, SyncError, SyncRecord,
    BUNDLE_SCHEMA_VERSION,
};

/// LAN messages carry the same schema number as a bundle. A mismatch refuses
/// the whole exchange.
pub const LAN_SCHEMA: i64 = BUNDLE_SCHEMA_VERSION;

const PRIVATE_KEYS: &[&str] = &[
    "serial",
    "latitude",
    "longitude",
    "grid_latitude",
    "grid_longitude",
    "weather_latitude",
    "weather_longitude",
];

/// Chronograph sessions, groups, and history rows are appended. Their bodies
/// are not merged field by field. Truing history lives on the profile and is
/// split into one field per entry by [`field_values`].
pub fn append_only_kind(kind: &str) -> bool {
    matches!(kind, "chrono" | "group" | "history")
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stamped {
    pub record: SyncRecord,
    pub fields: BTreeMap<String, FieldStamp>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Mine,
    Theirs,
}

impl Side {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "mine" | "keep-mine" => Some(Self::Mine),
            "theirs" | "take-theirs" => Some(Self::Theirs),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mine => "mine",
            Self::Theirs => "theirs",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanChoice {
    KeepMine,
    TakeTheirs,
    KeepBoth { copy_id: String },
    Fields(BTreeMap<String, Side>),
}

impl LanChoice {
    pub fn parse(value: &Value) -> Option<Self> {
        let choice = value.get("choice")?.as_str()?;
        match choice {
            "keep-mine" => Some(Self::KeepMine),
            "take-theirs" => Some(Self::TakeTheirs),
            "keep-both" => Some(Self::KeepBoth {
                copy_id: value.get("copy_id")?.as_str()?.to_string(),
            }),
            "fields" => {
                let map = value.get("fields")?.as_object()?;
                let mut fields = BTreeMap::new();
                for (key, side) in map {
                    fields.insert(key.clone(), Side::parse(side.as_str()?)?);
                }
                Some(Self::Fields(fields))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LanConflict {
    pub field: String,
    pub local_text: String,
    pub remote_text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LanRow {
    pub id: String,
    pub kind: String,
    pub status: MergeStatus,
    /// True when the initiator has the row and the responder does not.
    pub push: bool,
    pub label: String,
    pub local_label: String,
    pub remote_label: String,
    pub keep_both: bool,
    pub conflicts: Vec<LanConflict>,
    pub merged: Option<Stamped>,
    pub local: Option<Stamped>,
    pub remote: Option<Stamped>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LanPreview {
    pub schema: i64,
    pub rows: Vec<LanRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LanCommit {
    pub writes: Vec<Stamped>,
    pub bases: Vec<(String, SyncBase)>,
}

/// Top-level fields, plus one field per truing-history entry.
/// Serials and coordinates are left out. The caller puts those back from the
/// copy that already had them.
pub fn field_values(body: &Value) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let Some(map) = body.as_object() else {
        return out;
    };
    for (key, value) in map {
        if PRIVATE_KEYS.contains(&key.as_str()) {
            continue;
        }
        if key == "truing" {
            flatten_truing(value, &mut out);
        } else {
            out.insert(key.clone(), value.clone());
        }
    }
    out
}

pub fn body_from_fields(fields: &BTreeMap<String, Value>) -> Value {
    let mut body = Map::new();
    let mut history = BTreeMap::new();
    let mut truing = Map::new();
    for (key, value) in fields {
        if let Some(id) = key.strip_prefix("truing.history.") {
            if !id.is_empty() {
                history.insert(id.to_string(), value.clone());
            }
        } else if let Some(sub) = key.strip_prefix("truing.") {
            truing.insert(sub.to_string(), value.clone());
        } else {
            body.insert(key.clone(), value.clone());
        }
    }
    if !history.is_empty() || !truing.is_empty() {
        let rows: Vec<Value> = history.into_values().collect();
        truing.insert("history".into(), Value::Array(rows));
        body.insert("truing".into(), Value::Object(truing));
    }
    Value::Object(body)
}

pub fn lan_preview(
    local: &[Stamped],
    remote: &[Stamped],
    local_bases: &BTreeMap<String, SyncBase>,
    remote_bases: &BTreeMap<String, SyncBase>,
) -> LanPreview {
    let mut ids = BTreeSet::new();
    for row in local.iter().chain(remote.iter()) {
        ids.insert(row.record.id.clone());
    }
    let local_by: BTreeMap<&str, &Stamped> = local
        .iter()
        .map(|row| (row.record.id.as_str(), row))
        .collect();
    let remote_by: BTreeMap<&str, &Stamped> = remote
        .iter()
        .map(|row| (row.record.id.as_str(), row))
        .collect();
    let mut rows = Vec::new();
    for id in ids {
        let local_row = local_by.get(id.as_str()).copied();
        let remote_row = remote_by.get(id.as_str()).copied();
        rows.push(plan_one(
            local_row,
            remote_row,
            local_bases.get(&id),
            remote_bases.get(&id),
        ));
    }
    rows.sort_by(|left, right| {
        status_rank(left.status)
            .cmp(&status_rank(right.status))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });
    LanPreview {
        schema: LAN_SCHEMA,
        rows,
    }
}

pub fn lan_commit(
    preview: &LanPreview,
    choices: &BTreeMap<String, LanChoice>,
) -> Result<LanCommit, SyncError> {
    let mut writes = Vec::new();
    let mut bases = Vec::new();
    for row in &preview.rows {
        if row.status == MergeStatus::Unchanged {
            if let Some(base) = anchor(row) {
                bases.push((row.id.clone(), base));
            }
            continue;
        }
        if row.status != MergeStatus::Conflict {
            let merged = row.merged.clone().ok_or(SyncError::BadRecord("merge"))?;
            bases.push((row.id.clone(), base_from(&merged)));
            writes.push(merged);
            continue;
        }
        let choice = choices.get(&row.id).ok_or(SyncError::Unresolved)?;
        match choice {
            LanChoice::KeepMine => {
                let kept = row.local.clone().ok_or(SyncError::BadRecord("local"))?;
                bases.push((row.id.clone(), base_from(&kept)));
            }
            LanChoice::TakeTheirs => {
                let taken = row.remote.clone().ok_or(SyncError::BadRecord("remote"))?;
                bases.push((row.id.clone(), base_from(&taken)));
                writes.push(taken);
            }
            LanChoice::KeepBoth { copy_id } => {
                if !row.keep_both || copy_id.trim().is_empty() {
                    return Err(SyncError::KeepBothUnavailable);
                }
                let remote = row.remote.clone().ok_or(SyncError::BadRecord("remote"))?;
                let local = row.local.clone().ok_or(SyncError::BadRecord("local"))?;
                let mut copy = remote.record.clone();
                copy.id = copy_id.clone();
                copy.revision = 1;
                copy.deleted_at = None;
                copy.device_id = local.record.device_id.clone();
                copy.created_at = local.record.updated_at.clone();
                copy.updated_at = local.record.updated_at.clone();
                let mut fields = BTreeMap::new();
                let stamp = FieldStamp {
                    updated_at: copy.updated_at.clone(),
                    device_id: copy.device_id.clone(),
                    revision: 1,
                };
                for key in remote.fields.keys() {
                    fields.insert(key.clone(), stamp.clone());
                }
                let copied = Stamped {
                    record: copy,
                    fields,
                };
                bases.push((row.id.clone(), base_from(&local)));
                bases.push((copy_id.clone(), base_from(&copied)));
                writes.push(copied);
            }
            LanChoice::Fields(picks) => {
                let merged = merge_fields(row, picks)?;
                bases.push((row.id.clone(), base_from(&merged)));
                writes.push(merged);
            }
        }
    }
    Ok(LanCommit { writes, bases })
}

fn plan_one(
    local: Option<&Stamped>,
    remote: Option<&Stamped>,
    local_base: Option<&SyncBase>,
    remote_base: Option<&SyncBase>,
) -> LanRow {
    match (local, remote) {
        (None, None) => unreachable!("an id came from a record"),
        (Some(local), None) => one_sided(local, true),
        (None, Some(remote)) => one_sided(remote, false),
        (Some(local), Some(remote)) => both(local, remote, local_base, remote_base),
    }
}

fn one_sided(row: &Stamped, push: bool) -> LanRow {
    let status = if is_deleted(&row.record) {
        MergeStatus::Deleted
    } else if push {
        MergeStatus::Updated
    } else {
        MergeStatus::New
    };
    let merged = if is_deleted(&row.record) && push {
        None
    } else {
        Some(row.clone())
    };
    // A tombstone that only the initiator has must still be written on the
    // responder, so the delete propagates. `merged` stays Some for that.
    let merged = if push && is_deleted(&row.record) {
        Some(row.clone())
    } else {
        merged
    };
    LanRow {
        id: row.record.id.clone(),
        kind: row.record.kind.clone(),
        status,
        push,
        label: record_label(&row.record),
        local_label: if push {
            record_label(&row.record)
        } else {
            String::new()
        },
        remote_label: if push {
            String::new()
        } else {
            record_label(&row.record)
        },
        keep_both: false,
        conflicts: Vec::new(),
        merged,
        local: push.then(|| row.clone()),
        remote: (!push).then(|| row.clone()),
    }
}

fn both(
    local: &Stamped,
    remote: &Stamped,
    local_base: Option<&SyncBase>,
    remote_base: Option<&SyncBase>,
) -> LanRow {
    if local.record.kind != remote.record.kind {
        return conflict_row(local, remote, Vec::new(), false);
    }
    let same = comparable(&local.record) == comparable(&remote.record)
        && is_deleted(&local.record) == is_deleted(&remote.record);
    if same {
        return unchanged(local, remote);
    }
    if append_only_kind(&local.record.kind) {
        return append_only(local, remote, local_base, remote_base);
    }
    let local_fields = values_of(local);
    let remote_fields = values_of(remote);
    let mut keys = BTreeSet::new();
    keys.extend(local_fields.keys().cloned());
    keys.extend(remote_fields.keys().cloned());
    let local_delete = delete_changed(local, local_base);
    let remote_delete = delete_changed(remote, remote_base);
    if is_deleted(&local.record) && is_deleted(&remote.record) {
        return unchanged(local, remote);
    }
    if (local_delete || remote_delete) && (is_deleted(&local.record) || is_deleted(&remote.record))
    {
        let other_edited = if is_deleted(&remote.record) {
            fields_changed(local, local_base, &local_fields)
        } else {
            fields_changed(remote, remote_base, &remote_fields)
        };
        let only_delete =
            (is_deleted(&remote.record) && remote_delete && !local_delete && !other_edited)
                || (is_deleted(&local.record) && local_delete && !remote_delete && !other_edited);
        if only_delete {
            let source = if is_deleted(&remote.record) {
                remote
            } else {
                local
            };
            return LanRow {
                id: local.record.id.clone(),
                kind: local.record.kind.clone(),
                status: MergeStatus::Deleted,
                push: false,
                label: record_label(&local.record),
                local_label: record_label(&local.record),
                remote_label: record_label(&remote.record),
                keep_both: false,
                conflicts: Vec::new(),
                merged: Some(source.clone()),
                local: Some(local.clone()),
                remote: Some(remote.clone()),
            };
        }
        if local_delete != remote_delete {
            return conflict_row(local, remote, Vec::new(), false);
        }
    }
    let mut chosen: BTreeMap<String, Value> = BTreeMap::new();
    let mut chosen_stamps: BTreeMap<String, FieldStamp> = BTreeMap::new();
    let mut conflicts = Vec::new();
    for key in keys {
        let lv = local_fields.get(&key);
        let rv = remote_fields.get(&key);
        if lv == rv {
            if let Some(value) = lv {
                chosen.insert(key.clone(), value.clone());
                chosen_stamps.insert(key.clone(), stamp_of(local, &key));
            }
            continue;
        }
        let local_changed = field_changed(local, local_base, &key);
        let remote_changed = field_changed(remote, remote_base, &key);
        if local_changed && remote_changed {
            conflicts.push(LanConflict {
                field: key.clone(),
                local_text: preview_text(lv),
                remote_text: preview_text(rv),
            });
            continue;
        }
        let (value, stamp) = if remote_changed {
            (rv.cloned(), stamp_of(remote, &key))
        } else {
            (lv.cloned(), stamp_of(local, &key))
        };
        if let Some(value) = value {
            chosen.insert(key.clone(), value);
            chosen_stamps.insert(key, stamp);
        }
    }
    if !conflicts.is_empty() {
        return conflict_row(
            local,
            remote,
            conflicts,
            keep_both_allowed(&local.record, &remote.record),
        );
    }
    let merged = assemble(local, remote, chosen, chosen_stamps);
    LanRow {
        id: local.record.id.clone(),
        kind: local.record.kind.clone(),
        status: MergeStatus::Updated,
        push: false,
        label: record_label(&merged.record),
        local_label: record_label(&local.record),
        remote_label: record_label(&remote.record),
        keep_both: false,
        conflicts: Vec::new(),
        merged: Some(merged),
        local: Some(local.clone()),
        remote: Some(remote.clone()),
    }
}

fn append_only(
    local: &Stamped,
    remote: &Stamped,
    local_base: Option<&SyncBase>,
    remote_base: Option<&SyncBase>,
) -> LanRow {
    let local_changed = !matches_record(&local.record, local_base);
    let remote_changed = !matches_record(&remote.record, remote_base);
    if local_changed && remote_changed {
        return conflict_row(
            local,
            remote,
            vec![LanConflict {
                field: "record".into(),
                local_text: record_label(&local.record),
                remote_text: record_label(&remote.record),
            }],
            false,
        );
    }
    let source = if remote_changed { remote } else { local };
    let status = if is_deleted(&source.record) && !is_deleted(&local.record)
        || is_deleted(&source.record) && remote_changed
    {
        MergeStatus::Deleted
    } else if remote_changed || local_changed {
        MergeStatus::Updated
    } else {
        MergeStatus::Unchanged
    };
    if status == MergeStatus::Unchanged {
        return unchanged(local, remote);
    }
    LanRow {
        id: local.record.id.clone(),
        kind: local.record.kind.clone(),
        status,
        push: false,
        label: record_label(&source.record),
        local_label: record_label(&local.record),
        remote_label: record_label(&remote.record),
        keep_both: false,
        conflicts: Vec::new(),
        merged: Some(source.clone()),
        local: Some(local.clone()),
        remote: Some(remote.clone()),
    }
}

fn unchanged(local: &Stamped, remote: &Stamped) -> LanRow {
    LanRow {
        id: local.record.id.clone(),
        kind: local.record.kind.clone(),
        status: MergeStatus::Unchanged,
        push: false,
        label: record_label(&local.record),
        local_label: record_label(&local.record),
        remote_label: record_label(&remote.record),
        keep_both: false,
        conflicts: Vec::new(),
        merged: None,
        local: Some(local.clone()),
        remote: Some(remote.clone()),
    }
}

fn conflict_row(
    local: &Stamped,
    remote: &Stamped,
    conflicts: Vec<LanConflict>,
    keep_both: bool,
) -> LanRow {
    LanRow {
        id: local.record.id.clone(),
        kind: local.record.kind.clone(),
        status: MergeStatus::Conflict,
        push: false,
        label: record_label(&local.record),
        local_label: record_label(&local.record),
        remote_label: record_label(&remote.record),
        keep_both,
        conflicts,
        merged: None,
        local: Some(local.clone()),
        remote: Some(remote.clone()),
    }
}

fn merge_fields(row: &LanRow, picks: &BTreeMap<String, Side>) -> Result<Stamped, SyncError> {
    let local = row.local.as_ref().ok_or(SyncError::BadRecord("local"))?;
    let remote = row.remote.as_ref().ok_or(SyncError::BadRecord("remote"))?;
    if row.conflicts.is_empty() {
        // Delete versus edit, or two kinds. A field map cannot express that.
        return Err(SyncError::Unresolved);
    }
    if picks.len() != row.conflicts.len()
        || row
            .conflicts
            .iter()
            .any(|conflict| !picks.contains_key(&conflict.field))
    {
        return Err(SyncError::Unresolved);
    }
    let local_fields = values_of(local);
    let remote_fields = values_of(remote);
    let mut keys = BTreeSet::new();
    keys.extend(local_fields.keys().cloned());
    keys.extend(remote_fields.keys().cloned());
    let mut chosen = BTreeMap::new();
    let mut stamps = BTreeMap::new();
    for key in keys {
        let conflict = row.conflicts.iter().any(|item| item.field == key);
        let side = if conflict {
            picks.get(&key).copied().unwrap()
        } else if remote_fields.contains_key(&key) && !local_fields.contains_key(&key) {
            Side::Theirs
        } else {
            Side::Mine
        };
        // For non-conflicts that differ, the preview already auto-merged. The
        // field choice path is only for rows whose every difference is listed
        // or equal. Rebuild equals from local, and conflicts from the pick.
        let (value, stamp_row) = match side {
            Side::Mine => (local_fields.get(&key), local),
            Side::Theirs => (remote_fields.get(&key), remote),
        };
        if !conflict && local_fields.get(&key) != remote_fields.get(&key) {
            // Should have been listed. Refuse rather than guess.
            return Err(SyncError::BadRecord("field"));
        }
        if let Some(value) = value {
            chosen.insert(key.clone(), value.clone());
            stamps.insert(key.clone(), stamp_of(stamp_row, &key));
        }
    }
    Ok(assemble(local, remote, chosen, stamps))
}

fn assemble(
    local: &Stamped,
    remote: &Stamped,
    chosen: BTreeMap<String, Value>,
    stamps: BTreeMap<String, FieldStamp>,
) -> Stamped {
    let mut record = local.record.clone();
    record.body = body_from_fields(&chosen);
    record.kind = local.record.kind.clone();
    record.deleted_at = None;
    let mut latest: Option<&FieldStamp> = None;
    let mut revision = local.record.revision.max(remote.record.revision);
    for stamp in stamps.values() {
        revision = revision.max(stamp.revision);
        latest = Some(match latest {
            Some(current) if later_time(&current.updated_at, &stamp.updated_at) => current,
            _ => stamp,
        });
    }
    if let Some(stamp) = latest {
        record.updated_at = stamp.updated_at.clone();
        record.device_id = stamp.device_id.clone();
    }
    record.revision = revision;
    if later_time(&remote.record.created_at, &local.record.created_at) {
        record.created_at = local.record.created_at.clone();
    } else {
        record.created_at = remote.record.created_at.clone();
    }
    Stamped {
        record,
        fields: stamps,
    }
}

fn anchor(row: &LanRow) -> Option<SyncBase> {
    let stamped = row.local.as_ref().or(row.remote.as_ref())?;
    Some(base_from(stamped))
}

fn base_from(row: &Stamped) -> SyncBase {
    let mut base = SyncBase::from_record(&row.record);
    base.fields = row.fields.clone();
    base
}

fn values_of(row: &Stamped) -> BTreeMap<String, Value> {
    field_values(&row.record.body)
}

fn stamp_of(row: &Stamped, field: &str) -> FieldStamp {
    row.fields.get(field).cloned().unwrap_or(FieldStamp {
        updated_at: row.record.updated_at.clone(),
        device_id: row.record.device_id.clone(),
        revision: row.record.revision,
    })
}

fn field_changed(row: &Stamped, base: Option<&SyncBase>, field: &str) -> bool {
    let Some(base) = base else {
        return true;
    };
    if base.fields.is_empty() {
        return !matches_record(&row.record, Some(base));
    }
    match (row.fields.get(field), base.fields.get(field)) {
        (Some(now), Some(was)) => now != was,
        (None, None) => false,
        _ => true,
    }
}

fn fields_changed(
    row: &Stamped,
    base: Option<&SyncBase>,
    values: &BTreeMap<String, Value>,
) -> bool {
    if base.map(|item| item.fields.is_empty()).unwrap_or(true) {
        return !matches_record(&row.record, base);
    }
    values.keys().any(|key| field_changed(row, base, key))
        || base
            .map(|item| {
                item.fields
                    .keys()
                    .any(|key| !values.contains_key(key) && field_changed(row, Some(item), key))
            })
            .unwrap_or(false)
}

fn delete_changed(row: &Stamped, base: Option<&SyncBase>) -> bool {
    match base {
        Some(base) => is_deleted(&row.record) != base.deleted,
        None => is_deleted(&row.record),
    }
}

fn matches_record(record: &SyncRecord, base: Option<&SyncBase>) -> bool {
    let Some(base) = base else {
        return false;
    };
    record.device_id == base.device_id
        && record.revision == base.revision
        && record.updated_at == base.updated_at
        && is_deleted(record) == base.deleted
}

fn comparable(record: &SyncRecord) -> Value {
    let mut copy = record.body.clone();
    if let Some(map) = copy.as_object_mut() {
        map.retain(|key, _| !PRIVATE_KEYS.contains(&key.as_str()));
    }
    copy
}

fn keep_both_allowed(local: &SyncRecord, remote: &SyncRecord) -> bool {
    local.kind != "history"
        && remote.kind != "history"
        && !append_only_kind(&local.kind)
        && local.kind == remote.kind
        && !is_deleted(local)
        && !is_deleted(remote)
}

fn preview_text(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "removed".into();
    };
    let text = value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string());
    if text.chars().count() > 80 {
        let trimmed: String = text.chars().take(80).collect();
        format!("{trimmed}…")
    } else {
        text
    }
}

fn flatten_truing(value: &Value, out: &mut BTreeMap<String, Value>) {
    let Some(map) = value.as_object() else {
        out.insert("truing".into(), value.clone());
        return;
    };
    for (key, child) in map {
        if key == "history" {
            let Some(rows) = child.as_array() else {
                out.insert("truing.history".into(), child.clone());
                continue;
            };
            for row in rows {
                let id = row.get("id").and_then(Value::as_str).unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                out.insert(format!("truing.history.{id}"), row.clone());
            }
        } else {
            out.insert(format!("truing.{key}"), child.clone());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn stamp(device: &str, revision: i64, at: &str) -> FieldStamp {
        FieldStamp {
            updated_at: at.into(),
            device_id: device.into(),
            revision,
        }
    }

    fn row(
        id: &str,
        device: &str,
        revision: i64,
        at: &str,
        body: Value,
        fields: &[(&str, &str, i64, &str)],
    ) -> Stamped {
        let mut map = BTreeMap::new();
        for (key, who, rev, when) in fields {
            map.insert((*key).to_string(), stamp(who, *rev, when));
        }
        Stamped {
            record: SyncRecord {
                id: id.into(),
                kind: "rifle".into(),
                created_at: "2026-11-01T00:00:00.000Z".into(),
                updated_at: at.into(),
                revision,
                deleted_at: None,
                device_id: device.into(),
                body,
            },
            fields: map,
        }
    }

    fn base_for(row: &Stamped) -> SyncBase {
        base_from(row)
    }

    #[test]
    fn independent_fields_merge_and_the_same_sync_is_unchanged() {
        let ancestor = row(
            "r",
            "desk",
            1,
            "2026-11-01T00:00:01.000Z",
            json!({"label": "Alpha", "notes": "old"}),
            &[
                ("label", "desk", 1, "2026-11-01T00:00:01.000Z"),
                ("notes", "desk", 1, "2026-11-01T00:00:01.000Z"),
            ],
        );
        let base = base_for(&ancestor);
        let mut local = ancestor.clone();
        local.record.revision = 2;
        local.record.updated_at = "2026-11-01T00:00:02.000Z".into();
        local.record.body["label"] = json!("Mine");
        local
            .fields
            .insert("label".into(), stamp("desk", 2, "2026-11-01T00:00:02.000Z"));
        let mut remote = ancestor;
        remote.record.device_id = "phone".into();
        remote.record.revision = 2;
        remote.record.updated_at = "2026-11-01T00:00:03.000Z".into();
        remote.record.body["notes"] = json!("From the phone");
        remote.fields.insert(
            "notes".into(),
            stamp("phone", 2, "2026-11-01T00:00:03.000Z"),
        );
        let bases = BTreeMap::from([("r".into(), base)]);
        let preview = lan_preview(
            std::slice::from_ref(&local),
            std::slice::from_ref(&remote),
            &bases,
            &bases,
        );
        assert_eq!(preview.rows[0].status, MergeStatus::Updated);
        assert!(preview.rows[0].conflicts.is_empty());
        let commit = lan_commit(&preview, &BTreeMap::new()).unwrap();
        let written = &commit.writes[0];
        assert_eq!(written.record.body["label"], "Mine");
        assert_eq!(written.record.body["notes"], "From the phone");
        let next_bases = BTreeMap::from([(written.record.id.clone(), commit.bases[0].1.clone())]);
        let again = lan_preview(
            std::slice::from_ref(written),
            std::slice::from_ref(written),
            &next_bases,
            &next_bases,
        );
        assert_eq!(again.rows[0].status, MergeStatus::Unchanged);
        let second = lan_commit(&again, &BTreeMap::new()).unwrap();
        assert!(second.writes.is_empty());
    }

    #[test]
    fn the_same_field_conflicts_until_someone_chooses() {
        let ancestor = row(
            "r",
            "desk",
            1,
            "2026-11-01T00:00:01.000Z",
            json!({"label": "Alpha", "notes": "old"}),
            &[
                ("label", "desk", 1, "2026-11-01T00:00:01.000Z"),
                ("notes", "desk", 1, "2026-11-01T00:00:01.000Z"),
            ],
        );
        let base = base_for(&ancestor);
        let mut local = ancestor.clone();
        local.record.body["label"] = json!("Mine");
        local.record.revision = 2;
        local
            .fields
            .insert("label".into(), stamp("desk", 2, "2026-11-01T00:00:02.000Z"));
        let mut remote = ancestor;
        remote.record.body["label"] = json!("Theirs");
        remote.record.device_id = "phone".into();
        remote.record.revision = 2;
        remote.fields.insert(
            "label".into(),
            stamp("phone", 2, "2026-11-01T00:00:03.000Z"),
        );
        let bases = BTreeMap::from([("r".into(), base)]);
        let preview = lan_preview(&[local], &[remote], &bases, &bases);
        assert_eq!(preview.rows[0].status, MergeStatus::Conflict);
        assert_eq!(preview.rows[0].conflicts[0].field, "label");
        assert!(lan_commit(&preview, &BTreeMap::new()).is_err());
        let mut picks = BTreeMap::new();
        picks.insert(
            "r".into(),
            LanChoice::Fields(BTreeMap::from([("label".into(), Side::Theirs)])),
        );
        let commit = lan_commit(&preview, &picks).unwrap();
        assert_eq!(commit.writes[0].record.body["label"], "Theirs");
        assert_eq!(commit.writes[0].record.body["notes"], "old");
    }

    #[test]
    fn a_tombstone_applies_and_an_edit_beside_a_delete_conflicts() {
        let ancestor = row(
            "r",
            "desk",
            1,
            "2026-11-01T00:00:01.000Z",
            json!({"label": "Alpha"}),
            &[("label", "desk", 1, "2026-11-01T00:00:01.000Z")],
        );
        let base = base_for(&ancestor);
        let mut deleted = ancestor.clone();
        deleted.record.deleted_at = Some("2026-11-01T00:00:04.000Z".into());
        deleted.record.revision = 2;
        deleted.record.updated_at = "2026-11-01T00:00:04.000Z".into();
        deleted.record.device_id = "phone".into();
        let bases = BTreeMap::from([("r".into(), base.clone())]);
        let preview = lan_preview(
            std::slice::from_ref(&ancestor),
            std::slice::from_ref(&deleted),
            &bases,
            &bases,
        );
        assert_eq!(preview.rows[0].status, MergeStatus::Deleted);
        let mut edited = ancestor.clone();
        edited.record.body["label"] = json!("Still here");
        edited.record.revision = 2;
        edited
            .fields
            .insert("label".into(), stamp("desk", 2, "2026-11-01T00:00:05.000Z"));
        let clash = lan_preview(&[edited], &[deleted], &bases, &bases);
        assert_eq!(clash.rows[0].status, MergeStatus::Conflict);
        assert!(!clash.rows[0].keep_both);
    }

    #[test]
    fn truing_entries_append_and_the_active_fit_can_conflict() {
        let mut ancestor = row(
            "p",
            "desk",
            1,
            "2026-11-01T00:00:01.000Z",
            json!({"label": "Profile", "truing": {"active": {"mv": 800}, "history": []}}),
            &[
                ("label", "desk", 1, "2026-11-01T00:00:01.000Z"),
                ("truing.active", "desk", 1, "2026-11-01T00:00:01.000Z"),
            ],
        );
        ancestor.record.kind = "profile".into();
        let base = base_for(&ancestor);
        let mut local = ancestor.clone();
        local.record.body["truing"]["history"] = json!([{"id": "h1", "summary": "desk"}]);
        local.fields.insert(
            "truing.history.h1".into(),
            stamp("desk", 2, "2026-11-01T00:00:02.000Z"),
        );
        let mut remote = ancestor;
        remote.record.device_id = "phone".into();
        remote.record.body["truing"]["history"] = json!([{"id": "h2", "summary": "phone"}]);
        remote.record.body["truing"]["active"] = json!({"mv": 810});
        remote.fields.insert(
            "truing.history.h2".into(),
            stamp("phone", 2, "2026-11-01T00:00:03.000Z"),
        );
        remote.fields.insert(
            "truing.active".into(),
            stamp("phone", 2, "2026-11-01T00:00:03.000Z"),
        );
        let bases = BTreeMap::from([("p".into(), base)]);
        let preview = lan_preview(&[local.clone()], &[remote.clone()], &bases, &bases);
        assert_eq!(
            preview.rows[0].status,
            MergeStatus::Updated,
            "{:?}",
            preview.rows[0].conflicts
        );
        let commit = lan_commit(&preview, &BTreeMap::new()).unwrap();
        let history = commit.writes[0].record.body["truing"]["history"]
            .as_array()
            .unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(commit.writes[0].record.body["truing"]["active"]["mv"], 810);

        let mut both = local.clone();
        both.record.body["truing"]["active"] = json!({"mv": 790});
        both.fields.insert(
            "truing.active".into(),
            stamp("desk", 3, "2026-11-01T00:00:04.000Z"),
        );
        let clash = lan_preview(&[both], &[remote], &bases, &bases);
        assert!(clash.rows[0]
            .conflicts
            .iter()
            .any(|item| item.field == "truing.active"));
    }

    #[test]
    fn chrono_rows_append_and_a_rewritten_session_conflicts() {
        let mut local = row(
            "c1",
            "desk",
            1,
            "2026-11-01T00:00:01.000Z",
            json!({"label": "String", "shots": [800]}),
            &[("label", "desk", 1, "2026-11-01T00:00:01.000Z")],
        );
        local.record.kind = "chrono".into();
        let mut remote = row(
            "c2",
            "phone",
            1,
            "2026-11-01T00:00:02.000Z",
            json!({"label": "Phone string", "shots": [801]}),
            &[("label", "phone", 1, "2026-11-01T00:00:02.000Z")],
        );
        remote.record.kind = "chrono".into();
        let preview = lan_preview(
            &[local.clone()],
            &[remote.clone()],
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert!(preview
            .rows
            .iter()
            .all(|row| row.status != MergeStatus::Conflict));
        let commit = lan_commit(&preview, &BTreeMap::new()).unwrap();
        assert_eq!(commit.writes.len(), 2);

        let mut rewritten = remote.clone();
        rewritten.record.id = local.record.id.clone();
        rewritten.record.body["shots"] = json!([1]);
        rewritten.record.revision = 2;
        let base = base_for(&local);
        let mut local_edit = local.clone();
        local_edit.record.revision = 2;
        local_edit.record.body["shots"] = json!([2]);
        let bases = BTreeMap::from([(local.record.id.clone(), base)]);
        let clash = lan_preview(&[local_edit], &[rewritten], &bases, &bases);
        assert_eq!(clash.rows[0].status, MergeStatus::Conflict);
    }

    #[test]
    fn a_newer_schema_constant_matches_the_bundle() {
        assert_eq!(LAN_SCHEMA, 1);
        let _ = json!({"schema": LAN_SCHEMA});
    }
}
