//! Desktop and phone sync over Tailscale. The listener stays on port 43720
//! while the app is open and accepts only 100.64.0.0/10 from a paired key.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use holdover_core::{field_values, merge_private, FieldStamp, LanChoice, LanCommit, Stamped};
#[cfg(test)]
use holdover_lan::open_sync;
use holdover_lan::{
    decode_qr_image, host_kind, is_tailscale, join_pair_with, new_identity, offer_qr,
    open_sync_many, parse_offer, preview_value, sort_hosts, tailscale_hosts, unix_now, Catalog,
    ClientSync, Identity, Library, PairWait, Peer, SyncListener, DEFAULT_PORT, PREVIEW_WAIT,
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
    worker: Option<thread::JoinHandle<()>>,
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
    detail: String,
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
            detail: String::new(),
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
            worker: None,
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
            "detail": view.detail,
            "preview": view.preview,
            "found": view.found,
            "name": store.lan_name()?,
            "peers": peers,
            "tailscale": tailscale_hosts(),
            "listen_port": DEFAULT_PORT,
            "listening": view.mode == "listen" && view.port == DEFAULT_PORT,
        }))
    }

    fn remember_port(&self, store: &Store, port: u16) {
        if port != 0 {
            let _ = store.set_meta("lan_port", &port.to_string());
        }
    }

    fn shutdown_worker(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // The accept loop notices the flag within a second and drops the
        // socket. Joining here is what frees 43720 for the next bind.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        crate::phone::mdns_stop();
    }

    pub fn stop(&mut self) {
        self.shutdown_worker();
        self.session.take();
        if let Ok(mut view) = self.shared.lock() {
            *view = LanView::default();
        }
    }

    /// Keep a sync listener on 43720 unless a pairing code is on screen.
    pub fn ensure_listener(&mut self, store: Arc<Mutex<Store>>) -> Result<(), String> {
        let mode = self
            .shared
            .lock()
            .map(|view| view.mode.clone())
            .unwrap_or_else(|_| "idle".into());
        if mode == "pair" || mode == "joining" {
            return Ok(());
        }
        if self.worker_alive() {
            return Ok(());
        }
        match self.listen(store) {
            Ok(()) => Ok(()),
            Err(err) if err == "port-busy" => Ok(()),
            Err(err) => Err(err),
        }
    }

    pub fn cancel(&mut self, store: Arc<Mutex<Store>>) -> Result<(), String> {
        self.session.take();
        let mode = self
            .shared
            .lock()
            .map(|view| view.mode.clone())
            .unwrap_or_else(|_| "idle".into());
        if mode == "pair" || mode == "joining" {
            self.shutdown_worker();
            if let Ok(mut view) = self.shared.lock() {
                view.mode = "idle".into();
                view.offer_svg.clear();
                view.offer_url.clear();
                view.pending_name.clear();
                view.pending_fp.clear();
                view.code.clear();
                view.detail.clear();
            }
        } else if mode == "preview" {
            if let Ok(mut view) = self.shared.lock() {
                view.preview = None;
                view.mode = "idle".into();
            }
        }
        self.ensure_listener(store)
    }

    fn worker_alive(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    pub fn set_name(&mut self, store: &Store, name: &str) -> Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.chars().count() > 40 {
            return Err("lan-name".into());
        }
        store.set_meta("lan_name", trimmed)
    }

    pub fn pair_start(&mut self, store: Arc<Mutex<Store>>) -> Result<(), String> {
        let hosts = tailscale_hosts();
        if hosts.is_empty() {
            self.fail_visible("no-tailscale", "no tailscale");
            lan_log_detail("pair-start", "no-tailscale", "host=none port=43720");
            return Err("no-tailscale".into());
        }
        self.stop();
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = stop.clone();
        let identity = {
            let guard = store
                .lock()
                .map_err(|_| "the library is busy".to_string())?;
            guard.lan_identity()?
        };
        let (mut wait, offer) =
            PairWait::start_on(identity.clone(), hosts.clone(), unix_now(), DEFAULT_PORT)
                .map_err(|err| err.code)?;
        if offer.port != DEFAULT_PORT {
            drop(wait);
            self.fail_visible("port-busy", "43720");
            lan_log_detail("pair-start", "port-busy", "port=43720");
            return Err("port-busy".into());
        }
        wait.accept_tailscale_only();
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
            view.detail.clear();
        }
        if let Ok(guard) = store.lock() {
            self.remember_port(&guard, offer.port);
        }
        lan_log(
            "pair-start",
            "ok",
            hosts.first().map(String::as_str).unwrap_or(""),
            offer.port,
        );
        let shared = self.shared.clone();
        let decision = self.decision.clone();
        *decision
            .lock()
            .map_err(|_| "the sync state is busy".to_string())? = None;
        self.worker = Some(thread::spawn(move || {
            pair_loop(store, wait, shared, decision, stop, identity)
        }));
        Ok(())
    }

    pub fn pair_join(&mut self, store: Arc<Mutex<Store>>, code: &str) -> Result<(), String> {
        let mut offer = match parse_offer(code.trim()) {
            Ok(offer) => offer,
            Err(err) => {
                self.note(&err.code, "");
                return Err(err.code);
            }
        };
        if unix_now() >= offer.expires {
            self.note("pair-expired", "");
            return Err("pair-expired".into());
        }
        offer.port = DEFAULT_PORT;
        offer.hosts.retain(|host| is_tailscale(host));
        if offer.hosts.is_empty() {
            self.fail_visible("need-tailscale", "no tailscale");
            lan_log_detail("pair-join", "need-tailscale", "host=none port=43720");
            return Err("need-tailscale".into());
        }
        self.stop();
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = stop.clone();
        let identity = {
            let guard = store
                .lock()
                .map_err(|_| "the library is busy".to_string())?;
            guard.lan_identity()?
        };
        self.note("", "reaching");
        if let Ok(mut view) = self.shared.lock() {
            view.mode = "joining".into();
        }
        let shared = self.shared.clone();
        let stop_flag = stop.clone();
        thread::spawn(move || {
            let result = join_pair_with(
                &identity,
                &offer,
                unix_now(),
                Duration::from_secs(8),
                |phase| {
                    if stop_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    if let Ok(mut view) = shared.lock() {
                        if view.mode == "joining" {
                            view.message = phase.to_string();
                            view.code.clear();
                        }
                    }
                },
            );
            if stop_flag.load(Ordering::SeqCst) {
                return;
            }
            let mut view = match shared.lock() {
                Ok(view) => view,
                Err(_) => return,
            };
            match result {
                Ok(mut peer) => {
                    let mut hosts = offer.hosts.clone();
                    if is_tailscale(&peer.host) && !hosts.iter().any(|host| host == &peer.host) {
                        hosts.push(peer.host.clone());
                    }
                    hosts.retain(|host| is_tailscale(host));
                    sort_hosts(&mut hosts);
                    peer.host = hosts.join(",");
                    peer.port = DEFAULT_PORT;
                    if let Ok(guard) = store.lock() {
                        let _ = guard.save_peer(&peer);
                    }
                    view.mode = "paired".into();
                    view.message = peer.name;
                    view.code.clear();
                    view.detail.clear();
                    lan_log("pair-join", "ok", &peer.host, DEFAULT_PORT);
                }
                Err(err) => {
                    view.mode = "idle".into();
                    view.message.clear();
                    view.code = err.code.clone();
                    view.detail = pair_detail(&offer.hosts, &err.code);
                    lan_log_detail("pair-join", &err.code, &view.detail);
                }
            }
        });
        Ok(())
    }

    fn note(&self, code: &str, message: &str) {
        if let Ok(mut view) = self.shared.lock() {
            view.mode = if message == "reaching" {
                "joining".into()
            } else {
                "idle".into()
            };
            view.code = code.to_string();
            view.message = message.to_string();
        }
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
        if self.listener_up() {
            return Ok(());
        }
        self.shutdown_worker();
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = stop.clone();
        let (identity, pins) = {
            let guard = store
                .lock()
                .map_err(|_| "the library is busy".to_string())?;
            (guard.lan_identity()?, guard.lan_pins()?)
        };
        let mut listener =
            SyncListener::bind_on(identity, pins, DEFAULT_PORT).map_err(|err| err.code)?;
        if listener.port != DEFAULT_PORT {
            drop(listener);
            self.fail_visible("port-busy", "43720");
            lan_log_detail("listen", "port-busy", "port=43720");
            return Err("port-busy".into());
        }
        listener.accept_tailscale_only();
        let tails = tailscale_hosts();
        {
            let mut view = self
                .shared
                .lock()
                .map_err(|_| "the sync state is busy".to_string())?;
            view.mode = "listen".into();
            view.port = DEFAULT_PORT;
            if view.code == "port-busy" {
                view.code.clear();
                view.detail.clear();
            }
        }
        if let Ok(guard) = store.lock() {
            self.remember_port(&guard, DEFAULT_PORT);
        }
        if let Some(host) = tails.first() {
            lan_log("listen", "ok", host, DEFAULT_PORT);
        } else {
            lan_log_detail("listen", "no-tailscale", "host=none port=43720");
        }
        let shared = self.shared.clone();
        self.worker = Some(thread::spawn(move || {
            listen_loop(store, listener, shared, stop)
        }));
        Ok(())
    }

    fn listener_up(&self) -> bool {
        if !self.worker_alive() {
            return false;
        }
        self.shared
            .lock()
            .map(|view| view.mode == "listen" && view.port == DEFAULT_PORT)
            .unwrap_or(false)
    }

    pub fn sync_begin(
        &mut self,
        store: Arc<Mutex<Store>>,
        host: &str,
        _port: u16,
        spki: &str,
        include_location: bool,
    ) -> Result<(), String> {
        self.session.take();
        let (identity, peers) = {
            let guard = store
                .lock()
                .map_err(|_| "the library is busy".to_string())?;
            (guard.lan_identity()?, guard.lan_peers()?)
        };
        let saved = peers
            .as_array()
            .and_then(|items| items.iter().find(|peer| peer["spki"] == spki));
        let addresses = sync_addresses(host, saved);
        if addresses.is_empty() || spki_pin(spki).is_err() {
            let typed = host.trim();
            let (code, detail) = if typed.is_empty() {
                ("need-tailscale", "no tailscale".to_string())
            } else {
                ("no-tailscale", format!("tried {typed}:43720 no tailscale"))
            };
            self.fail_visible(code, &detail);
            lan_log_detail("sync-begin", code, &detail);
            return Err(code.into());
        }
        let pin = spki_pin(spki)?;
        if let Ok(mut view) = self.shared.lock() {
            view.message = "sync-reaching".into();
            view.code.clear();
        }
        let mut catalog = ArcCatalog::new(store.clone())?;
        let session = match open_sync_many(
            &mut catalog,
            &identity,
            &addresses,
            pin,
            include_location,
            Duration::from_secs(4),
        ) {
            Ok(session) => session,
            Err(err) => {
                let code = if matches!(
                    err.code.as_str(),
                    "sync-unreachable" | "disconnected" | "tls"
                ) {
                    "sync-unreachable"
                } else {
                    err.code.as_str()
                };
                let detail = err.detail.replace("unauthorized", "auth failed");
                self.fail_visible(code, &detail);
                lan_log_detail("sync-begin", code, &detail);
                return Err(code.to_string());
            }
        };
        let used_host = session.peer().host.clone();
        let used_port = session.peer().port;
        let preview = preview_value(&session.preview);
        self.session = Some(session);
        let mut view = self
            .shared
            .lock()
            .map_err(|_| "the sync state is busy".to_string())?;
        view.mode = "preview".into();
        view.preview = Some(preview);
        view.message.clear();
        view.code.clear();
        view.detail.clear();
        if let Ok(guard) = store.lock() {
            let _ = guard.remember_peer_addr(spki, &used_host, used_port);
        }
        lan_log("sync-begin", "ok", &used_host, used_port);
        Ok(())
    }

    fn fail_visible(&self, code: &str, detail: &str) {
        if let Ok(mut view) = self.shared.lock() {
            if view.mode != "listen" && view.mode != "pair" {
                view.mode = "idle".into();
            }
            view.message.clear();
            view.preview = None;
            view.code = code.to_string();
            view.detail = detail.to_string();
        }
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
                    Some(true) => {
                        let mut hosts = pending.client_hosts().to_vec();
                        match pending
                            .accept_with(identity.name.as_str(), identity.device_id.as_str())
                        {
                            Ok(mut peer) => {
                                if is_tailscale(&peer.host)
                                    && !hosts.iter().any(|item| item == &peer.host)
                                {
                                    hosts.push(peer.host.clone());
                                }
                                hosts.retain(|item| is_tailscale(item));
                                sort_hosts(&mut hosts);
                                peer.host = hosts.join(",");
                                peer.port = DEFAULT_PORT;
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
                        }
                    }
                    Some(false) => {
                        let _ = pending.reject();
                    }
                    None => {
                        let _ = pending.reject();
                        return;
                    }
                }
            }
            Err(err) if matches!(err.code.as_str(), "disconnected" | "tls" | "not-tailscale") => {}
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
                view.offer_svg.clear();
                view.offer_url.clear();
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
            Err(err) if err.code == "disconnected" || err.code == "not-tailscale" => {
                if err.code == "not-tailscale" {
                    lan_log_detail("listen", "not-tailscale", "host=other port=43720");
                }
            }
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

fn peer_hosts(value: &str) -> Vec<String> {
    let mut hosts: Vec<String> = value
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect();
    hosts.dedup();
    hosts
}

fn pair_detail(hosts: &[String], code: &str) -> String {
    let why = if code == "pair-pin" || code == "unauthorized" {
        "auth failed"
    } else if code == "pair-unreachable" {
        "timeout"
    } else {
        code
    };
    hosts
        .iter()
        .map(|host| format!("tried {host}:{DEFAULT_PORT} {why}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// A typed address wins. Otherwise the saved Tailscale addresses are used.
/// Wi-Fi addresses and port 0 are never dialed. A MagicDNS name is kept only
/// when it resolves inside 100.64.0.0/10.
fn sync_addresses(explicit_host: &str, saved: Option<&Value>) -> Vec<SocketAddr> {
    let typed = explicit_host.trim();
    let mut hosts = Vec::new();
    if !typed.is_empty() {
        hosts.extend(tailscale_targets(typed));
    } else if let Some(peer) = saved {
        for host in peer_hosts(peer["host"].as_str().unwrap_or("")) {
            hosts.extend(tailscale_targets(&host));
        }
    }
    let mut out = Vec::new();
    for ip in hosts {
        let addr = SocketAddr::new(ip, DEFAULT_PORT);
        if addr.port() != 0 && !out.contains(&addr) {
            out.push(addr);
        }
    }
    out
}

fn tailscale_targets(host: &str) -> Vec<IpAddr> {
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return if is_tailscale(host) {
            vec![IpAddr::V4(ip)]
        } else {
            Vec::new()
        };
    }
    let Ok(found) = (host, DEFAULT_PORT).to_socket_addrs() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for addr in found {
        if let IpAddr::V4(ip) = addr.ip() {
            if is_tailscale(&ip.to_string()) && !out.contains(&IpAddr::V4(ip)) {
                out.push(IpAddr::V4(ip));
            }
        }
    }
    out
}

fn lan_log(action: &str, result: &str, host: &str, port: u16) {
    let kind = if host.is_empty() {
        "none"
    } else {
        host_kind(host)
    };
    lan_log_detail(action, result, &format!("host={kind} port={port}"));
}

fn lan_log_detail(action: &str, result: &str, detail: &str) {
    let detail = detail.replace(['\n', '\r'], " ");
    let line = format!("lan action={action} result={result} {detail}");
    eprintln!("holdover: {line}");
    crate::phone::log_line(&line);
    #[cfg(not(target_os = "android"))]
    {
        let message = std::ffi::CString::new(line)
            .unwrap_or_else(|_| std::ffi::CString::new("holdover lan").unwrap());
        unsafe {
            libc::openlog(
                c"holdover".as_ptr(),
                libc::LOG_PID | libc::LOG_NDELAY,
                libc::LOG_USER,
            );
            libc::syslog(libc::LOG_INFO, c"%s".as_ptr(), message.as_ptr());
        }
    }
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
        let current = self.lan_peers()?;
        let existing = current
            .as_array()
            .and_then(|items| {
                items
                    .iter()
                    .find(|peer| peer["spki"] == spki)
                    .and_then(|peer| peer["host"].as_str())
                    .map(|text| text.to_string())
            })
            .unwrap_or_default();
        let mut hosts = peer_hosts(&existing);
        hosts.retain(|item| is_tailscale(item));
        if is_tailscale(host) && !hosts.iter().any(|item| item == host) {
            hosts.push(host.to_string());
        }
        sort_hosts(&mut hosts);
        let joined = if hosts.is_empty() {
            existing
        } else {
            hosts.join(",")
        };
        let stored_port = if port == 0 { DEFAULT_PORT } else { port };
        self.conn
            .execute(
                "UPDATE lan_peer SET host = ?1, port = ?2 WHERE spki = ?3",
                params![joined, stored_port, spki],
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
    fn sync_dials_only_the_tailscale_address_on_the_fixed_port() {
        let saved = json!({
            "host": "192.168.1.9,100.64.0.2",
            "port": 0,
            "spki": "abc",
        });
        let addrs = sync_addresses("", Some(&saved));
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0].to_string(), "100.64.0.2:43720");
        assert!(sync_addresses("192.168.1.9", Some(&saved)).is_empty());
        assert_eq!(
            sync_addresses("100.127.1.4", Some(&saved))[0].to_string(),
            "100.127.1.4:43720"
        );
        assert!(sync_addresses("localhost", None).is_empty());
        assert!(sync_addresses("", None).is_empty());
    }

    #[test]
    fn ready_reuses_the_listener_and_a_new_pair_uses_that_port() {
        let dir = std::env::temp_dir().join(format!(
            "holdover-lan-port-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(&dir.join("desk.db")).unwrap();
        let other = holdover_lan::new_identity("phone", "Pocket").unwrap();
        store
            .save_peer(&holdover_lan::Peer {
                device_id: other.device_id.clone(),
                name: other.name.clone(),
                spki_hex: other.spki_hex.clone(),
                spki: other.spki,
                host: "192.168.1.9,100.64.0.2".into(),
                port: 0,
            })
            .unwrap();
        store.set_meta("lan_port", "41189").unwrap();
        let stable_free = std::net::TcpListener::bind("0.0.0.0:43720").is_ok();
        let store = Arc::new(Mutex::new(store));
        let mut hub = LanHub::default();
        hub.listen(store.clone()).unwrap();
        let first = hub.status(&store.lock().unwrap()).unwrap()["port"]
            .as_u64()
            .unwrap();
        if stable_free {
            assert_eq!(
                first, 43720,
                "a remembered random port must not win over 43720"
            );
        }
        assert_ne!(first, 0);
        let still_held = std::net::TcpListener::bind(("0.0.0.0", first as u16)).is_err();
        assert!(still_held, "the first Ready is not accepting");
        hub.listen(store.clone()).unwrap();
        let second = hub.status(&store.lock().unwrap()).unwrap()["port"]
            .as_u64()
            .unwrap();
        assert_eq!(second, first, "the second listen opened a new port");
        if tailscale_hosts().is_empty() {
            let err = hub.pair_start(store.clone()).unwrap_err();
            assert_eq!(err, "no-tailscale");
            assert!(hub.status(&store.lock().unwrap()).unwrap()["listening"]
                .as_bool()
                .unwrap());
        } else {
            hub.pair_start(store.clone()).unwrap();
            let paired = hub.status(&store.lock().unwrap()).unwrap()["port"]
                .as_u64()
                .unwrap();
            assert_eq!(paired, 43720);
        }
        hub.stop();
        if stable_free {
            let closed = std::net::TcpListener::bind("0.0.0.0:43720");
            assert!(closed.is_ok(), "Cancel left 43720 open");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

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
