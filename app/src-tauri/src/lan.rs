//! Desktop and phone sync over Tailscale. The listener stays on port 43720
//! while the app is open and accepts only 100.64.0.0/10 from a paired key.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use holdover_core::{field_values, merge_private, FieldStamp, LanChoice, LanCommit, Stamped};
#[cfg(test)]
use holdover_lan::open_sync;
use holdover_lan::{
    decode_qr_image, host_kind, is_tailscale, is_tailscale_ipv6, join_pair_with, new_identity,
    offer_qr, open_sync_many, parse_offer, pin_from_hex, preview_value, sort_hosts,
    tailscale_hosts, tailscale_ipv6_only, unix_now, Catalog, ClientSync, Identity, LanError,
    Library, PairWait, Peer, SyncListener, DEFAULT_PORT, PREVIEW_WAIT, SYNC_HANDSHAKE,
};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

use crate::bundle::{self, load_bases, load_records};
use crate::store::Store;

pub struct LanHub {
    stop: Arc<AtomicBool>,
    shared: Arc<Mutex<LanView>>,
    session: Arc<Mutex<Option<ClientSync>>>,
    sync_gen: Arc<AtomicU64>,
    dialing: Arc<AtomicU64>,
    serving: Arc<AtomicUsize>,
    decision: Arc<Mutex<Option<bool>>>,
    worker: Option<thread::JoinHandle<()>>,
    /// Accept-thread binds. A second listen while one is alive does not increment this.
    binds: Arc<AtomicU64>,
    keeper_stop: Arc<AtomicBool>,
    keeper: Option<thread::JoinHandle<()>>,
}

#[derive(Clone)]
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
        }
    }
}

impl Default for LanHub {
    fn default() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            shared: Arc::new(Mutex::new(LanView::default())),
            session: Arc::new(Mutex::new(None)),
            sync_gen: Arc::new(AtomicU64::new(0)),
            dialing: Arc::new(AtomicU64::new(0)),
            serving: Arc::new(AtomicUsize::new(0)),
            decision: Arc::new(Mutex::new(None)),
            worker: None,
            binds: Arc::new(AtomicU64::new(0)),
            keeper_stop: Arc::new(AtomicBool::new(false)),
            keeper: None,
        }
    }
}

/// How often startup retries the listener while Tailscale has no 100.x address.
const LISTEN_RETRY: Duration = Duration::from_secs(2);

/// Listen on 43720 for the life of the process. The Phone tab is not involved.
/// If Tailscale is not up yet, the same thread keeps trying until a 100.x
/// address appears. A later `ensure_listener` does not open a second socket.
pub fn arm_listener(hub: Arc<Mutex<LanHub>>, store: Arc<Mutex<Store>>) {
    let mut guard = hub.lock().unwrap_or_else(|err| err.into_inner());
    if guard
        .keeper
        .as_ref()
        .is_some_and(|handle| !handle.is_finished())
    {
        return;
    }
    guard.keeper_stop.store(false, Ordering::SeqCst);
    let thread_hub = hub.clone();
    let thread_store = store.clone();
    guard.keeper = Some(thread::spawn(move || {
        keep_listener(thread_hub, thread_store)
    }));
}

fn keep_listener(hub: Arc<Mutex<LanHub>>, store: Arc<Mutex<Store>>) {
    let mut saw_tailscale = !tailscale_hosts().is_empty();
    loop {
        if keeper_stopped(&hub) {
            break;
        }
        let tails = tailscale_hosts();
        let up = {
            let mut guard = hub.lock().unwrap_or_else(|err| err.into_inner());
            if guard.keeper_stop.load(Ordering::SeqCst) {
                break;
            }
            if !guard.worker_alive() {
                let _ = guard.ensure_listener(store.clone());
            }
            guard.listener_up()
        };
        if !saw_tailscale && up {
            if let Some(host) = tails.first() {
                lan_log("listen", "ok", host, DEFAULT_PORT);
                saw_tailscale = true;
            }
        }
        if !sleep_or_stop(&hub, LISTEN_RETRY) {
            break;
        }
    }
}

fn keeper_stopped(hub: &Mutex<LanHub>) -> bool {
    hub.lock()
        .unwrap_or_else(|err| err.into_inner())
        .keeper_stop
        .load(Ordering::SeqCst)
}

/// Sleep in short slices so a test can stop the retry without waiting the
/// whole period. Returns false when the keeper was asked to stop.
fn sleep_or_stop(hub: &Mutex<LanHub>, total: Duration) -> bool {
    let slice = total / 20;
    for _ in 0..20 {
        if keeper_stopped(hub) {
            return false;
        }
        thread::sleep(slice);
    }
    true
}

struct SyncStart {
    store: Arc<Mutex<Store>>,
    addresses: Vec<SocketAddr>,
    identity: Identity,
    pin: [u8; 32],
    spki: String,
    include_location: bool,
    bound: Duration,
}

impl LanHub {
    pub fn status(&self, store: &Store) -> Result<Value, String> {
        let view = self
            .shared
            .lock()
            .map_err(|_| "the sync state is busy".to_string())?
            .clone();
        let peers = store.lan_peers()?;
        let name = store.lan_name()?;
        // Interface enumeration stays outside the sync lock. A phone sync
        // publishes its preview on another thread, and this call must not
        // hold that lock across a scan.
        let tails = tailscale_hosts();
        let ipv6 = tails.is_empty() && tailscale_ipv6_only();
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
            "name": name,
            "peers": peers,
            "tailscale": tails,
            "tailscale_ipv6": ipv6,
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

    fn take_session(&self) -> Option<ClientSync> {
        self.session.lock().ok().and_then(|mut slot| slot.take())
    }

    fn abandon_sync(&self) {
        self.sync_gen.fetch_add(1, Ordering::SeqCst);
        self.dialing.store(0, Ordering::SeqCst);
        let _ = self.take_session();
    }

    pub fn stop(&mut self) {
        self.shutdown_worker();
        self.abandon_sync();
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

    fn sync_in_progress(&self) -> bool {
        if self.dialing.load(Ordering::SeqCst) != 0 || self.serving.load(Ordering::SeqCst) != 0 {
            return true;
        }
        if self
            .session
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false)
        {
            return true;
        }
        self.shared
            .lock()
            .map(|view| {
                view.mode == "preview"
                    || matches!(
                        view.message.as_str(),
                        "sync-reaching" | "sync-applying" | "sync-serving"
                    )
            })
            .unwrap_or(false)
    }

    fn note_busy(&self) {
        if let Ok(mut view) = self.shared.lock() {
            view.code = "sync-busy".into();
            view.detail = "already in progress".into();
        }
        lan_log_detail("sync-begin", "sync-busy", "already in progress");
    }

    pub fn cancel(&mut self, store: Arc<Mutex<Store>>) -> Result<(), String> {
        self.abandon_sync();
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
                view.message.clear();
            }
        } else if let Ok(mut view) = self.shared.lock() {
            if view.mode == "preview" {
                view.mode = "idle".into();
            }
            view.preview = None;
            if matches!(
                view.message.as_str(),
                "sync-reaching" | "sync-serving" | "sync-applying"
            ) {
                view.message.clear();
            }
            if view.code.starts_with("sync-") {
                view.code.clear();
                view.detail.clear();
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
            let code = if tailscale_ipv6_only() {
                "tailscale-ipv6"
            } else {
                "no-tailscale"
            };
            let detail = if code == "tailscale-ipv6" {
                "ipv6-only tailscale"
            } else {
                "no tailscale"
            };
            self.fail_visible(code, detail);
            lan_log_detail("pair-start", code, "host=none port=43720");
            return Err(code.into());
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
            let already = self
                .shared
                .lock()
                .map(|view| view.code == "port-busy")
                .unwrap_or(false);
            self.fail_visible("port-busy", "43720");
            if !already {
                lan_log_detail("listen", "port-busy", "port=43720");
            }
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
        } else if tailscale_ipv6_only() {
            lan_log_detail("listen", "tailscale-ipv6", "host=none port=43720");
        } else {
            lan_log_detail("listen", "no-tailscale", "host=none port=43720");
        }
        let shared = self.shared.clone();
        let dialing = self.dialing.clone();
        let serving = self.serving.clone();
        self.binds.fetch_add(1, Ordering::SeqCst);
        self.worker = Some(thread::spawn(move || {
            listen_loop(store, listener, shared, stop, dialing, serving)
        }));
        Ok(())
    }

    #[cfg(test)]
    fn disarm_keeper(&mut self) -> Option<thread::JoinHandle<()>> {
        self.keeper_stop.store(true, Ordering::SeqCst);
        self.shutdown_worker();
        self.keeper.take()
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
        if self.sync_in_progress() {
            self.note_busy();
            return Err("sync-busy".into());
        }
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
            let (code, detail) = missing_tailscale(host.trim());
            self.fail_visible(code, &detail);
            lan_log_detail("sync-begin", code, &detail);
            return Err(code.into());
        }
        let pin = spki_pin(spki)?;
        self.begin_sync(SyncStart {
            store,
            addresses,
            identity,
            pin,
            spki: spki.to_string(),
            include_location,
            bound: SYNC_HANDSHAKE,
        });
        Ok(())
    }

    fn begin_sync(&mut self, start: SyncStart) {
        let SyncStart {
            store,
            addresses,
            identity,
            pin,
            spki,
            include_location,
            bound,
        } = start;
        if self.sync_in_progress() {
            self.note_busy();
            return;
        }
        self.abandon_sync();
        let generation = self.sync_gen.fetch_add(1, Ordering::SeqCst) + 1;
        self.dialing.store(generation, Ordering::SeqCst);
        if let Ok(mut view) = self.shared.lock() {
            view.message = "sync-reaching".into();
            view.code.clear();
            view.detail.clear();
            view.preview = None;
        }
        let shared = self.shared.clone();
        let session_slot = self.session.clone();
        let sync_gen = self.sync_gen.clone();
        let dialing = self.dialing.clone();
        thread::spawn(move || {
            let store_for_peer = store.clone();
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
                let mut catalog =
                    ArcCatalog::new(store.clone()).map_err(|_| LanError::new("busy"))?;
                open_sync_many(
                    &mut catalog,
                    &identity,
                    &addresses,
                    pin,
                    include_location,
                    bound,
                )
            }));
            if dialing
                .compare_exchange(generation, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return;
            }
            if sync_gen.load(Ordering::SeqCst) != generation {
                return;
            }
            match outcome {
                Ok(Ok(session)) => {
                    if sync_gen.load(Ordering::SeqCst) != generation {
                        return;
                    }
                    let used_host = session.peer().host.clone();
                    let used_port = session.peer().port;
                    let preview = preview_value(&session.preview);
                    if let Ok(mut slot) = session_slot.lock() {
                        *slot = Some(session);
                    }
                    if let Ok(mut view) = shared.lock() {
                        view.mode = "preview".into();
                        view.preview = Some(preview);
                        view.message.clear();
                        view.code.clear();
                        view.detail.clear();
                    }
                    if let Ok(guard) = store_for_peer.lock() {
                        let _ = guard.remember_peer_addr(&spki, &used_host, used_port);
                    }
                    lan_log("sync-begin", "ok", &used_host, used_port);
                }
                Ok(Err(err)) => {
                    let code = if matches!(
                        err.code.as_str(),
                        "sync-unreachable" | "disconnected" | "tls"
                    ) {
                        "sync-unreachable"
                    } else {
                        err.code.as_str()
                    };
                    let detail = err.detail.replace("unauthorized", "auth failed");
                    show_sync_error(&shared, code, &detail);
                    lan_log_detail("sync-begin", code, &detail);
                }
                Err(payload) => {
                    let text = panic_text(&payload);
                    report_panic_text(&text);
                    show_sync_error(&shared, "sync-failed", &text);
                    lan_log_detail("sync-begin", "sync-failed", &text);
                }
            }
        });
    }

    #[cfg(test)]
    fn begin_sync_for_test(
        &mut self,
        store: Arc<Mutex<Store>>,
        address: SocketAddr,
        pin: [u8; 32],
        spki: &str,
        bound: Duration,
    ) {
        let identity = store
            .lock()
            .expect("library")
            .lan_identity()
            .expect("identity");
        self.begin_sync(SyncStart {
            store,
            addresses: vec![address],
            identity,
            pin,
            spki: spki.to_string(),
            include_location: false,
            bound,
        });
    }

    /// Listen on an ephemeral port without the Tailscale filter, for tests.
    #[cfg(test)]
    fn listen_for_test(&mut self, store: Arc<Mutex<Store>>) -> Result<u16, String> {
        self.shutdown_worker();
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = stop.clone();
        let (identity, pins) = {
            let guard = store
                .lock()
                .map_err(|_| "the library is busy".to_string())?;
            (guard.lan_identity()?, guard.lan_pins()?)
        };
        let listener = SyncListener::bind(identity, pins).map_err(|err| err.code)?;
        let port = listener.port;
        {
            let mut view = self
                .shared
                .lock()
                .map_err(|_| "the sync state is busy".to_string())?;
            view.mode = "listen".into();
            view.port = port;
            view.message.clear();
            view.code.clear();
        }
        let shared = self.shared.clone();
        let dialing = self.dialing.clone();
        let serving = self.serving.clone();
        self.worker = Some(thread::spawn(move || {
            listen_loop(store, listener, shared, stop, dialing, serving);
        }));
        Ok(port)
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
        let session = self.take_session().ok_or("lan-no-preview")?;
        let parsed = parse_choice_map(choices)?;
        if let Ok(mut view) = self.shared.lock() {
            view.message = "sync-applying".into();
            view.code.clear();
        }
        let shared = self.shared.clone();
        thread::spawn(move || {
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
                let mut catalog = ArcCatalog::new(store)?;
                session
                    .finish(&mut catalog, &parsed)
                    .map_err(|err| err.code)
            }));
            match outcome {
                Ok(Ok(())) => {
                    if let Ok(mut view) = shared.lock() {
                        view.mode = "done".into();
                        view.preview = None;
                        view.code.clear();
                        view.detail.clear();
                        view.message = "applied".into();
                    }
                    lan_log_detail("sync-apply", "ok", "host=none port=0");
                }
                Ok(Err(code)) => {
                    show_sync_error(&shared, &code, "");
                    lan_log_detail("sync-apply", &code, "host=none port=0");
                }
                Err(payload) => {
                    let text = panic_text(&payload);
                    report_panic_text(&text);
                    show_sync_error(&shared, "sync-failed", &text);
                    lan_log_detail("sync-apply", "sync-failed", &text);
                }
            }
        });
        Ok(())
    }

    pub fn session_cancel(&mut self) {
        self.abandon_sync();
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
    pin_from_hex(spki).map_err(|_| "unauthorized".into())
}

fn missing_tailscale(typed: &str) -> (&'static str, String) {
    if is_tailscale_ipv6(typed) {
        (
            "tailscale-ipv6",
            format!("tried {typed}:43720 ipv6-only tailscale"),
        )
    } else if typed.is_empty() {
        ("need-tailscale", "no tailscale".to_string())
    } else {
        ("no-tailscale", format!("tried {typed}:43720 no tailscale"))
    }
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
    dialing: Arc<AtomicU64>,
    serving: Arc<AtomicUsize>,
) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept_sync(Duration::from_secs(1)) {
            Ok(accepted) => {
                let store = store.clone();
                let shared = shared.clone();
                let dialing = dialing.clone();
                let serving = serving.clone();
                thread::spawn(move || serve_inbound(store, shared, dialing, serving, accepted));
            }
            Err(err) if err.code == "disconnected" => {}
            Err(err) => {
                if err.code == "not-tailscale" {
                    lan_log_detail("listen", "not-tailscale", "host=other port=43720");
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

fn serve_inbound(
    store: Arc<Mutex<Store>>,
    shared: Arc<Mutex<LanView>>,
    dialing: Arc<AtomicU64>,
    serving: Arc<AtomicUsize>,
    accepted: holdover_lan::AcceptedSync,
) {
    if dialing.load(Ordering::SeqCst) == 0 {
        if let Ok(mut view) = shared.lock() {
            if view.mode != "preview" && view.message != "sync-reaching" {
                view.message = "sync-serving".into();
                view.code.clear();
            }
        }
    }
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        let mut catalog = ArcCatalog::new(store)?;
        accepted
            .serve(&mut catalog, &dialing, &serving, |stage| {
                lan_log_detail("sync-serve", "ok", &format!("stage={stage}"));
            })
            .map_err(|err| err.code)
    }));
    match outcome {
        Ok(Ok(())) => {
            if let Ok(mut view) = shared.lock() {
                if view.mode != "preview" && view.message != "sync-reaching" {
                    view.message = "applied".into();
                    view.code.clear();
                    view.detail.clear();
                }
            }
        }
        Ok(Err(code))
            if matches!(
                code.as_str(),
                "disconnected" | "not-tailscale" | "sync-yield" | "sync-busy"
            ) =>
        {
            if let Ok(mut view) = shared.lock() {
                if view.message == "sync-serving" && view.mode != "preview" {
                    view.message.clear();
                }
            }
            if code == "not-tailscale" {
                lan_log_detail("listen", "not-tailscale", "host=other port=43720");
            }
        }
        Ok(Err(code)) => {
            if let Ok(mut view) = shared.lock() {
                if view.mode != "preview" && view.message != "sync-reaching" {
                    view.code = code;
                }
            }
        }
        Err(payload) => {
            let text = panic_text(&payload);
            report_panic_text(&text);
            show_sync_error(&shared, "sync-failed", &text);
        }
    }
}

fn show_sync_error(shared: &Mutex<LanView>, code: &str, detail: &str) {
    if let Ok(mut view) = shared.lock() {
        if view.mode == "preview" {
            return;
        }
        if view.mode != "listen" && view.mode != "pair" {
            view.mode = "idle".into();
        }
        view.message.clear();
        view.preview = None;
        view.code = code.to_string();
        view.detail = detail.chars().take(200).collect();
    }
}

pub(crate) fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    let raw = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic");
    let flat = raw.replace(['\n', '\r'], " ");
    let text: String = flat.chars().take(200).collect();
    if text.is_empty() {
        "unknown panic".into()
    } else {
        text
    }
}

pub(crate) fn report_panic_text(text: &str) {
    let flat = text.replace(['\n', '\r'], " ");
    let shown: String = flat.chars().take(200).collect();
    syslog_line(&format!("panic {shown}"));
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
    syslog_line(&format!("lan action={action} result={result} {detail}"));
}

fn syslog_line(line: &str) {
    eprintln!("holdover: {line}");
    crate::phone::log_line(line);
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
        // PDF and photograph bytes land after the commit, by hash, matching a
        // bundle import. A rollback then leaves no new blob file.
        for (hash, bytes) in blobs {
            let written = self.put_blob(bytes)?;
            if written != *hash {
                return Err("blob".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holdover_core::{MergeStatus, SyncBase};
    use serde_json::json;

    /// Tests that bind 43720 take this so they do not steal the port from each other.
    fn listen_port_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|err| err.into_inner())
    }

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
        assert!(sync_addresses("fd7a:115c:a1e0::1", None).is_empty());
        let (code, detail) = missing_tailscale("fd7a:115c:a1e0::1");
        assert_eq!(code, "tailscale-ipv6");
        assert!(detail.contains("ipv6-only tailscale"));
        assert!(!detail.contains("no tailscale"));
        assert_eq!(missing_tailscale("192.168.1.9").0, "no-tailscale");
        assert_eq!(missing_tailscale("").0, "need-tailscale");
        assert!(is_tailscale_ipv6("[fd7a:115c:a1e0:abcd::1]"));
        assert!(!is_tailscale_ipv6("fd7a:115c:a1df::1"));
        assert!(!is_tailscale_ipv6("100.64.0.2"));
    }

    #[test]
    fn a_rolled_back_apply_leaves_no_new_blob() {
        use sha2::{Digest, Sha256};

        let dir = std::env::temp_dir().join(format!(
            "holdover-lan-blob-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(&dir.join("desk.db")).unwrap();
        {
            let extra = rusqlite::Connection::open(dir.join("desk.db")).unwrap();
            extra
                .execute_batch(
                    "CREATE TRIGGER fail_apply BEFORE INSERT ON records
                     BEGIN SELECT RAISE(ABORT, 'forced'); END;",
                )
                .unwrap();
        }
        let bytes = b"%PDF-1.4 rollback";
        let hash = {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let digest = Sha256::digest(bytes);
            let mut text = String::with_capacity(64);
            for byte in digest {
                text.push(HEX[(byte >> 4) as usize] as char);
                text.push(HEX[(byte & 0x0f) as usize] as char);
            }
            text
        };
        let broken = LanCommit {
            writes: vec![Stamped {
                record: holdover_core::SyncRecord {
                    id: "would-roll-back".into(),
                    kind: "rifle".into(),
                    created_at: "2026-11-12T00:00:00.000Z".into(),
                    updated_at: "2026-11-12T00:00:00.000Z".into(),
                    revision: 1,
                    deleted_at: None,
                    device_id: "desk".into(),
                    body: json!({}),
                },
                fields: BTreeMap::new(),
            }],
            bases: Vec::new(),
        };
        assert!(store
            .lan_apply(&broken, &[(hash.clone(), bytes.to_vec())], false)
            .is_err());
        let blob = dir.join("desk.blobs").join(&hash);
        assert!(!blob.exists(), "a rolled-back apply left a blob file");
        {
            let extra = rusqlite::Connection::open(dir.join("desk.db")).unwrap();
            extra.execute_batch("DROP TRIGGER fail_apply;").unwrap();
        }
        let kept = LanCommit {
            writes: vec![Stamped {
                record: holdover_core::SyncRecord {
                    id: "ok".into(),
                    kind: "rifle".into(),
                    created_at: "2026-11-12T00:00:00.000Z".into(),
                    updated_at: "2026-11-12T00:00:00.000Z".into(),
                    revision: 1,
                    deleted_at: None,
                    device_id: "desk".into(),
                    body: json!({"label": "Kept"}),
                },
                fields: BTreeMap::new(),
            }],
            bases: Vec::new(),
        };
        store
            .lan_apply(&kept, &[(hash.clone(), bytes.to_vec())], false)
            .unwrap();
        assert!(blob.is_file(), "a committed apply did not store the blob");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ready_reuses_the_listener_and_a_new_pair_uses_that_port() {
        let _port = listen_port_lock();
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
        let probe = std::net::TcpListener::bind("0.0.0.0:43720");
        let stable_free = probe.is_ok();
        drop(probe);
        if !stable_free {
            return;
        }
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
            let expected = if tailscale_ipv6_only() {
                "tailscale-ipv6"
            } else {
                "no-tailscale"
            };
            assert_eq!(err, expected);
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

    fn wait_view(hub: &LanHub, store: &Arc<Mutex<Store>>, ready: impl Fn(&Value) -> bool) -> Value {
        let start = std::time::Instant::now();
        loop {
            let view = {
                let guard = store.lock().unwrap();
                hub.status(&guard).unwrap()
            };
            if ready(&view) || start.elapsed() > Duration::from_secs(6) {
                return view;
            }
            thread::sleep(Duration::from_millis(40));
        }
    }

    #[test]
    fn sync_begin_returns_before_the_peer_answers_and_then_shows_a_preview() {
        let dir = std::env::temp_dir().join(format!(
            "holdover-lan-now-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let desk = Store::open(&dir.join("desk.db")).unwrap();
        let phone = Store::open(&dir.join("phone.db")).unwrap();
        let desk_key = desk.lan_identity().unwrap();
        let phone_key = phone.lan_identity().unwrap();
        let phone_store = Arc::new(Mutex::new(phone));
        let listener = SyncListener::bind(desk_key.clone(), vec![phone_key.spki]).unwrap();
        let port = listener.port;
        let server = thread::spawn(move || {
            let mut listener = listener;
            let mut catalog = ArcCatalog::new(Arc::new(Mutex::new(desk))).unwrap();
            listener
                .serve_one(&mut catalog, Duration::from_secs(5))
                .map_err(|err| err.code)
        });
        let addr = format!("127.0.0.1:{port}").parse().unwrap();
        let mut hub = LanHub::default();
        let started = std::time::Instant::now();
        hub.begin_sync_for_test(
            phone_store.clone(),
            addr,
            desk_key.spki,
            &desk_key.spki_hex,
            Duration::from_secs(4),
        );
        assert!(
            started.elapsed() < Duration::from_millis(300),
            "sync begin waited for the peer"
        );
        let reaching = hub.status(&phone_store.lock().unwrap()).unwrap();
        assert_eq!(reaching["message"], "sync-reaching");
        let view = wait_view(&hub, &phone_store, |view| {
            view["mode"] == "preview" || !view["code"].as_str().unwrap_or("").is_empty()
        });
        assert_eq!(view["mode"], "preview", "{view}");
        let _ = hub.take_session();
        let _ = server.join();
        hub.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_phone_sync_shows_a_preview_while_the_desktop_keeps_accepting() {
        let dir = std::env::temp_dir().join(format!(
            "holdover-lan-phone-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let phone = Store::open(&dir.join("phone.db")).unwrap();
        let desk = Store::open(&dir.join("desk.db")).unwrap();
        phone
            .save_rifle(&json!({
                "label": "Pocket rifle",
                "sight_height": "38 mm",
                "twist": "1:8",
            }))
            .unwrap();
        let phone_key = phone.lan_identity().unwrap();
        let desk_key = desk.lan_identity().unwrap();
        desk.save_peer(&holdover_lan::Peer {
            device_id: phone_key.device_id.clone(),
            name: phone_key.name.clone(),
            spki_hex: phone_key.spki_hex.clone(),
            spki: phone_key.spki,
            host: "100.64.0.8".into(),
            port: DEFAULT_PORT,
        })
        .unwrap();
        let phone_store = Arc::new(Mutex::new(phone));
        let desk_store = Arc::new(Mutex::new(desk));
        let mut desk_hub = LanHub::default();
        let port = desk_hub.listen_for_test(desk_store.clone()).unwrap();
        let mut phone_hub = LanHub::default();
        let started = std::time::Instant::now();
        phone_hub.begin_sync_for_test(
            phone_store.clone(),
            format!("127.0.0.1:{port}").parse().unwrap(),
            desk_key.spki,
            &desk_key.spki_hex,
            Duration::from_secs(4),
        );
        assert!(
            started.elapsed() < Duration::from_millis(300),
            "the phone sync waited on the window thread"
        );
        let phone_view = wait_view(&phone_hub, &phone_store, |view| {
            view["mode"] == "preview" || !view["code"].as_str().unwrap_or("").is_empty()
        });
        assert_eq!(phone_view["mode"], "preview", "{phone_view}");
        assert!(
            !phone_view["preview"]["rows"].as_array().unwrap().is_empty(),
            "the phone preview was empty: {phone_view}"
        );
        let again = phone_hub.sync_begin(
            phone_store.clone(),
            "100.64.0.9",
            DEFAULT_PORT,
            &desk_key.spki_hex,
            false,
        );
        assert_eq!(again, Err("sync-busy".into()));
        let kept = phone_hub.status(&phone_store.lock().unwrap()).unwrap();
        assert_eq!(kept["mode"], "preview", "{kept}");
        let desk_started = std::time::Instant::now();
        let desk_view = desk_hub.status(&desk_store.lock().unwrap()).unwrap();
        assert!(
            desk_started.elapsed() < Duration::from_millis(200),
            "desktop status blocked while a phone sync was open"
        );
        assert_eq!(desk_view["message"], "sync-serving", "{desk_view}");
        let second_store = phone_store.clone();
        let second_key = phone_key;
        let desk_pin = desk_key.spki;
        let second = thread::spawn(move || {
            let mut catalog = ArcCatalog::new(second_store).unwrap();
            open_sync(
                &mut catalog,
                &second_key,
                format!("127.0.0.1:{port}").parse().unwrap(),
                desk_pin,
                false,
                Duration::from_secs(2),
            )
            .map(|_| "preview".to_string())
            .map_err(|err| err.code)
        });
        let second_started = std::time::Instant::now();
        let mut second_result = None;
        while second_started.elapsed() < Duration::from_secs(3) {
            if second.is_finished() {
                second_result = Some(second.join().unwrap());
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let second_result =
            second_result.expect("a second sync hung while the phone preview was open");
        assert_eq!(second_result, Err("sync-busy".to_string()));
        phone_hub.session_cancel();
        phone_hub.stop();
        desk_hub.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_half_open_incoming_session_does_not_block_a_phone_sync() {
        let dir = std::env::temp_dir().join(format!(
            "holdover-lan-half-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let phone = Store::open(&dir.join("phone.db")).unwrap();
        let desk = Store::open(&dir.join("desk.db")).unwrap();
        phone
            .save_rifle(&json!({
                "label": "Pocket rifle",
                "sight_height": "38 mm",
                "twist": "1:8",
            }))
            .unwrap();
        let phone_key = phone.lan_identity().unwrap();
        let desk_key = desk.lan_identity().unwrap();
        desk.save_peer(&holdover_lan::Peer {
            device_id: phone_key.device_id.clone(),
            name: phone_key.name.clone(),
            spki_hex: phone_key.spki_hex.clone(),
            spki: phone_key.spki,
            host: "100.64.0.8".into(),
            port: DEFAULT_PORT,
        })
        .unwrap();
        let phone_store = Arc::new(Mutex::new(phone));
        let desk_store = Arc::new(Mutex::new(desk));
        let mut desk_hub = LanHub::default();
        let port = desk_hub.listen_for_test(desk_store.clone()).unwrap();
        let stale = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        thread::sleep(Duration::from_millis(150));
        let mut phone_hub = LanHub::default();
        let started = std::time::Instant::now();
        phone_hub.begin_sync_for_test(
            phone_store.clone(),
            format!("127.0.0.1:{port}").parse().unwrap(),
            desk_key.spki,
            &desk_key.spki_hex,
            Duration::from_secs(4),
        );
        let view = wait_view(&phone_hub, &phone_store, |view| {
            view["mode"] == "preview" || !view["code"].as_str().unwrap_or("").is_empty()
        });
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "a half-open incoming session pinned the listener for {:?}",
            started.elapsed()
        );
        assert_eq!(view["mode"], "preview", "{view}");
        drop(stale);
        phone_hub.session_cancel();
        phone_hub.stop();
        desk_hub.stop();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_listener_is_up_after_launch_without_a_status_poll() {
        let _port = listen_port_lock();
        let probe = std::net::TcpListener::bind("0.0.0.0:43720");
        if probe.is_err() {
            return;
        }
        drop(probe);
        let dir = std::env::temp_dir().join(format!(
            "holdover-lan-launch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Arc::new(Mutex::new(Store::open(&dir.join("desk.db")).unwrap()));
        let hub = Arc::new(Mutex::new(LanHub::default()));
        arm_listener(hub.clone(), store.clone());
        let mut up = false;
        for _ in 0..50 {
            let guard = hub.lock().unwrap_or_else(|err| err.into_inner());
            let view = guard.status(&store.lock().unwrap()).unwrap();
            if view["listening"] == true && view["port"] == 43720 {
                assert_eq!(
                    guard.binds.load(Ordering::SeqCst),
                    1,
                    "launch bound 43720 more than once"
                );
                up = true;
                break;
            }
            drop(guard);
            thread::sleep(Duration::from_millis(20));
        }
        assert!(up, "the listener was not up without a status poll");
        let keeper = {
            let mut guard = hub.lock().unwrap_or_else(|err| err.into_inner());
            guard.ensure_listener(store.clone()).unwrap();
            let view = guard.status(&store.lock().unwrap()).unwrap();
            assert_eq!(view["listening"], true);
            assert_eq!(view["port"], 43720);
            assert_eq!(
                guard.binds.load(Ordering::SeqCst),
                1,
                "a status poll opened a second listener"
            );
            guard.disarm_keeper()
        };
        if let Some(keeper) = keeper {
            keeper.join().unwrap();
        }
        let closed = std::net::TcpListener::bind("0.0.0.0:43720");
        assert!(closed.is_ok(), "launch left 43720 open");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_during_a_handshake_returns_immediately() {
        let _port = listen_port_lock();
        let dir = std::env::temp_dir().join(format!(
            "holdover-lan-cancel-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let phone = Store::open(&dir.join("phone.db")).unwrap();
        let desk_key = holdover_lan::new_identity("desk", "Bench").unwrap();
        let phone_store = Arc::new(Mutex::new(phone));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let held = thread::spawn(move || {
            let _socket = listener.accept();
            thread::sleep(Duration::from_secs(3));
        });
        let mut hub = LanHub::default();
        hub.begin_sync_for_test(
            phone_store.clone(),
            format!("127.0.0.1:{port}").parse().unwrap(),
            desk_key.spki,
            &desk_key.spki_hex,
            Duration::from_secs(2),
        );
        let reaching = hub.status(&phone_store.lock().unwrap()).unwrap();
        assert_eq!(reaching["message"], "sync-reaching");
        let started = std::time::Instant::now();
        hub.cancel(phone_store.clone()).unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(300),
            "cancel waited on the handshake"
        );
        let view = hub.status(&phone_store.lock().unwrap()).unwrap();
        assert_ne!(view["message"], "sync-reaching", "{view}");
        thread::sleep(Duration::from_millis(500));
        let later = hub.status(&phone_store.lock().unwrap()).unwrap();
        assert_ne!(later["mode"], "preview", "{later}");
        hub.stop();
        let _ = held.join();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stalled_peer_becomes_a_visible_error_inside_the_handshake_bound() {
        let dir = std::env::temp_dir().join(format!(
            "holdover-lan-stall-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let phone = Store::open(&dir.join("phone.db")).unwrap();
        let desk_key = holdover_lan::new_identity("desk", "Bench").unwrap();
        let phone_store = Arc::new(Mutex::new(phone));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let held = thread::spawn(move || {
            let _socket = listener.accept();
            thread::sleep(Duration::from_secs(3));
        });
        let mut hub = LanHub::default();
        let started = std::time::Instant::now();
        hub.begin_sync_for_test(
            phone_store.clone(),
            format!("127.0.0.1:{port}").parse().unwrap(),
            desk_key.spki,
            &desk_key.spki_hex,
            Duration::from_secs(1),
        );
        assert!(started.elapsed() < Duration::from_millis(300));
        let view = wait_view(&hub, &phone_store, |view| {
            !view["code"].as_str().unwrap_or("").is_empty()
        });
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "the stalled sync was still open after {:?}",
            started.elapsed()
        );
        assert_eq!(view["code"], "sync-unreachable", "{view}");
        assert_ne!(view["mode"], "preview");
        hub.stop();
        let _ = held.join();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sync_panic_becomes_an_on_screen_error() {
        let shared = Arc::new(Mutex::new(LanView::default()));
        let payload = std::panic::catch_unwind(|| panic!("sync boom\nline")).unwrap_err();
        let text = panic_text(payload.as_ref());
        report_panic_text(&text);
        show_sync_error(&shared, "sync-failed", &text);
        let view = shared.lock().unwrap();
        assert_eq!(view.code, "sync-failed");
        assert!(view.detail.contains("sync boom"));
        assert!(!view.detail.contains('\n'));
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
