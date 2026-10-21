//! `.holdover` bundle. A zip of versioned JSON, PDF attachments, and a SHA-256
//! manifest. The merge decisions live in `holdover-core`. Nothing is written
//! until the preview is accepted, and the write is one database transaction.
//! Bundle bytes are stored data. They are never executed.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use holdover_core::{
    bundle_doc_value, choices_for, effect, merge_plan, merge_private, parse_bundle_doc,
    referenced_blobs, strip_private, BundleDoc, CopyStamp, Resolution, SeenRevision, SyncBase,
    SyncRecord, BUNDLE_FORMAT, BUNDLE_SCHEMA_VERSION,
};
use rusqlite::params;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::store::{decode_base64, hex_encode, now_iso, Store};

const MAX_ENTRY: u64 = 32 * 1024 * 1024;
const MAX_TOTAL: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 4096;

impl Store {
    pub fn export_bundle_bytes(&self, include_location: bool) -> Result<Vec<u8>, String> {
        let records = load_records(&self.conn)?;
        let mut exported = Vec::with_capacity(records.len());
        for record in &records {
            let mut copy = record.clone();
            strip_private(&copy.kind, &mut copy.body, include_location);
            exported.push(copy);
        }
        exported.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.id.cmp(&right.id))
        });
        let hashes = referenced_blobs(&exported);
        let mut blobs = Vec::new();
        for hash in &hashes {
            let bytes = self.read_blob(hash)?;
            if !bytes.starts_with(b"%PDF") || bytes.len() as u64 > MAX_ENTRY {
                return Err("bundle-missing-pdf".into());
            }
            blobs.push((hash.clone(), bytes));
        }
        let place = self.weather_place()?;
        let doc = BundleDoc {
            schema_version: BUNDLE_SCHEMA_VERSION,
            exported_at: now_iso(),
            source_device_id: self.device_id()?,
            include_location,
            place_name: place.name,
            place_latitude: if include_location {
                place.latitude
            } else {
                None
            },
            place_longitude: if include_location {
                place.longitude
            } else {
                None
            },
            stale_minutes: Some(place.stale_minutes),
            records: exported,
        };
        let library = serde_json::to_vec_pretty(&bundle_doc_value(&doc))
            .map_err(|_| "bundle-corrupt".to_string())?;
        let mut entries = vec![("library.json".to_string(), library)];
        for (hash, bytes) in blobs {
            entries.push((format!("blobs/{hash}"), bytes));
        }
        pack_bundle(&entries)
    }

    /// Remember the current rows as the sync ancestor when this computer has
    /// not published them before. A later local edit keeps the old ancestor, so
    /// a concurrent edit stays a conflict instead of looking like a fast-forward.
    pub fn anchor_bundle_bases(&self) -> Result<(), String> {
        let records = load_records(&self.conn)?;
        let existing = load_bases(&self.conn)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|err| err.to_string())?;
        for record in &records {
            if existing.contains_key(&record.id) {
                continue;
            }
            write_base(&tx, &record.id, &SyncBase::from_record(record))?;
        }
        tx.commit().map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn import_bundle_bytes(
        &self,
        bytes: &[u8],
        resolutions: &BTreeMap<String, Resolution>,
        commit: bool,
        abort_after: Option<usize>,
    ) -> Result<Value, String> {
        let opened = open_entries(bytes)?;
        let doc = document_from_entries(&opened)?;
        let blobs = blobs_from_entries(&opened, &doc)?;
        let local = load_records(&self.conn)?;
        let bases = load_bases(&self.conn)?;
        let strip = !doc.include_location;
        let rows = merge_plan(&local, &doc.records, &bases, strip);
        let local_only = local
            .iter()
            .filter(|record| !doc.records.iter().any(|remote| remote.id == record.id))
            .count();
        let preview = preview_json(&doc, &rows, local_only, false);
        if !commit {
            return Ok(preview);
        }
        let device = self.device_id()?;
        let mut planned = Vec::new();
        for row in &rows {
            let stamp = CopyStamp {
                id: Uuid::new_v4().to_string(),
                device_id: device.clone(),
                now: now_iso(),
            };
            let effect = effect(
                row,
                resolutions.get(&row.id).copied(),
                Some(&stamp),
                bases.get(&row.id),
            )
            .map_err(|err| err.code())?;
            planned.push((row, effect));
        }
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|err| err.to_string())?;
        let mut writes = 0usize;
        for (row, effect) in &planned {
            if let Some(record) = effect.write.clone() {
                if abort_after == Some(writes) {
                    return Err("aborted".into());
                }
                let mut record = record;
                if record.id == row.id {
                    record.body = merge_private(
                        &row.kind,
                        row.local.as_ref().map(|local| &local.body),
                        &record.body,
                        doc.include_location,
                    );
                }
                upsert_record(&tx, &record)?;
                writes += 1;
            }
            for (id, base) in &effect.bases {
                write_base(&tx, id, base)?;
            }
        }
        tx.commit().map_err(|err| err.to_string())?;
        for (hash, blob) in blobs {
            let stored = self.put_blob(&blob)?;
            if stored != hash {
                return Err("bundle-corrupt".into());
            }
        }
        let mut written = preview;
        if let Some(object) = written.as_object_mut() {
            object.insert("written".into(), json!(true));
        }
        Ok(written)
    }
}

pub fn resolutions_from_value(value: &Value) -> Result<BTreeMap<String, Resolution>, String> {
    let Some(object) = value.as_object() else {
        return Err("bundle-bad:choice".into());
    };
    let mut out = BTreeMap::new();
    for (id, choice) in object {
        let text = choice.as_str().ok_or("bundle-bad:choice")?;
        let resolved = Resolution::parse(text).ok_or("bundle-bad:choice")?;
        out.insert(id.clone(), resolved);
    }
    Ok(out)
}

pub fn decode_bundle_bytes(text: &str) -> Result<Vec<u8>, String> {
    decode_base64(text).map_err(|_| "bundle-corrupt".to_string())
}

fn pack_bundle(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut files = Vec::new();
    for (path, bytes) in entries {
        files.push(json!({"path": path, "sha256": sha256(bytes)}));
    }
    let manifest = serde_json::to_vec_pretty(&json!({
        "format": BUNDLE_FORMAT,
        "schema_version": BUNDLE_SCHEMA_VERSION,
        "files": files,
    }))
    .map_err(|_| "bundle-corrupt".to_string())?;
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer
        .start_file("manifest.json", options)
        .map_err(|_| "bundle-corrupt".to_string())?;
    writer
        .write_all(&manifest)
        .map_err(|_| "bundle-corrupt".to_string())?;
    for (path, bytes) in entries {
        writer
            .start_file(path, options)
            .map_err(|_| "bundle-corrupt".to_string())?;
        writer
            .write_all(bytes)
            .map_err(|_| "bundle-corrupt".to_string())?;
    }
    let cursor = writer.finish().map_err(|_| "bundle-corrupt".to_string())?;
    Ok(cursor.into_inner())
}

fn open_entries(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    if bytes.is_empty() {
        return Err("bundle-truncated".into());
    }
    if !looks_like_zip(bytes) {
        return Err("bundle-foreign".into());
    }
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|_| "bundle-truncated".to_string())?;
    if archive.len() > MAX_FILES {
        return Err("bundle-corrupt".into());
    }
    let mut total = 0u64;
    let mut files = BTreeMap::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|_| "bundle-truncated".to_string())?;
        let name = file.name().replace('\\', "/");
        if name.ends_with('/') {
            if name != "blobs/" {
                return Err("bundle-foreign".into());
            }
            continue;
        }
        if !allowed_path(&name) {
            return Err("bundle-foreign".into());
        }
        let mut buf = Vec::new();
        (&mut file)
            .take(MAX_ENTRY + 1)
            .read_to_end(&mut buf)
            .map_err(|_| "bundle-truncated".to_string())?;
        if buf.len() as u64 > MAX_ENTRY {
            return Err("bundle-corrupt".into());
        }
        total += buf.len() as u64;
        if total > MAX_TOTAL {
            return Err("bundle-corrupt".into());
        }
        if files.insert(name.clone(), buf).is_some() {
            return Err("bundle-corrupt".into());
        }
    }
    Ok(files)
}

fn document_from_entries(files: &BTreeMap<String, Vec<u8>>) -> Result<BundleDoc, String> {
    let manifest_bytes = files.get("manifest.json").ok_or("bundle-foreign")?;
    let manifest: Value =
        serde_json::from_slice(manifest_bytes).map_err(|_| "bundle-corrupt".to_string())?;
    let format = manifest
        .get("format")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if format != BUNDLE_FORMAT {
        return Err("bundle-foreign".into());
    }
    let version = manifest
        .get("schema_version")
        .and_then(|value| value.as_i64())
        .ok_or("bundle-foreign")?;
    if version > BUNDLE_SCHEMA_VERSION {
        return Err("bundle-schema-newer".into());
    }
    if version < BUNDLE_SCHEMA_VERSION {
        return Err("bundle-schema-older".into());
    }
    let listed = manifest
        .get("files")
        .and_then(|value| value.as_array())
        .ok_or("bundle-corrupt")?;
    let mut expected = BTreeMap::new();
    for item in listed {
        let path = item
            .get("path")
            .and_then(|value| value.as_str())
            .ok_or("bundle-corrupt")?;
        let hash = item
            .get("sha256")
            .and_then(|value| value.as_str())
            .ok_or("bundle-corrupt")?;
        if !allowed_path(path) || path == "manifest.json" {
            return Err("bundle-foreign".into());
        }
        if expected
            .insert(path.to_string(), hash.to_ascii_lowercase())
            .is_some()
        {
            return Err("bundle-corrupt".into());
        }
    }
    if !expected.contains_key("library.json") {
        return Err("bundle-corrupt".into());
    }
    for key in files.keys() {
        if key == "manifest.json" {
            continue;
        }
        if !expected.contains_key(key) {
            return Err("bundle-foreign".into());
        }
    }
    for (path, hash) in &expected {
        let bytes = files.get(path).ok_or("bundle-corrupt")?;
        if sha256(bytes) != *hash {
            return Err("bundle-corrupt".into());
        }
    }
    let library = files.get("library.json").ok_or("bundle-corrupt")?;
    let value: Value = serde_json::from_slice(library).map_err(|_| "bundle-corrupt".to_string())?;
    parse_bundle_doc(&value).map_err(|err| err.code())
}

fn blobs_from_entries(
    files: &BTreeMap<String, Vec<u8>>,
    doc: &BundleDoc,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let needed = referenced_blobs(&doc.records);
    let mut blobs = Vec::new();
    for hash in &needed {
        let path = format!("blobs/{hash}");
        let bytes = files.get(&path).ok_or("bundle-corrupt")?;
        if sha256(bytes) != *hash || !bytes.starts_with(b"%PDF") {
            return Err("bundle-corrupt".into());
        }
        blobs.push((hash.clone(), bytes.clone()));
    }
    for key in files.keys() {
        if let Some(hash) = key.strip_prefix("blobs/") {
            if !needed.iter().any(|needed| needed == hash) {
                return Err("bundle-foreign".into());
            }
        }
    }
    Ok(blobs)
}

fn preview_json(
    doc: &BundleDoc,
    rows: &[holdover_core::MergeRow],
    local_only: usize,
    written: bool,
) -> Value {
    let mut counts = json!({"new": 0, "updated": 0, "deleted": 0, "unchanged": 0, "conflict": 0});
    let mut rendered = Vec::new();
    for row in rows {
        if let Some(count) = counts.get_mut(row.status.as_str()) {
            *count = json!(count.as_i64().unwrap_or(0) + 1);
        }
        rendered.push(json!({
            "id": row.id,
            "kind": row.kind,
            "status": row.status.as_str(),
            "label": row.label,
            "local_label": row.local_label,
            "remote_label": row.remote_label,
            "keep_both": row.keep_both,
            "choices": choices_for(row).into_iter().map(Resolution::as_str).collect::<Vec<_>>(),
        }));
    }
    json!({
        "written": written,
        "schema_version": doc.schema_version,
        "source_device_id": doc.source_device_id,
        "include_location": doc.include_location,
        "counts": counts,
        "local_only": local_only,
        "rows": rendered,
    })
}

fn load_records(conn: &rusqlite::Connection) -> Result<Vec<SyncRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, created_at, updated_at, revision, deleted_at, device_id, body
             FROM records ORDER BY kind, id",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(|err| err.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let (id, kind, created, updated, revision, deleted, device, body) =
            row.map_err(|err| err.to_string())?;
        let body: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
        out.push(SyncRecord {
            id,
            kind,
            created_at: created,
            updated_at: updated,
            revision,
            deleted_at: deleted.filter(|text| !text.is_empty()),
            device_id: device,
            body,
        });
    }
    Ok(out)
}

fn load_bases(conn: &rusqlite::Connection) -> Result<BTreeMap<String, SyncBase>, String> {
    let mut stmt = conn
        .prepare("SELECT id, device_id, revision, updated_at, deleted, suppress FROM sync_base")
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|err| err.to_string())?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (id, device, revision, updated, deleted, suppress) =
            row.map_err(|err| err.to_string())?;
        out.insert(
            id,
            base_from_parts(&device, revision, &updated, deleted != 0, &suppress),
        );
    }
    Ok(out)
}

fn base_from_parts(
    device: &str,
    revision: i64,
    updated: &str,
    deleted: bool,
    suppress: &str,
) -> SyncBase {
    let parsed: Value = serde_json::from_str(suppress).unwrap_or(json!({}));
    let devices = parsed
        .get("devices")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let seen = parsed
        .get("seen")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(SeenRevision {
                        device_id: item.get("device_id")?.as_str()?.to_string(),
                        revision: item.get("revision")?.as_i64()?,
                        updated_at: item.get("updated_at")?.as_str()?.to_string(),
                        deleted: item
                            .get("deleted")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    SyncBase {
        device_id: device.to_string(),
        revision,
        updated_at: updated.to_string(),
        deleted,
        devices,
        seen,
    }
}

fn write_base(tx: &rusqlite::Transaction<'_>, id: &str, base: &SyncBase) -> Result<(), String> {
    let suppress = json!({
        "devices": base.devices,
        "seen": base.seen.iter().map(|seen| json!({
            "device_id": seen.device_id,
            "revision": seen.revision,
            "updated_at": seen.updated_at,
            "deleted": seen.deleted,
        })).collect::<Vec<_>>(),
    })
    .to_string();
    tx.execute(
        "INSERT INTO sync_base (id, device_id, revision, updated_at, deleted, suppress)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
            device_id = excluded.device_id,
            revision = excluded.revision,
            updated_at = excluded.updated_at,
            deleted = excluded.deleted,
            suppress = excluded.suppress",
        params![
            id,
            base.device_id,
            base.revision,
            base.updated_at,
            base.deleted as i64,
            suppress
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn upsert_record(tx: &rusqlite::Transaction<'_>, record: &SyncRecord) -> Result<(), String> {
    tx.execute("DELETE FROM records WHERE id = ?1", params![record.id])
        .map_err(|err| err.to_string())?;
    let deleted = record
        .deleted_at
        .as_ref()
        .filter(|text| !text.is_empty())
        .cloned();
    tx.execute(
        "INSERT INTO records (id, kind, created_at, updated_at, revision, deleted_at, device_id, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            record.id,
            record.kind,
            record.created_at,
            record.updated_at,
            record.revision,
            deleted,
            record.device_id,
            record.body.to_string()
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn allowed_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains("..") || path.contains('\\') {
        return false;
    }
    path == "manifest.json"
        || path == "library.json"
        || (path.starts_with("blobs/")
            && path.len() == "blobs/".len() + 64
            && path["blobs/".len()..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()))
}

fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
}

fn sha256(bytes: &[u8]) -> String {
    hex_encode(&Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PDF: &[u8] = b"%PDF-1.4\n1 0 obj<</Type/Catalog>>endobj\ntrailer<<>>\n%%EOF\n";

    fn temp_store() -> Store {
        let path = std::env::temp_dir().join(format!(
            "holdover-bundle-{}-{}.db",
            std::process::id(),
            Uuid::new_v4()
        ));
        Store::open(&path).unwrap()
    }

    fn rifle(store: &Store, label: &str, serial: &str) -> Value {
        store
            .save_rifle(&json!({
                "label": label,
                "serial": serial,
                "twist": "1:10in",
                "twist_hand": "right",
                "sight_height": {"text": "45 mm", "unit": "mm"}
            }))
            .unwrap()
    }

    fn rename(store: &Store, id: &str, label: &str, serial: &str) {
        store
            .save_rifle(&json!({
                "id": id,
                "label": label,
                "serial": serial,
                "twist": "1:10in",
                "twist_hand": "right",
                "sight_height": {"text": "45 mm", "unit": "mm"}
            }))
            .unwrap();
    }

    fn encode_base64(bytes: &[u8]) -> String {
        const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        let mut index = 0;
        while index + 3 <= bytes.len() {
            let chunk = ((bytes[index] as u32) << 16)
                | ((bytes[index + 1] as u32) << 8)
                | bytes[index + 2] as u32;
            for shift in [18, 12, 6, 0] {
                out.push(TABLE[((chunk >> shift) & 63) as usize] as char);
            }
            index += 3;
        }
        if index < bytes.len() {
            let mut chunk = (bytes[index] as u32) << 16;
            if index + 1 < bytes.len() {
                chunk |= (bytes[index + 1] as u32) << 8;
            }
            out.push(TABLE[((chunk >> 18) & 63) as usize] as char);
            out.push(TABLE[((chunk >> 12) & 63) as usize] as char);
            if index + 1 < bytes.len() {
                out.push(TABLE[((chunk >> 6) & 63) as usize] as char);
                out.push('=');
            } else {
                out.push('=');
                out.push('=');
            }
        }
        out
    }

    fn published(store: &Store, include_location: bool) -> Vec<u8> {
        let bytes = store.export_bundle_bytes(include_location).unwrap();
        store.anchor_bundle_bases().unwrap();
        bytes
    }

    fn library_json(bytes: &[u8]) -> Value {
        let files = open_entries(bytes).unwrap();
        serde_json::from_slice(files.get("library.json").unwrap()).unwrap()
    }

    fn record_count(store: &Store) -> i64 {
        store
            .conn
            .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
            .unwrap()
    }

    fn stored_body(store: &Store, id: &str) -> Value {
        let raw: String = store
            .conn
            .query_row(
                "SELECT body FROM records WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    #[test]
    fn round_trip_into_an_empty_database_keeps_records_and_blobs() {
        let source = temp_store();
        let saved = rifle(&source, "Alpha", "SECRET");
        let id = saved["id"].as_str().unwrap().to_string();
        let extra = rifle(&source, "Spare", "");
        source
            .delete_record("rifle", extra["id"].as_str().unwrap())
            .unwrap();
        source
            .save_document(&json!({
                "title": "Card",
                "file": {"filename": "card.pdf", "base64": encode_base64(PDF)}
            }))
            .unwrap();
        let source_device = source.device_id().unwrap();
        let bytes = published(&source, false);
        let exported = parse_bundle_doc(&library_json(&bytes)).unwrap();
        assert!(exported
            .records
            .iter()
            .filter(|record| record.kind == "rifle" && record.deleted_at.is_none())
            .all(|record| record.body["serial"] == ""));
        let dest = temp_store();
        let dest_device = dest.device_id().unwrap();
        assert_ne!(dest_device, source_device);
        let preview = dest
            .import_bundle_bytes(&bytes, &BTreeMap::new(), false, None)
            .unwrap();
        assert_eq!(preview["written"], false);
        assert!(preview["counts"]["new"].as_i64().unwrap() >= 1);
        assert!(preview["counts"]["deleted"].as_i64().unwrap() >= 1);
        dest.import_bundle_bytes(&bytes, &BTreeMap::new(), true, None)
            .unwrap();
        assert_eq!(dest.device_id().unwrap(), dest_device);
        let imported = load_records(&dest.conn).unwrap();
        assert_eq!(imported.len(), exported.records.len());
        for (remote, local) in exported.records.iter().zip(imported.iter()) {
            assert_eq!(local.id, remote.id);
            assert_eq!(local.kind, remote.kind);
            assert_eq!(local.created_at, remote.created_at);
            assert_eq!(local.updated_at, remote.updated_at);
            assert_eq!(local.revision, remote.revision);
            assert_eq!(local.deleted_at, remote.deleted_at);
            assert_eq!(local.device_id, remote.device_id);
            assert_eq!(local.body, remote.body);
        }
        assert_eq!(stored_body(&source, &id)["serial"], "SECRET");
        let hash = sha256(PDF);
        assert_eq!(dest.read_blob(&hash).unwrap(), PDF);
        let again = dest
            .import_bundle_bytes(&bytes, &BTreeMap::new(), false, None)
            .unwrap();
        assert_eq!(
            again["counts"]["unchanged"].as_i64().unwrap(),
            imported.len() as i64
        );
        assert_eq!(again["counts"]["new"].as_i64().unwrap(), 0);
    }

    #[test]
    fn coordinates_are_omitted_unless_asked_and_the_local_place_stays() {
        let source = temp_store();
        source
            .set_preferences(&json!({
                "preset": "metric",
                "weather_place": "Bench",
                "weather_latitude": "59.33",
                "weather_longitude": "18.06"
            }))
            .unwrap();
        let hidden = library_json(&source.export_bundle_bytes(false).unwrap());
        assert!(hidden["place"].get("latitude").is_none());
        assert!(hidden["place"].get("longitude").is_none());
        let shown = library_json(&source.export_bundle_bytes(true).unwrap());
        assert_eq!(shown["place"]["latitude"], 59.33);
        assert_eq!(shown["place"]["longitude"], 18.06);
        let dest = temp_store();
        let dest_device = dest.device_id().unwrap();
        dest.set_preferences(&json!({
            "preset": "metric",
            "weather_place": "Home",
            "weather_latitude": "1.5",
            "weather_longitude": "2.5"
        }))
        .unwrap();
        dest.import_bundle_bytes(
            &source.export_bundle_bytes(true).unwrap(),
            &BTreeMap::new(),
            true,
            None,
        )
        .unwrap();
        assert_eq!(dest.device_id().unwrap(), dest_device);
        let place = dest.weather_place().unwrap();
        assert_eq!(place.name, "Home");
        assert!((place.latitude.unwrap() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn a_coordinate_on_a_record_is_left_out_unless_asked() {
        let source = temp_store();
        let saved = rifle(&source, "Alpha", "SN");
        let id = saved["id"].as_str().unwrap();
        let mut body = stored_body(&source, id);
        body["grid_latitude"] = json!(59.3);
        body["grid_longitude"] = json!(18.0);
        source
            .conn
            .execute(
                "UPDATE records SET body = ?1 WHERE id = ?2",
                params![body.to_string(), id],
            )
            .unwrap();
        let hidden =
            parse_bundle_doc(&library_json(&source.export_bundle_bytes(false).unwrap())).unwrap();
        let hidden_rifle = hidden
            .records
            .iter()
            .find(|record| record.id == id)
            .unwrap();
        assert!(hidden_rifle.body.get("grid_latitude").is_none());
        assert_eq!(hidden_rifle.body["serial"], "");
        let shown =
            parse_bundle_doc(&library_json(&source.export_bundle_bytes(true).unwrap())).unwrap();
        let shown_rifle = shown.records.iter().find(|record| record.id == id).unwrap();
        assert_eq!(shown_rifle.body["grid_latitude"], 59.3);
        assert_eq!(stored_body(&source, id)["grid_latitude"], 59.3);
    }

    #[test]
    fn an_update_keeps_the_local_serial() {
        let source = temp_store();
        let saved = rifle(&source, "Alpha", "SECRET");
        let id = saved["id"].as_str().unwrap().to_string();
        let bytes = published(&source, false);
        let dest = temp_store();
        dest.import_bundle_bytes(&bytes, &BTreeMap::new(), true, None)
            .unwrap();
        rename(&dest, &id, "From B", "");
        let update = published(&dest, false);
        let preview = source
            .import_bundle_bytes(&update, &BTreeMap::new(), false, None)
            .unwrap();
        let row = preview["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap();
        assert_eq!(row["status"], "updated");
        source
            .import_bundle_bytes(&update, &BTreeMap::new(), true, None)
            .unwrap();
        let kept = stored_body(&source, &id);
        assert_eq!(kept["label"], "From B");
        assert_eq!(kept["serial"], "SECRET");
    }

    #[test]
    fn a_tombstone_propagates_to_the_other_database() {
        let source = temp_store();
        let saved = rifle(&source, "Alpha", "SECRET");
        let id = saved["id"].as_str().unwrap().to_string();
        let bytes = published(&source, false);
        let dest = temp_store();
        dest.import_bundle_bytes(&bytes, &BTreeMap::new(), true, None)
            .unwrap();
        source.delete_record("rifle", &id).unwrap();
        let tombstone = published(&source, false);
        let preview = dest
            .import_bundle_bytes(&tombstone, &BTreeMap::new(), false, None)
            .unwrap();
        let row = preview["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap();
        assert_eq!(row["status"], "deleted");
        dest.import_bundle_bytes(&tombstone, &BTreeMap::new(), true, None)
            .unwrap();
        let deleted: Option<String> = dest
            .conn
            .query_row(
                "SELECT deleted_at FROM records WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(deleted.unwrap().len() > 8);
    }

    fn conflicted() -> (Store, Vec<u8>, String) {
        let source = temp_store();
        let saved = rifle(&source, "Alpha", "SECRET");
        let id = saved["id"].as_str().unwrap().to_string();
        let bytes = published(&source, false);
        let other = temp_store();
        other
            .import_bundle_bytes(&bytes, &BTreeMap::new(), true, None)
            .unwrap();
        rename(&source, &id, "Mine", "SECRET");
        rename(&other, &id, "Theirs", "");
        let bundle = published(&other, false);
        (source, bundle, id)
    }

    #[test]
    fn the_conflict_preview_offers_keep_mine_theirs_and_both() {
        let (source, bundle, id) = conflicted();
        let preview = source
            .import_bundle_bytes(&bundle, &BTreeMap::new(), false, None)
            .unwrap();
        let row = preview["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap();
        assert_eq!(row["status"], "conflict");
        assert_eq!(row["local_label"], "Mine");
        assert_eq!(row["remote_label"], "Theirs");
        assert_eq!(
            row["choices"],
            json!(["keep-mine", "take-theirs", "keep-both"])
        );
        let before = record_count(&source);
        let error = source
            .import_bundle_bytes(&bundle, &BTreeMap::new(), true, None)
            .unwrap_err();
        assert_eq!(error, "bundle-unresolved");
        assert_eq!(record_count(&source), before);
        assert_eq!(stored_body(&source, &id)["label"], "Mine");
    }

    #[test]
    fn conflict_resolutions_keep_mine_take_theirs_or_both() {
        let (source, bundle, id) = conflicted();
        let mut keep = BTreeMap::new();
        keep.insert(id.clone(), Resolution::KeepMine);
        source
            .import_bundle_bytes(&bundle, &keep, true, None)
            .unwrap();
        assert_eq!(stored_body(&source, &id)["label"], "Mine");
        assert_eq!(stored_body(&source, &id)["serial"], "SECRET");

        let (source, bundle, id) = conflicted();
        let mut take = BTreeMap::new();
        take.insert(id.clone(), Resolution::TakeTheirs);
        source
            .import_bundle_bytes(&bundle, &take, true, None)
            .unwrap();
        assert_eq!(stored_body(&source, &id)["label"], "Theirs");
        assert_eq!(stored_body(&source, &id)["serial"], "SECRET");

        let (source, bundle, id) = conflicted();
        let mut both = BTreeMap::new();
        both.insert(id.clone(), Resolution::KeepBoth);
        source
            .import_bundle_bytes(&bundle, &both, true, None)
            .unwrap();
        let labels: Vec<String> = source
            .live_bodies("rifle")
            .unwrap()
            .into_iter()
            .map(|body| body["label"].as_str().unwrap_or("").to_string())
            .collect();
        assert!(labels.contains(&"Mine".to_string()));
        assert!(labels.contains(&"Theirs".to_string()));
        assert!(labels.len() >= 2);
    }

    #[test]
    fn a_newer_schema_a_foreign_file_and_a_bad_bundle_write_nothing() {
        let store = temp_store();
        rifle(&store, "Alpha", "SN");
        let before = record_count(&store);
        let device = store.device_id().unwrap();
        let newer = pack_named(&[(
            "manifest.json",
            br#"{"format":"holdover-bundle","schema_version":2,"files":[]}"#,
        )]);
        assert_eq!(
            store
                .import_bundle_bytes(&newer, &BTreeMap::new(), true, None)
                .unwrap_err(),
            "bundle-schema-newer"
        );
        assert_eq!(
            store
                .import_bundle_bytes(b"%PDF-1.7 not a bundle", &BTreeMap::new(), true, None)
                .unwrap_err(),
            "bundle-foreign"
        );
        assert_eq!(
            store
                .import_bundle_bytes(
                    br#"{"format":"holdover-library","schema_version":1}"#,
                    &BTreeMap::new(),
                    true,
                    None
                )
                .unwrap_err(),
            "bundle-foreign"
        );
        let foreign_zip = pack_named(&[("notes.txt", b"hello")]);
        assert_eq!(
            store
                .import_bundle_bytes(&foreign_zip, &BTreeMap::new(), true, None)
                .unwrap_err(),
            "bundle-foreign"
        );
        assert_eq!(
            store
                .import_bundle_bytes(b"", &BTreeMap::new(), true, None)
                .unwrap_err(),
            "bundle-truncated"
        );
        let real = published(&store, false);
        let cut = &real[..real.len() / 3];
        assert_eq!(
            store
                .import_bundle_bytes(cut, &BTreeMap::new(), true, None)
                .unwrap_err(),
            "bundle-truncated"
        );
        let bad_manifest = serde_json::to_vec(&json!({
            "format": BUNDLE_FORMAT,
            "schema_version": 1,
            "files": [{"path": "library.json", "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]
        }))
        .unwrap();
        let corrupt = pack_named(&[
            ("manifest.json", &bad_manifest),
            (
                "library.json",
                &serde_json::to_vec(&library_json(&real)).unwrap(),
            ),
        ]);
        assert_eq!(
            store
                .import_bundle_bytes(&corrupt, &BTreeMap::new(), true, None)
                .unwrap_err(),
            "bundle-corrupt"
        );
        assert_eq!(record_count(&store), before);
        assert_eq!(store.device_id().unwrap(), device);
    }

    #[test]
    fn a_failed_import_rolls_back() {
        let source = temp_store();
        rifle(&source, "Alpha", "");
        rifle(&source, "Beta", "");
        let bytes = published(&source, false);
        let dest = temp_store();
        let device = dest.device_id().unwrap();
        let error = dest
            .import_bundle_bytes(&bytes, &BTreeMap::new(), true, Some(1))
            .unwrap_err();
        assert_eq!(error, "aborted");
        assert_eq!(record_count(&dest), 0);
        assert_eq!(dest.device_id().unwrap(), device);
        assert_eq!(
            dest.conn
                .query_row("SELECT COUNT(*) FROM sync_base", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    fn pack_named(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
}
