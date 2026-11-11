//! Two devices, two libraries, a real TLS socket between them.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use holdover_core::{
    FieldStamp, LanChoice, LanCommit, MergeStatus, Side, Stamped, SyncBase, SyncRecord,
};
use holdover_lan::{
    browse, decode_qr_image, join_pair, new_identity, offer_qr_png, open_sync, open_sync_many,
    parse_offer, unix_now, Catalog, ClientSync, Library, PairWait, SyncListener, SERVICE_TYPE,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0,
    0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 0, 3, 1,
    1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

struct Mem {
    id: String,
    schema: i64,
    records: Vec<Stamped>,
    bases: BTreeMap<String, SyncBase>,
    blobs: BTreeMap<String, Vec<u8>>,
    applied: u32,
    fail: bool,
}

impl Mem {
    fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            schema: 1,
            records: Vec::new(),
            bases: BTreeMap::new(),
            blobs: BTreeMap::new(),
            applied: 0,
            fail: false,
        }
    }

    fn add(&mut self, row: Stamped) {
        let base = {
            let mut base = SyncBase::from_record(&row.record);
            base.fields = row.fields.clone();
            base
        };
        self.bases.insert(row.record.id.clone(), base);
        self.records.push(row);
    }

    fn body(&self, id: &str) -> &Value {
        &self
            .records
            .iter()
            .find(|row| row.record.id == id)
            .unwrap()
            .record
            .body
    }
}

impl Catalog for Mem {
    fn schema(&self) -> i64 {
        self.schema
    }

    fn device_id(&self) -> &str {
        &self.id
    }

    fn library(&self) -> Result<Library, holdover_lan::LanError> {
        Ok(Library {
            records: self.records.clone(),
            bases: self.bases.clone(),
        })
    }

    fn blob(&self, hash: &str) -> Result<Option<Vec<u8>>, holdover_lan::LanError> {
        Ok(self.blobs.get(hash).cloned())
    }

    fn apply(
        &mut self,
        commit: &LanCommit,
        blobs: &[(String, Vec<u8>)],
        include_location: bool,
    ) -> Result<(), holdover_lan::LanError> {
        if self.fail {
            return Err(holdover_lan::LanError::new("aborted"));
        }
        for (hash, bytes) in blobs {
            self.blobs.insert(hash.clone(), bytes.clone());
        }
        for write in &commit.writes {
            let local = self
                .records
                .iter()
                .find(|row| row.record.id == write.record.id)
                .map(|row| row.record.body.clone());
            let mut stored = write.clone();
            stored.record.body = holdover_core::merge_private(
                &stored.record.kind,
                local.as_ref(),
                &stored.record.body,
                include_location,
            );
            if let Some(slot) = self
                .records
                .iter_mut()
                .find(|row| row.record.id == stored.record.id)
            {
                *slot = stored;
            } else {
                self.records.push(stored);
            }
        }
        for (id, base) in &commit.bases {
            self.bases.insert(id.clone(), base.clone());
        }
        self.applied += 1;
        Ok(())
    }
}

fn stamp(device: &str, revision: i64, at: &str) -> FieldStamp {
    FieldStamp {
        updated_at: at.into(),
        device_id: device.into(),
        revision,
    }
}

fn rifle(
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

fn ancestor() -> Stamped {
    rifle(
        "rifle-1",
        "desk",
        1,
        "2026-11-01T00:00:01.000Z",
        json!({"label": "Alpha", "notes": "old", "serial": "SECRET", "latitude": 59.3}),
        &[
            ("label", "desk", 1, "2026-11-01T00:00:01.000Z"),
            ("notes", "desk", 1, "2026-11-01T00:00:01.000Z"),
        ],
    )
}

fn exchange(
    mut desk: Mem,
    mut phone: Mem,
    desk_key: &holdover_lan::Identity,
    phone_key: &holdover_lan::Identity,
    decide: impl FnOnce(&holdover_core::LanPreview) -> Result<BTreeMap<String, LanChoice>, String>
        + Send
        + 'static,
) -> (Mem, Mem) {
    let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
    let port = listener.port;
    let pin = desk_key.spki;
    let phone_key = phone_key.clone();
    let server = thread::spawn(move || {
        let mut listener = listener;
        let result = listener.serve_one(&mut desk, Duration::from_secs(5));
        (desk, result.map_err(|err| err.code))
    });
    let addr = format!("127.0.0.1:{port}").parse().unwrap();
    let session = open_sync(
        &mut phone,
        &phone_key,
        addr,
        pin,
        false,
        Duration::from_secs(5),
    )
    .unwrap();
    let choices = decide(&session.preview).unwrap();
    session.finish(&mut phone, &choices).unwrap();
    let (desk, result) = server.join().unwrap();
    result.expect("responder");
    (desk, phone)
}

fn pair_ok() -> (holdover_lan::Peer, holdover_lan::Peer) {
    let desk = new_identity("desk", "Bench").unwrap();
    let phone = new_identity("phone", "Pocket").unwrap();
    let now = unix_now();
    let (mut wait, offer) = PairWait::start(desk.clone(), vec!["127.0.0.1".into()], now).unwrap();
    let parsed = parse_offer(&offer.url()).unwrap();
    assert_eq!(parsed.token, offer.token);
    let png = offer_qr_png(&offer).unwrap();
    let decoded = decode_qr_image(&png).unwrap();
    assert_eq!(parse_offer(&decoded).unwrap().spki_hex, offer.spki_hex);
    let offer2 = offer.clone();
    let phone2 = phone.clone();
    let client = thread::spawn(move || join_pair(&phone2, &offer2, now, Duration::from_secs(5)));
    let pending = wait.next(now, Duration::from_secs(5)).unwrap();
    assert_eq!(pending.name(), "Pocket");
    assert_eq!(pending.fingerprint(), phone.spki_hex);
    let server_peer = pending
        .accept_with(wait.device_name(), wait.device_id())
        .unwrap();
    let client_peer = client.join().unwrap().unwrap();
    assert_eq!(client_peer.spki, desk.spki);
    assert_eq!(server_peer.spki, phone.spki);
    (server_peer, client_peer)
}

#[test]
fn pairing_qr_round_trip_and_rejects_bad_expired_and_replayed_codes() {
    let _ = pair_ok();
    let desk = new_identity("desk", "Bench").unwrap();
    let phone = new_identity("phone", "Pocket").unwrap();
    let now = unix_now();
    let (mut wait, offer) = PairWait::start(desk, vec!["127.0.0.1".into()], now).unwrap();

    let mut bad = offer.clone();
    bad.token = "not-the-token-not-the-token-not-the-token".into();
    let phone_bad = phone.clone();
    let bad_offer = bad.clone();
    let client =
        thread::spawn(move || join_pair(&phone_bad, &bad_offer, now, Duration::from_secs(5)));
    let err = wait.next(now, Duration::from_secs(5)).unwrap_err();
    assert_eq!(err.code, "pair-invalid");
    assert_eq!(client.join().unwrap().unwrap_err().code, "pair-invalid");

    let mut expired = offer.clone();
    expired.expires = now.saturating_sub(5);
    let early = join_pair(&phone, &expired, now, Duration::from_secs(1)).unwrap_err();
    assert_eq!(early.code, "pair-expired");

    let phone_exp = phone.clone();
    let offer_exp = offer.clone();
    // The phone's clock is still inside the code's lifetime. The desktop's is not.
    let client =
        thread::spawn(move || join_pair(&phone_exp, &offer_exp, now, Duration::from_secs(5)));
    let err = wait
        .next(offer.expires, Duration::from_secs(5))
        .unwrap_err();
    assert_eq!(err.code, "pair-expired");
    assert_eq!(client.join().unwrap().unwrap_err().code, "pair-expired");

    let phone_ok = phone.clone();
    let offer_ok = offer.clone();
    let client =
        thread::spawn(move || join_pair(&phone_ok, &offer_ok, now, Duration::from_secs(5)));
    let pending = wait.next(now, Duration::from_secs(5)).unwrap();
    pending
        .accept_with(wait.device_name(), wait.device_id())
        .unwrap();
    client.join().unwrap().unwrap();

    let phone_again = phone.clone();
    let offer_again = offer.clone();
    let client =
        thread::spawn(move || join_pair(&phone_again, &offer_again, now, Duration::from_secs(5)));
    let err = wait.next(now, Duration::from_secs(5)).unwrap_err();
    assert_eq!(err.code, "pair-replayed");
    assert_eq!(client.join().unwrap().unwrap_err().code, "pair-replayed");

    let mut newer = offer.clone();
    newer.schema = 2;
    let err = join_pair(&phone, &newer, now, Duration::from_secs(1)).unwrap_err();
    assert_eq!(err.code, "schema-newer");
}

#[test]
fn a_wrong_server_key_does_not_pair() {
    let desk = new_identity("desk", "Bench").unwrap();
    let other = new_identity("other", "Other").unwrap();
    let phone = new_identity("phone", "Pocket").unwrap();
    let now = unix_now();
    let (mut wait, mut offer) = PairWait::start(desk, vec!["127.0.0.1".into()], now).unwrap();
    offer.spki_hex = other.spki_hex.clone();
    let client = thread::spawn(move || join_pair(&phone, &offer, now, Duration::from_secs(5)));
    let err = wait.next(now, Duration::from_secs(5)).unwrap_err();
    assert_eq!(err.code, "tls");
    let client_err = client.join().unwrap().unwrap_err();
    assert!(
        client_err.code == "pair-pin"
            || client_err.code == "unauthorized"
            || client_err.code == "tls"
    );
}

#[test]
fn independent_fields_tombstones_conflicts_attachments_and_a_second_sync() {
    let desk_key = new_identity("desk", "Bench").unwrap();
    let phone_key = new_identity("phone", "Pocket").unwrap();
    let mut desk = Mem::new("desk");
    let mut phone = Mem::new("phone");
    let base = ancestor();
    desk.add(base.clone());
    phone.add(base.clone());

    desk.records[0].record.revision = 2;
    desk.records[0].record.updated_at = "2026-11-01T00:00:02.000Z".into();
    desk.records[0].record.body["label"] = json!("Desk label");
    desk.records[0].record.device_id = "desk".into();
    desk.records[0]
        .fields
        .insert("label".into(), stamp("desk", 2, "2026-11-01T00:00:02.000Z"));

    phone.records[0].record.revision = 2;
    phone.records[0].record.updated_at = "2026-11-01T00:00:03.000Z".into();
    phone.records[0].record.body["notes"] = json!("Phone note");
    phone.records[0].record.device_id = "phone".into();
    phone.records[0].fields.insert(
        "notes".into(),
        stamp("phone", 2, "2026-11-01T00:00:03.000Z"),
    );

    let hash = hex_encode(&Sha256::digest(PNG));
    phone.blobs.insert(hash.clone(), PNG.to_vec());
    phone.records.push(rifle(
        "group-1",
        "phone",
        1,
        "2026-11-01T00:00:04.000Z",
        json!({"label": "Group", "photo": {"sha256": hash}}),
        &[("label", "phone", 1, "2026-11-01T00:00:04.000Z")],
    ));
    phone.records.last_mut().unwrap().record.kind = "group".into();

    let (desk, phone) = exchange(desk, phone, &desk_key, &phone_key, |_preview| {
        Ok(BTreeMap::new())
    });
    assert_eq!(desk.body("rifle-1")["label"], "Desk label");
    assert_eq!(desk.body("rifle-1")["notes"], "Phone note");
    assert_eq!(desk.body("rifle-1")["serial"], "SECRET");
    assert_eq!(phone.body("rifle-1")["label"], "Desk label");
    assert_eq!(phone.body("rifle-1")["notes"], "Phone note");
    assert_eq!(phone.body("rifle-1")["serial"], "SECRET");
    assert!(
        phone.body("rifle-1").get("latitude").is_none()
            || desk.body("rifle-1").get("latitude").is_some()
    );
    assert_eq!(desk.blobs.get(&hash).map(Vec::as_slice), Some(PNG));
    assert_eq!(
        desk.records
            .iter()
            .find(|row| row.record.id == "group-1")
            .unwrap()
            .record
            .kind,
        "group"
    );

    let applied_before = desk.applied;
    let (desk, phone) = exchange(desk, phone, &desk_key, &phone_key, |preview| {
        assert!(preview
            .rows
            .iter()
            .all(|row| row.status == MergeStatus::Unchanged));
        Ok(BTreeMap::new())
    });
    assert_eq!(desk.body("rifle-1")["label"], "Desk label");
    assert_eq!(phone.body("rifle-1")["notes"], "Phone note");
    let _ = applied_before;

    let mut deleted = phone;
    let row = deleted
        .records
        .iter_mut()
        .find(|row| row.record.id == "rifle-1")
        .unwrap();
    row.record.deleted_at = Some("2026-11-01T00:00:09.000Z".into());
    row.record.revision += 1;
    row.record.updated_at = "2026-11-01T00:00:09.000Z".into();
    let (desk, _phone) = exchange(desk, deleted, &desk_key, &phone_key, |preview| {
        assert_eq!(
            preview
                .rows
                .iter()
                .find(|row| row.id == "rifle-1")
                .unwrap()
                .status,
            MergeStatus::Deleted
        );
        Ok(BTreeMap::new())
    });
    assert!(desk
        .records
        .iter()
        .find(|row| row.record.id == "rifle-1")
        .unwrap()
        .record
        .deleted_at
        .is_some());
}

#[test]
fn a_same_field_conflict_is_resolved_explicitly_and_an_open_conflict_writes_nothing() {
    let desk_key = new_identity("desk", "Bench").unwrap();
    let phone_key = new_identity("phone", "Pocket").unwrap();
    let mut desk = Mem::new("desk");
    let mut phone = Mem::new("phone");
    let base = ancestor();
    desk.add(base.clone());
    phone.add(base);
    desk.records[0].record.body["label"] = json!("Desk");
    desk.records[0].record.revision = 2;
    desk.records[0]
        .fields
        .insert("label".into(), stamp("desk", 2, "2026-11-01T00:00:02.000Z"));
    phone.records[0].record.body["label"] = json!("Phone");
    phone.records[0].record.revision = 2;
    phone.records[0].record.device_id = "phone".into();
    phone.records[0].fields.insert(
        "label".into(),
        stamp("phone", 2, "2026-11-01T00:00:03.000Z"),
    );
    let desk_applied = desk.applied;
    let phone_applied = phone.applied;

    let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
    let port = listener.port;
    let pin = desk_key.spki;
    let server = thread::spawn(move || {
        let mut listener = listener;
        let result = listener.serve_one(&mut desk, Duration::from_secs(5));
        (desk, result.map_err(|err| err.code))
    });
    let addr = format!("127.0.0.1:{port}").parse().unwrap();
    let session = open_sync(
        &mut phone,
        &phone_key,
        addr,
        pin,
        false,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(session.preview.rows[0].status, MergeStatus::Conflict);
    let err = session.finish(&mut phone, &BTreeMap::new()).unwrap_err();
    assert_eq!(err.code, "unresolved");
    let (desk, result) = server.join().unwrap();
    assert_eq!(result.unwrap_err(), "unresolved");
    assert_eq!(desk.applied, desk_applied);
    assert_eq!(phone.applied, phone_applied);
    assert_eq!(desk.body("rifle-1")["label"], "Desk");
    assert_eq!(phone.body("rifle-1")["label"], "Phone");

    let (desk, phone) = exchange(desk, phone, &desk_key, &phone_key, |preview| {
        assert_eq!(preview.rows[0].conflicts[0].field, "label");
        let mut choices = BTreeMap::new();
        choices.insert(
            "rifle-1".into(),
            LanChoice::Fields(BTreeMap::from([("label".into(), Side::Mine)])),
        );
        Ok(choices)
    });
    // The phone opened the sync, so "mine" is the phone's label.
    assert_eq!(desk.body("rifle-1")["label"], "Phone");
    assert_eq!(phone.body("rifle-1")["label"], "Phone");
}

#[test]
fn schema_mismatch_unauthorized_plaintext_and_disconnect_write_nothing() {
    let desk_key = new_identity("desk", "Bench").unwrap();
    let phone_key = new_identity("phone", "Pocket").unwrap();
    let mut desk = Mem::new("desk");
    let mut phone = Mem::new("phone");
    desk.add(ancestor());
    phone.schema = 2;

    let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
    let port = listener.port;
    let pin = desk_key.spki;
    let server = thread::spawn(move || {
        let mut listener = listener;
        let result = listener.serve_one(&mut desk, Duration::from_secs(5));
        (desk, result.map_err(|err| err.code))
    });
    let addr = format!("127.0.0.1:{port}").parse().unwrap();
    let err = open_sync(
        &mut phone,
        &phone_key,
        addr,
        pin,
        false,
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert_eq!(err.code, "schema-newer");
    let (mut desk, result) = server.join().unwrap();
    assert_eq!(result.unwrap_err(), "schema-newer");
    assert_eq!(desk.applied, 0);
    assert_eq!(desk.records.len(), 1);

    let stranger = new_identity("stranger", "No").unwrap();
    let mut phone = Mem::new("phone");
    phone.add(ancestor());
    let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
    let port = listener.port;
    let server = thread::spawn(move || {
        let mut listener = listener;
        let result = listener.serve_one(&mut desk, Duration::from_secs(5));
        (desk, result.map_err(|err| err.code))
    });
    let addr = format!("127.0.0.1:{port}").parse().unwrap();
    let err = open_sync(
        &mut phone,
        &stranger,
        addr,
        desk_key.spki,
        false,
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert_eq!(err.code, "unauthorized");
    let (mut desk, result) = server.join().unwrap();
    assert_eq!(result.unwrap_err(), "unauthorized");
    assert_eq!(desk.applied, 0);

    let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
    let port = listener.port;
    let server = thread::spawn(move || {
        let mut listener = listener;
        let result = listener.serve_one(&mut desk, Duration::from_secs(5));
        (desk, result.map_err(|err| err.code))
    });
    let mut plain = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    plain.write_all(b"{{not-tls").unwrap();
    let _ = plain.shutdown(Shutdown::Both);
    let (mut desk, result) = server.join().unwrap();
    assert_eq!(result.unwrap_err(), "tls");
    assert_eq!(desk.applied, 0);
    assert_eq!(desk.body("rifle-1")["label"], "Alpha");

    let mut phone = Mem::new("phone");
    phone.add(ancestor());
    let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
    let port = listener.port;
    let pin = desk_key.spki;
    let server = thread::spawn(move || {
        let mut listener = listener;
        let result = listener.serve_one(&mut desk, Duration::from_secs(5));
        (desk, result.map_err(|err| err.code))
    });
    let addr = format!("127.0.0.1:{port}").parse().unwrap();
    let session = open_sync(
        &mut phone,
        &phone_key,
        addr,
        pin,
        false,
        Duration::from_secs(5),
    )
    .unwrap();
    drop(session);
    let (desk, result) = server.join().unwrap();
    assert!(result.is_err());
    assert_eq!(desk.applied, 0);
    assert_eq!(phone.applied, 0);
}

#[test]
fn the_bytes_on_the_wire_are_tls_and_do_not_contain_the_record() {
    let desk_key = new_identity("desk", "Bench").unwrap();
    let phone_key = new_identity("phone", "Pocket").unwrap();
    let mut desk = Mem::new("desk");
    let mut phone = Mem::new("phone");
    let mut row = ancestor();
    row.record.body["notes"] = json!("SYNCLABEL-9f3c2a-not-plaintext");
    desk.add(row.clone());
    phone.add(row);
    let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
    let target = listener.port;
    let log = Arc::new(Mutex::new(Vec::new()));
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_port = proxy.local_addr().unwrap().port();
    let log2 = log.clone();
    thread::spawn(move || {
        let (mut client, _) = proxy.accept().unwrap();
        let mut server = TcpStream::connect(format!("127.0.0.1:{target}")).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(5))).ok();
        server.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let mut a = client.try_clone().unwrap();
        let mut b = server.try_clone().unwrap();
        let log_a = log2.clone();
        let log_b = log2.clone();
        let left = thread::spawn(move || copy_log(&mut a, &mut server, &log_a));
        let right = thread::spawn(move || copy_log(&mut b, &mut client, &log_b));
        let _ = left.join();
        let _ = right.join();
    });
    let server = thread::spawn(move || {
        let mut listener = listener;
        let _ = listener.serve_one(&mut desk, Duration::from_secs(5));
    });
    let addr = format!("127.0.0.1:{proxy_port}").parse().unwrap();
    let session = open_sync(
        &mut phone,
        &phone_key,
        addr,
        desk_key.spki,
        false,
        Duration::from_secs(5),
    )
    .unwrap();
    session.finish(&mut phone, &BTreeMap::new()).unwrap();
    let _ = server.join();
    let bytes = log.lock().unwrap().clone();
    assert!(bytes.len() > 32, "expected a TLS transcript");
    assert_eq!(bytes[0], 0x16, "first flight is a TLS handshake, not JSON");
    assert!(!bytes.windows(12).any(|window| window == b"SYNCLABEL-9f"));
    assert!(!bytes.windows(8).any(|window| window == b"\"notes\""));
}

#[test]
fn an_apply_failure_leaves_both_libraries_unchanged() {
    let desk_key = new_identity("desk", "Bench").unwrap();
    let phone_key = new_identity("phone", "Pocket").unwrap();
    let mut desk = Mem::new("desk");
    let mut phone = Mem::new("phone");
    desk.add(ancestor());
    phone.add(rifle(
        "rifle-2",
        "phone",
        1,
        "2026-11-01T00:00:04.000Z",
        json!({"label": "Only phone"}),
        &[("label", "phone", 1, "2026-11-01T00:00:04.000Z")],
    ));
    desk.fail = true;
    let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
    let port = listener.port;
    let pin = desk_key.spki;
    let server = thread::spawn(move || {
        let mut listener = listener;
        let result = listener.serve_one(&mut desk, Duration::from_secs(5));
        (desk, result.map_err(|err| err.code))
    });
    let addr = format!("127.0.0.1:{port}").parse().unwrap();
    let session = open_sync(
        &mut phone,
        &phone_key,
        addr,
        pin,
        false,
        Duration::from_secs(5),
    )
    .unwrap();
    let err = session.finish(&mut phone, &BTreeMap::new()).unwrap_err();
    assert_eq!(err.code, "aborted");
    let (desk, result) = server.join().unwrap();
    assert!(result.is_err());
    assert_eq!(desk.records.len(), 1);
    assert_eq!(phone.records.len(), 1);
    assert!(phone.records.iter().all(|row| row.record.id == "rifle-2"));
}

#[test]
fn a_dead_sync_address_does_not_hide_the_listener() {
    let desk_key = new_identity("desk", "Bench").unwrap();
    let phone_key = new_identity("phone", "Pocket").unwrap();
    let mut phone = Mem::new("phone");
    let mut desk = Mem::new("desk");
    let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
    let port = listener.port;
    let server = thread::spawn(move || {
        let mut listener = listener;
        let _ = listener.serve_one(&mut desk, Duration::from_secs(5));
    });
    let dead: SocketAddr = "192.0.2.1:9".parse().unwrap();
    let live: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let started = Instant::now();
    let session = open_sync_many(
        &mut phone,
        &phone_key,
        &[dead, live],
        desk_key.spki,
        false,
        Duration::from_secs(3),
    )
    .unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "a dead address held the sync for {:?}",
        started.elapsed()
    );
    assert_eq!(session.peer().host, "127.0.0.1");
    assert_eq!(session.peer().port, port);
    drop(session);
    let _ = server.join();
}

#[test]
fn a_closed_port_names_the_address_and_port_zero_is_not_dialed() {
    let phone_key = new_identity("phone", "Pocket").unwrap();
    let desk_key = new_identity("desk", "Bench").unwrap();
    let mut phone = Mem::new("phone");
    let closed = SocketAddr::from(([127, 0, 0, 1], 1));
    let zero = SocketAddr::from(([127, 0, 0, 1], 0));
    let err = open_sync_many(
        &mut phone,
        &phone_key,
        &[zero, closed],
        desk_key.spki,
        false,
        Duration::from_secs(2),
    )
    .unwrap_err();
    assert_eq!(err.code, "sync-unreachable");
    assert!(err.detail.contains("127.0.0.1:1"), "{}", err.detail);
    assert!(err.detail.contains("refused"), "{}", err.detail);
    assert!(!err.detail.contains(":0"), "{}", err.detail);
    let only_zero = open_sync_many(
        &mut phone,
        &phone_key,
        &[zero],
        desk_key.spki,
        false,
        Duration::from_secs(1),
    )
    .unwrap_err();
    assert_eq!(only_zero.detail, "no port");
}

#[test]
fn sync_with_no_address_is_unreachable() {
    let phone_key = new_identity("phone", "Pocket").unwrap();
    let desk_key = new_identity("desk", "Bench").unwrap();
    let mut phone = Mem::new("phone");
    let err = open_sync_many(
        &mut phone,
        &phone_key,
        &[],
        desk_key.spki,
        false,
        Duration::from_secs(1),
    )
    .unwrap_err();
    assert_eq!(err.code, "sync-unreachable");
}

#[test]
fn mdns_advertises_the_fingerprint_and_not_a_pairing_token() {
    let key = new_identity("desk", "Bench").unwrap();
    let host = holdover_lan::local_hosts()
        .into_iter()
        .next()
        .unwrap_or_else(|| "127.0.0.1".into());
    let ad = holdover_lan::advertise(43111, "desk-id", "Bench", &key.spki_hex, &host);
    let Ok(ad) = ad else {
        let err = browse(Duration::from_millis(200)).unwrap_err();
        assert_eq!(err.code, "mdns-unavailable");
        return;
    };
    let found = browse(Duration::from_secs(3)).unwrap();
    ad.stop();
    let hit = found.iter().find(|item| item.spki_hex == key.spki_hex);
    assert!(
        hit.is_some(),
        "mDNS did not resolve the Holdover service; found {found:?}"
    );
    let hit = hit.unwrap();
    assert_eq!(hit.device_id, "desk-id");
    assert_eq!(hit.port, 43111);
    assert!(!format!("{hit:?}").contains("token"));
    let _ = SERVICE_TYPE;
}

fn copy_log(from: &mut TcpStream, to: &mut TcpStream, log: &Mutex<Vec<u8>>) {
    let mut buf = [0u8; 4096];
    loop {
        match from.read(&mut buf) {
            Ok(0) | Err(_) => {
                let _ = to.shutdown(Shutdown::Both);
                break;
            }
            Ok(n) => {
                log.lock().unwrap().extend_from_slice(&buf[..n]);
                if to.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
        }
    }
}

#[test]
fn a_dead_address_does_not_block_the_address_that_is_listening() {
    let desk = new_identity("desk", "Bench").unwrap();
    let phone = new_identity("phone", "Pocket").unwrap();
    let now = unix_now();
    let (mut wait, mut offer) =
        PairWait::start(desk.clone(), vec!["127.0.0.1".into()], now).unwrap();
    offer.hosts = vec!["192.0.2.1".into(), "127.0.0.1".into()];
    let offer_for_client = offer.clone();
    let phone_for_client = phone.clone();
    let started = Instant::now();
    let client = thread::spawn(move || {
        join_pair(
            &phone_for_client,
            &offer_for_client,
            now,
            Duration::from_secs(3),
        )
    });
    let pending = wait.next(now, Duration::from_secs(3)).expect("hello");
    let _server = pending
        .accept_with(wait.device_name(), wait.device_id())
        .unwrap();
    let peer = client.join().unwrap().expect("paired");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the unreachable address consumed the wait"
    );
    assert_eq!(peer.host, "127.0.0.1");
    assert_eq!(phone.name, "Pocket");
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[allow(dead_code)]
fn _keep(session: ClientSync) {
    let _ = session.peer();
}
