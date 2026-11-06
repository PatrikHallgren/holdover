//! Desktop and phone LAN sync. The listener runs only after the person asks.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use holdover_core::{field_values, merge_private, FieldStamp, LanChoice, LanCommit, Stamped};
use holdover_lan::{
    advertise, browse, decode_qr_image, join_pair, local_hosts, new_identity, offer_qr, open_sync,
    parse_offer, preview_value, unix_now, Advertisement, Catalog, ClientSync, Identity, Library,
    PairWait, Peer, SyncListener, PREVIEW_WAIT,
};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

use crate::bundle::{self, load_bases, load_records};
use crate::store::Store;

pub struct LanHub {
    stop: Arc<AtomicBool>,
    shared: Arc<Mutex<LanView>>,
    session: Option<ClientSync>,
    decision: Arc<Mutex<Option<bool>>>,
    _advertise: Option<Advertisement>,
}

struct LanView {
    mode: String,
    offer_url: String,
    offer_svg: String,
    expires: u64,
    port: u16,
    pending_name: String,
    pending_fp: String,
    message: String,
    code: String,
    preview: Option<Value>,
    found: Vec<Value>,
}

impl Default for LanView {
    fn default() -> Self {
        Self {
            mode: "idle".into(),
            offer_url: String::new(),
            offer_svg: String::new(),
            expires: 0,
            port: 0,
            pending_name: String::new(),
            pending_fp: String::new(),
            message: String::new(),
            code: String::new(),
            preview: None,
            found: Vec::new(),
        }
    }
}

impl Default for LanHub {
    fn default() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            shared: Arc::new(Mutex::new(LanView::default())),
            session: None,
            decision: Arc::new(Mutex::new(None)),
            _advertise: None,
        }
    }
}

impl LanHub {
    pub fn status(&self, store: &Store) -> Result<Value, String> {
        let view = self
            .shared
            .lock()
            .map_err(|_| "the sync state is busy".to_string())?;
        let peers = store.lan_peers()?;
        Ok(json!({
            "mode": view.mode,
            "offer_url": view.offer_url,
            "offer_svg": view.offer_svg,
            "expires": view.expires,
            "port": view.port,
            "pending_name": view.pending_name,
            "pending_fingerprint": view.pending_fp,
            "message": view.message,
            "code": view.code,
            "preview": view.preview,
            "found": view.found,
            "name": store.lan_name()?,
            "peers": peers,
            "hosts": local_hosts(),
        }))
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.session.take();
        self._advertise.take();
        if let Ok(mut view) = self.shared.lock() {
            *view = LanView::default();
        }
        multicast(false);
    }

    pub fn set_name(&mut self, store: &Store, name: &str) -> Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.chars().count() > 40 {
            return Err("lan-name".into());
        }
        store.set_meta("lan_name", trimmed)
    }

    pub fn pair_start(&mut self, store: Arc<Mutex<Store>>) -> Result<(), String> {
        self.stop();
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = stop.clone();
        let (identity, hosts) = {
            let guard = store
                .lock()
                .map_err(|_| "the library is busy".to_string())?;
            (guard.lan_identity()?, local_hosts())
        };
        let (wait, offer) =
            PairWait::start(identity.clone(), hosts, unix_now()).map_err(|err| err.code)?;
        let svg = offer_qr(&offer).map_err(|err| err.code)?;
        {
            let mut view = self
                .shared
                .lock()
                .map_err(|_| "the sync state is busy".to_string())?;
            view.mode = "pair".into();
            view.offer_url = offer.url();
            view.offer_svg = svg;
            view.expires = offer.expires;
            view.port = offer.port;
            view.message = String::new();
            view.code = String::new();
        }
        let shared = self.shared.clone();
        let decision = self.decision.clone();
        *decision
            .lock()
            .map_err(|_| "the sync state is busy".to_string())? = None;
        let ad = advertise(
            offer.port,
            &identity.device_id,
            &identity.name,
            &identity.spki_hex,
            &local_hosts()[0],
        )
        .ok();
        self._advertise = ad;
        multicast(true);
        thread::spawn(move || pair_loop(store, wait, shared, decision, stop, identity));
        Ok(())
    }

    pub fn pair_join(&mut self, store: Arc<Mutex<Store>>, code: &str) -> Result<(), String> {
        self.stop();
        let offer = parse_offer(code).map_err(|err| err.code)?;
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = stop.clone();
        let identity = {
            let guard = store
                .lock()
                .map_err(|_| "the library is busy".to_string())?;
            guard.lan_identity()?
        };
        {
            let mut view = self
                .shared
                .lock()
                .map_err(|_| "the sync state is busy".to_string())?;
            view.mode = "joining".into();
            view.message = String::new();
            view.code = String::new();
        }
        let shared = self.shared.clone();
        thread::spawn(move || {
            let result = join_pair(&identity, &offer, unix_now(), Duration::from_secs(20));
            let mut view = match shared.lock() {
                Ok(view) => view,
                Err(_) => return,
            };
            match result {
                Ok(peer) => {
                    if let Ok(guard) = store.lock() {
                        let _ = guard.save_peer(&peer);
                    }
                    view.mode = "paired".into();
                    view.message = peer.name;
                    view.code = String::new();
                }
                Err(err) => {
                    view.mode = "idle".into();
                    view.code = err.code;
                }
            }
        });
        let _ = stop;
        Ok(())
    }

    pub fn pair_decide(&mut self, accept: bool) -> Result<(), String> {
        let mut slot = self
            .decision
            .lock()
            .map_err(|_| "the sync state is busy".to_string())?;
        if self
            .shared
            .lock()
            .map_err(|_| "the sync state is busy".to_string())?
            .pending_name
            .is_empty()
        {
            return Err("lan-no-pending".into());
        }
        *slot = Some(accept);
        Ok(())
    }

    pub fn listen(&mut self, store: Arc<Mutex<Store>>) -> Result<(), String> {
        self.stop();
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = stop.clone();
        let (identity, pins) = {
            let guard = store
                .lock()
                .map_err(|_| "the library is busy".to_string())?;
            (guard.lan_identity()?, guard.lan_pins()?)
        };
        if pins.is_empty() {
            return Err("lan-no-peer".into());
        }
        let listener = SyncListener::bind(identity.clone(), pins).map_err(|err| err.code)?;
        let port = listener.port;
        {
            let mut view = self
                .shared
                .lock()
                .map_err(|_| "the sync state is busy".to_string())?;
            view.mode = "listen".into();
            view.port = port;
            view.code = String::new();
            view.message = String::new();
        }
        let host = local_hosts()
            .into_iter()
            .next()
            .unwrap_or_else(|| "127.0.0.1".into());
        self._advertise = advertise(
            port,
            &identity.device_id,
            &identity.name,
            &identity.spki_hex,
            &host,
        )
        .ok();
        multicast(true);
        let shared = self.shared.clone();
        thread::spawn(move || listen_loop(store, listener, shared, stop));
        Ok(())
    }

    pub fn discover(&mut self, store: &Store) -> Result<(), String> {
        let peers = store.lan_peers()?;
        let found = match browse(Duration::from_millis(1500)) {
            Ok(rows) => rows
                .into_iter()
                .map(|row| {
                    let paired = peers
                        .as_array()
                        .map(|items| items.iter().any(|peer| peer["spki"] == row.spki_hex))
                        .unwrap_or(false);
                    json!({
                        "name": row.name,
                        "host": row.host,
                        "port": row.port,
                        "device_id": row.device_id,
                        "spki": row.spki_hex,
                        "paired": paired,
                    })
                })
                .collect(),
            Err(err) => {
                let mut view = self
                    .shared
                    .lock()
                    .map_err(|_| "the sync state is busy".to_string())?;
                view.code = err.code;
                view.found.clear();
                return Ok(());
            }
        };
        let mut view = self
            .shared
            .lock()
            .map_err(|_| "the sync state is busy".to_string())?;
        view.found = found;
        view.code = String::new();
        Ok(())
    }

    pub fn sync_begin(
        &mut self,
        store: Arc<Mutex<Store>>,
        host: &str,
        port: u16,
        spki: &str,
        include_location: bool,
    ) -> Result<(), String> {
        self.session.take();
        let identity = {
            let guard = store
                .lock()
                .map_err(|_| "the library is busy".to_string())?;
            guard.lan_identity()?
        };
        let pin = spki_pin(spki)?;
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|_| "lan-address".to_string())?;
        let mut catalog = ArcCatalog::new(store.clone())?;
        let session = open_sync(
            &mut catalog,
            &identity,
            addr,
            pin,
            include_location,
            Duration::from_secs(8),
        )
        .map_err(|err| err.code)?;
        let preview = preview_value(&session.preview);
        self.session = Some(session);
        let mut view = self
            .shared
            .lock()
            .map_err(|_| "the sync state is busy".to_string())?;
        view.mode = "preview".into();
        view.preview = Some(preview);
        view.code = String::new();
        if let Ok(guard) = store.lock() {
            let _ = guard.remember_peer_addr(spki, host, port);
        }
        Ok(())
    }

    pub fn sync_apply(&mut self, store: Arc<Mutex<Store>>, choices: &Value) -> Result<(), String> {
        let session = self.session.take().ok_or("lan-no-preview")?;
        let parsed = parse_choice_map(choices)?;
        let mut catalog = ArcCatalog::new(store)?;
        session
            .finish(&mut catalog, &parsed)
            .map_err(|err| err.code)?;
        let mut view = self
            .shared
            .lock()
            .map_err(|_| "the sync state is busy".to_string())?;
        view.mode = "done".into();
        view.preview = None;
        view.code = String::new();
        view.message = "applied".into();
        Ok(())
    }

    pub fn session_cancel(&mut self) {
        self.session.take();
        if let Ok(mut view) = self.shared.lock() {
            view.preview = None;
            if view.mode == "preview" {
                view.mode = "idle".into();
            }
        }
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<String, String> {
        decode_qr_image(bytes).map_err(|err| err.code)
    }
}

fn spki_pin(spki: &str) -> Result<[u8; 32], String> {
    // Reuse the offer parser's hex check via a one-field offer would drag a token in.
    // pin_from_hex is crate-private, so decode here.
    if spki.len() != 64 || !spki.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("unauthorized".into());
    }
    let mut pin = [0u8; 32];
    let bytes = spki.as_bytes();
    for index in 0..32 {
        let hi = (bytes[index * 2] as char).to_digit(16).unwrap();
        let lo = (bytes[index * 2 + 1] as char).to_digit(16).unwrap();
        pin[index] = ((hi << 4) | lo) as u8;
    }
    Ok(pin)
}

fn pair_loop(
    store: Arc<Mutex<Store>>,
    mut wait: PairWait,
    shared: Arc<Mutex<LanView>>,
    decision: Arc<Mutex<Option<bool>>>,
    stop: Arc<AtomicBool>,
    identity: Identity,
) {
    while !stop.load(Ordering::SeqCst) && unix_now() < wait.offer().expires {
        match wait.next(unix_now(), Duration::from_secs(1)) {
            Ok(pending) => {
                if let Ok(mut view) = shared.lock() {
                    view.pending_name = pending.name().to_string();
                    view.pending_fp = pending.fingerprint();
                    view.code = String::new();
                }
                let answer = wait_decision(&decision, &stop);
                if let Ok(mut view) = shared.lock() {
                    view.pending_name.clear();
                    view.pending_fp.clear();
                }
                match answer {
                    Some(true) => match pending
                        .accept_with(identity.name.as_str(), identity.device_id.as_str())
                    {
                        Ok(peer) => {
                            if let Ok(guard) = store.lock() {
                                let _ = guard.save_peer(&peer);
                            }
                            if let Ok(mut view) = shared.lock() {
                                view.mode = "paired".into();
                                view.message = peer.name;
                                view.offer_svg.clear();
                                view.offer_url.clear();
                            }
                            return;
                        }
                        Err(err) => {
                            if let Ok(mut view) = shared.lock() {
                                view.code = err.code;
                            }
                        }
                    },
                    Some(false) => {
                        let _ = pending.reject();
                    }
                    None => {
                        let _ = pending.reject();
                        return;
                    }
                }
            }
            Err(err) if err.code == "disconnected" => {}
            Err(err) => {
                if let Ok(mut view) = shared.lock() {
                    view.code = err.code;
                }
            }
        }
    }
    if let Ok(mut view) = shared.lock() {
        if view.mode == "pair" {
            view.mode = "idle".into();
            if view.code.is_empty() && unix_now() >= wait.offer().expires {
                view.code = "pair-expired".into();
            }
        }
    }
}

fn wait_decision(decision: &Mutex<Option<bool>>, stop: &AtomicBool) -> Option<bool> {
    let start = std::time::Instant::now();
    while start.elapsed() < PREVIEW_WAIT {
        if stop.load(Ordering::SeqCst) {
            return None;
        }
        if let Ok(mut slot) = decision.lock() {
            if let Some(answer) = slot.take() {
                return Some(answer);
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    None
}

fn listen_loop(
    store: Arc<Mutex<Store>>,
    mut listener: SyncListener,
    shared: Arc<Mutex<LanView>>,
    stop: Arc<AtomicBool>,
) {
    let mut catalog = match ArcCatalog::new(store) {
        Ok(catalog) => catalog,
        Err(_) => return,
    };
    while !stop.load(Ordering::SeqCst) {
        match listener.serve_one(&mut catalog, Duration::from_secs(1)) {
            Ok(()) => {
                if let Ok(mut view) = shared.lock() {
                    view.message = "applied".into();
                    view.code = String::new();
                }
            }
            Err(err) if err.code == "disconnected" => {}
            Err(err) => {
                if let Ok(mut view) = shared.lock() {
                    view.code = err.code;
                }
            }
        }
    }
    if let Ok(mut view) = shared.lock() {
        if view.mode == "listen" {
            view.mode = "idle".into();
        }
    }
}

struct ArcCatalog {
    store: Arc<Mutex<Store>>,
    schema: i64,
    device_id: String,
}

impl ArcCatalog {
    fn new(store: Arc<Mutex<Store>>) -> Result<Self, String> {
        let device_id = store
            .lock()
            .map_err(|_| "the library is busy".to_string())?
            .device_id()?;
        Ok(Self {
            store,
            schema: 1,
            device_id,
        })
    }
}

impl Catalog for ArcCatalog {
    fn schema(&self) -> i64 {
        self.schema
    }

    fn device_id(&self) -> &str {
        &self.device_id
    }

    fn library(&self) -> Result<Library, holdover_lan::LanError> {
        let guard = self
            .store
            .lock()
            .map_err(|_| holdover_lan::LanError::new("busy"))?;
        guard.lan_library().map_err(holdover_lan::LanError::new)
    }

    fn blob(&self, hash: &str) -> Result<Option<Vec<u8>>, holdover_lan::LanError> {
        let guard = self
            .store
            .lock()
            .map_err(|_| holdover_lan::LanError::new("busy"))?;
        match guard.read_blob(hash) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(_) => Ok(None),
        }
    }

    fn apply(
        &mut self,
        commit: &LanCommit,
        blobs: &[(String, Vec<u8>)],
        include_location: bool,
    ) -> Result<(), holdover_lan::LanError> {
        let guard = self
            .store
            .lock()
            .map_err(|_| holdover_lan::LanError::new("busy"))?;
        guard
            .lan_apply(commit, blobs, include_location)
            .map_err(holdover_lan::LanError::new)
    }
}

fn multicast(on: bool) {
    crate::phone::multicast(on);
}

fn parse_choice_map(value: &Value) -> Result<BTreeMap<String, LanChoice>, String> {
    let mut out = BTreeMap::new();
    let Some(map) = value.as_object() else {
        return Ok(out);
    };
    for (id, choice) in map {
        let parsed = LanChoice::parse(choice).ok_or("unresolved")?;
        out.insert(id.clone(), parsed);
    }
    Ok(out)
}

impl Store {
    pub fn lan_name(&self) -> Result<String, String> {
        if let Some(name) = self.meta_get("lan_name")? {
            if !name.trim().is_empty() {
                return Ok(name);
            }
        }
        Ok(if cfg!(target_os = "android") {
            "This phone".into()
        } else {
            "This computer".into()
        })
    }

    pub fn lan_identity(&self) -> Result<Identity, String> {
        let name = self.lan_name()?;
        if let (Some(cert), Some(key)) = (self.meta_get("lan_cert")?, self.meta_get("lan_key")?) {
            return holdover_lan::identity_from_parts(&self.device_id()?, &name, &cert, &key)
                .map_err(|err| err.code);
        }
        let identity = new_identity(&self.device_id()?, &name).map_err(|err| err.code)?;
        self.set_meta("lan_cert", &identity.cert_b64())?;
        self.set_meta("lan_key", &identity.key_b64())?;
        Ok(identity)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), String> {
        self.meta_put(key, value)
    }

    pub fn lan_peers(&self) -> Result<Value, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT spki, device_id, name, host, port FROM lan_peer ORDER BY name, spki")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "spki": row.get::<_, String>(0)?,
                    "device_id": row.get::<_, String>(1)?,
                    "name": row.get::<_, String>(2)?,
                    "host": row.get::<_, String>(3)?,
                    "port": row.get::<_, u16>(4)?,
                }))
            })
            .map_err(|err| err.to_string())?;
        let mut peers = Vec::new();
        for row in rows {
            peers.push(row.map_err(|err| err.to_string())?);
        }
        Ok(json!(peers))
    }

    pub fn lan_pins(&self) -> Result<Vec<[u8; 32]>, String> {
        let peers = self.lan_peers()?;
        let mut pins = Vec::new();
        for peer in peers.as_array().into_iter().flatten() {
            let spki = peer["spki"].as_str().unwrap_or("");
            if let Ok(pin) = spki_pin(spki) {
                pins.push(pin);
            }
        }
        Ok(pins)
    }

    pub fn save_peer(&self, peer: &Peer) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO lan_peer (spki, device_id, name, host, port, paired_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(spki) DO UPDATE SET
                    device_id = excluded.device_id,
                    name = excluded.name,
                    host = excluded.host,
                    port = CASE WHEN excluded.port = 0 THEN lan_peer.port ELSE excluded.port END,
                    paired_at = excluded.paired_at",
                params![
                    peer.spki_hex,
                    peer.device_id,
                    peer.name,
                    peer.host,
                    peer.port,
                    crate::store::now_iso()
                ],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn remember_peer_addr(&self, spki: &str, host: &str, port: u16) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE lan_peer SET host = ?1, port = ?2 WHERE spki = ?3",
                params![host, port, spki],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn forget_peer(&self, spki: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM lan_peer WHERE spki = ?1", params![spki])
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn lan_library(&self) -> Result<Library, String> {
        let records = load_records(&self.conn)?;
        let mut stamped = Vec::new();
        for record in records {
            let mut fields = self.field_stamps_public(&record.id)?;
            if fields.is_empty() {
                let stamp = FieldStamp {
                    updated_at: record.updated_at.clone(),
                    device_id: record.device_id.clone(),
                    revision: record.revision,
                };
                for key in field_values(&record.body).keys() {
                    fields.insert(key.clone(), stamp.clone());
                }
            }
            stamped.push(Stamped { record, fields });
        }
        Ok(Library {
            records: stamped,
            bases: load_bases(&self.conn)?,
        })
    }

    fn field_stamps_public(&self, id: &str) -> Result<BTreeMap<String, FieldStamp>, String> {
        self.field_stamps(id)
    }

    pub fn lan_apply(
        &self,
        commit: &LanCommit,
        blobs: &[(String, Vec<u8>)],
        include_location: bool,
    ) -> Result<(), String> {
        for (hash, bytes) in blobs {
            let written = self.put_blob(bytes)?;
            if written != *hash {
                return Err("blob".into());
            }
        }
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|err| err.to_string())?;
        for write in &commit.writes {
            let local: Option<String> = tx
                .query_row(
                    "SELECT body FROM records WHERE id = ?1",
                    params![write.record.id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|err| err.to_string())?;
            let local_body = local.and_then(|text| serde_json::from_str::<Value>(&text).ok());
            let mut record = write.record.clone();
            record.body = merge_private(
                &record.kind,
                local_body.as_ref(),
                &record.body,
                include_location,
            );
            bundle::upsert_record(&tx, &record)?;
            tx.execute(
                "DELETE FROM field_rev WHERE record_id = ?1",
                params![record.id],
            )
            .map_err(|err| err.to_string())?;
            for (field, stamp) in &write.fields {
                tx.execute(
                    "INSERT INTO field_rev (record_id, field, updated_at, device_id, revision)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        record.id,
                        field,
                        stamp.updated_at,
                        stamp.device_id,
                        stamp.revision
                    ],
                )
                .map_err(|err| err.to_string())?;
            }
        }
        for (id, base) in &commit.bases {
            let mut stored = base.clone();
            if stored.fields.is_empty() {
                if let Some(write) = commit.writes.iter().find(|row| row.record.id == *id) {
                    stored.fields = write.fields.clone();
                }
            }
            bundle::write_base(&tx, id, &stored)?;
        }
        tx.commit().map_err(|err| err.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holdover_core::{MergeStatus, SyncBase};
    use serde_json::json;

    #[test]
    fn two_sqlite_libraries_merge_independent_fields_over_tls() {
        let dir = std::env::temp_dir().join(format!("holdover-lan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let desk = Store::open(&dir.join("desk.db")).unwrap();
        let phone = Store::open(&dir.join("phone.db")).unwrap();
        let body = json!({
            "label": "Alpha",
            "sight_height": "38 mm",
            "twist": "1:8",
            "serial": "SECRET-DESK",
        });
        let saved = desk.save_rifle(&body).unwrap();
        let id = saved["id"].as_str().unwrap().to_string();
        copy_record(&desk, &phone, &id);
        anchor(&desk, &id);
        anchor(&phone, &id);
        desk.save_rifle(&json!({
            "id": id,
            "label": "Desk name",
            "sight_height": "38 mm",
            "twist": "1:8",
            "notes": "old",
            "serial": "SECRET-DESK",
        }))
        .unwrap();
        phone
            .save_rifle(&json!({
                "id": id,
                "label": "Alpha",
                "sight_height": "40 mm",
                "twist": "1:8",
                "serial": "SECRET-PHONE",
            }))
            .unwrap();
        // The copy shared the first stamp. Each later save moves only its own field.
        let desk_key = desk.lan_identity().unwrap();
        let phone_key = phone.lan_identity().unwrap();
        let desk_store = Arc::new(Mutex::new(desk));
        let phone_store = Arc::new(Mutex::new(phone));
        let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
        let port = listener.port;
        let desk_thread = desk_store.clone();
        let server = thread::spawn(move || {
            let mut listener = listener;
            let mut catalog = ArcCatalog::new(desk_thread).unwrap();
            listener
                .serve_one(&mut catalog, Duration::from_secs(5))
                .map_err(|err| err.code)
        });
        let addr = format!("127.0.0.1:{port}").parse().unwrap();
        let mut catalog = ArcCatalog::new(phone_store.clone()).unwrap();
        let session = open_sync(
            &mut catalog,
            &phone_key,
            addr,
            desk_key.spki,
            false,
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(session
            .preview
            .rows
            .iter()
            .any(|row| row.id == id && row.status == MergeStatus::Updated));
        assert!(session
            .preview
            .rows
            .iter()
            .all(|row| row.status != MergeStatus::Conflict));
        session.finish(&mut catalog, &BTreeMap::new()).unwrap();
        server.join().unwrap().unwrap();
        let desk = desk_store.lock().unwrap();
        let phone = phone_store.lock().unwrap();
        let desk_body = body_of(&desk, &id);
        let phone_body = body_of(&phone, &id);
        assert_eq!(desk_body["label"], "Desk name");
        assert_eq!(phone_body["label"], "Desk name");
        assert_eq!(
            desk_body["sight_height"]["si"],
            phone_body["sight_height"]["si"]
        );
        assert_eq!(desk_body["serial"], "SECRET-DESK");
        assert_eq!(phone_body["serial"], "SECRET-PHONE");
        let desk_id = desk.lan_identity().unwrap();
        let phone_id = phone.lan_identity().unwrap();
        drop(desk);
        drop(phone);
        let mut again = ArcCatalog::new(phone_store.clone()).unwrap();
        let listener = SyncListener::bind(desk_id.clone(), vec![phone_id.spki]).unwrap();
        let port = listener.port;
        let desk_thread = desk_store.clone();
        let server = thread::spawn(move || {
            let mut listener = listener;
            let mut catalog = ArcCatalog::new(desk_thread).unwrap();
            listener
                .serve_one(&mut catalog, Duration::from_secs(5))
                .map_err(|err| err.code)
        });
        let addr = format!("127.0.0.1:{port}").parse().unwrap();
        let session = open_sync(
            &mut again,
            &phone_id,
            addr,
            desk_id.spki,
            false,
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(session
            .preview
            .rows
            .iter()
            .all(|row| row.status == MergeStatus::Unchanged));
        session.finish(&mut again, &BTreeMap::new()).unwrap();
        server.join().unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn anchor(store: &Store, id: &str) {
        let library = store.lan_library().unwrap();
        let row = library
            .records
            .iter()
            .find(|row| row.record.id == id)
            .unwrap();
        let mut base = SyncBase::from_record(&row.record);
        base.fields = row.fields.clone();
        let mut fields = serde_json::Map::new();
        for (key, stamp) in &base.fields {
            fields.insert(
                key.clone(),
                json!({
                    "updated_at": stamp.updated_at,
                    "device_id": stamp.device_id,
                    "revision": stamp.revision,
                }),
            );
        }
        let suppress = json!({"devices": [], "seen": [], "fields": fields}).to_string();
        store
            .conn
            .execute(
                "INSERT INTO sync_base (id, device_id, revision, updated_at, deleted, suppress)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5)",
                params![id, base.device_id, base.revision, base.updated_at, suppress],
            )
            .unwrap();
    }

    fn copy_record(from: &Store, to: &Store, id: &str) {
        let (kind, created, updated, revision, device, body): (String, String, String, i64, String, String) = from
            .conn
            .query_row(
                "SELECT kind, created_at, updated_at, revision, device_id, body FROM records WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .unwrap();
        to.conn
            .execute(
                "INSERT INTO records (id, kind, created_at, updated_at, revision, deleted_at, device_id, body)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)",
                params![id, kind, created, updated, revision, device, body],
            )
            .unwrap();
        let stamps = from.field_stamps(id).unwrap();
        for (field, stamp) in stamps {
            to.conn
                .execute(
                    "INSERT INTO field_rev (record_id, field, updated_at, device_id, revision) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, field, stamp.updated_at, stamp.device_id, stamp.revision],
                )
                .unwrap();
        }
    }

    fn body_of(store: &Store, id: &str) -> Value {
        let text: String = store
            .conn
            .query_row(
                "SELECT body FROM records WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        serde_json::from_str(&text).unwrap()
    }
}
