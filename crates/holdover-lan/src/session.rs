//! Pairing and one sync at a time. The database is not touched until both
//! sides agree to commit.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use holdover_core::{
    is_deleted, lan_commit, lan_preview, record_value, referenced_blobs, strip_private, FieldStamp,
    LanChoice, LanCommit, LanPreview, MergeStatus, Stamped, SyncBase, SyncRecord, LAN_SCHEMA,
    RECORD_KINDS,
};
use serde_json::{json, Map, Value};

use crate::discover::bind_listener;
use crate::identity::{make_offer, pin_from_hex, Identity, Offer};
use crate::tls::{
    accept_tls, connect_tls, ct_eq, hex_encode, peer_spki_of, read_json, server_config, sha256_hex,
    token_hash, write_json, ClientStream, LanError, ServerStream,
};

pub const PREVIEW_WAIT: Duration = Duration::from_secs(600);

pub trait Catalog {
    fn schema(&self) -> i64 {
        LAN_SCHEMA
    }
    fn device_id(&self) -> &str;
    fn library(&self) -> Result<Library, LanError>;
    fn blob(&self, hash: &str) -> Result<Option<Vec<u8>>, LanError>;
    fn apply(
        &mut self,
        commit: &LanCommit,
        blobs: &[(String, Vec<u8>)],
        include_location: bool,
    ) -> Result<(), LanError>;
}

#[derive(Clone, Debug)]
pub struct Library {
    pub records: Vec<Stamped>,
    pub bases: BTreeMap<String, SyncBase>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    pub device_id: String,
    pub name: String,
    pub spki_hex: String,
    pub spki: [u8; 32],
    pub host: String,
    pub port: u16,
}

pub struct PairWait {
    listener: TcpListener,
    identity: Identity,
    offer: Offer,
    consumed: Arc<AtomicBool>,
    config: Arc<rustls::ServerConfig>,
    tailscale_only: bool,
}

pub struct Pending {
    stream: ServerStream,
    device_id: String,
    name: String,
    spki: [u8; 32],
    host: String,
    hosts: Vec<String>,
    consumed: Arc<AtomicBool>,
}

impl PairWait {
    pub fn start(
        identity: Identity,
        hosts: Vec<String>,
        now: u64,
    ) -> Result<(Self, Offer), LanError> {
        Self::start_on(identity, hosts, now, 0)
    }

    pub fn start_on(
        identity: Identity,
        hosts: Vec<String>,
        now: u64,
        preferred_port: u16,
    ) -> Result<(Self, Offer), LanError> {
        let (listener, port) = bind_listener(preferred_port)?;
        let offer = make_offer(&identity, port, hosts, now);
        let config = server_config(&identity)?;
        Ok((
            Self {
                listener,
                identity,
                offer: offer.clone(),
                consumed: Arc::new(AtomicBool::new(false)),
                config,
                tailscale_only: false,
            },
            offer,
        ))
    }

    /// Drop TCP connections that are not from 100.64.0.0/10 before the handshake.
    pub fn accept_tailscale_only(&mut self) {
        self.tailscale_only = true;
    }

    pub fn offer(&self) -> &Offer {
        &self.offer
    }

    pub fn device_name(&self) -> &str {
        &self.identity.name
    }

    pub fn device_id(&self) -> &str {
        &self.identity.device_id
    }

    pub fn port(&self) -> u16 {
        self.offer.port
    }

    pub fn next(&mut self, now: u64, timeout: Duration) -> Result<Pending, LanError> {
        let stream = accept_one(&self.listener, timeout)?;
        if self.tailscale_only && !stream_is_tailscale(&stream) {
            drop(stream);
            return Err(LanError::new("not-tailscale"));
        }
        // A socket that never starts TLS must not sit here for the whole
        // accept wait. The phone may have another address connected.
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|_| LanError::new("disconnected"))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|_| LanError::new("disconnected"))?;
        let mut tls = accept_tls(stream, self.config.clone()).map_err(|err| {
            if err.code == "tls" {
                LanError::new("tls")
            } else {
                err
            }
        })?;
        let peer = peer_spki_of(tls.conn.peer_certificates())?;
        let hello = read_json(&mut tls)?;
        if hello.get("t").and_then(Value::as_str) != Some("pair-hello") {
            write_json(
                &mut tls,
                &json!({"t": "pair-reject", "code": "pair-invalid"}),
            )?;
            return Err(LanError::new("pair-invalid"));
        }
        let schema = hello.get("schema").and_then(Value::as_i64).unwrap_or(0);
        if schema > LAN_SCHEMA {
            refuse(&mut tls, "schema-newer")?;
            return Err(LanError::new("schema-newer"));
        }
        if schema < LAN_SCHEMA {
            refuse(&mut tls, "schema-older")?;
            return Err(LanError::new("schema-older"));
        }
        let token = hello.get("token").and_then(Value::as_str).unwrap_or("");
        let hash = token_hash(token);
        let expected = token_hash(&self.offer.token);
        if self.consumed.load(Ordering::SeqCst) && ct_eq(&hash, &expected) {
            refuse(&mut tls, "pair-replayed")?;
            return Err(LanError::new("pair-replayed"));
        }
        if !ct_eq(&hash, &expected) {
            refuse(&mut tls, "pair-invalid")?;
            return Err(LanError::new("pair-invalid"));
        }
        if now >= self.offer.expires {
            refuse(&mut tls, "pair-expired")?;
            return Err(LanError::new("pair-expired"));
        }
        let device_id = hello
            .get("device_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let name = hello
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if device_id.is_empty() {
            refuse(&mut tls, "pair-invalid")?;
            return Err(LanError::new("pair-invalid"));
        }
        let host = tls
            .sock
            .peer_addr()
            .map(|addr| addr.ip().to_string())
            .unwrap_or_default();
        let _ = tls.sock.set_read_timeout(Some(PREVIEW_WAIT));
        let _ = tls.sock.set_write_timeout(Some(PREVIEW_WAIT));
        let hosts = announced_hosts(hello.get("hosts"));
        Ok(Pending {
            stream: tls,
            device_id,
            name,
            spki: peer,
            host,
            hosts,
            consumed: self.consumed.clone(),
        })
    }
}

impl std::fmt::Debug for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pending")
            .field("device_id", &self.device_id)
            .field("name", &self.name)
            .field("fingerprint", &self.fingerprint())
            .finish()
    }
}

impl Pending {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn fingerprint(&self) -> String {
        hex_encode(&self.spki)
    }

    pub fn client_hosts(&self) -> &[String] {
        &self.hosts
    }

    pub fn accept_with(mut self, server_name: &str, server_device: &str) -> Result<Peer, LanError> {
        write_json(
            &mut self.stream,
            &json!({
                "t": "pair-accept",
                "device_id": server_device,
                "name": server_name,
                "schema": LAN_SCHEMA,
            }),
        )?;
        let ack = read_json(&mut self.stream)?;
        if ack.get("t").and_then(Value::as_str) != Some("pair-ack") {
            return Err(LanError::new("disconnected"));
        }
        self.consumed.store(true, Ordering::SeqCst);
        Ok(Peer {
            device_id: self.device_id,
            name: self.name,
            spki_hex: hex_encode(&self.spki),
            spki: self.spki,
            host: self.host,
            port: 0,
        })
    }

    pub fn reject(mut self) -> Result<(), LanError> {
        write_json(&mut self.stream, &json!({"t": "pair-denied"}))?;
        Ok(())
    }
}

pub fn join_pair(
    identity: &Identity,
    offer: &Offer,
    now: u64,
    timeout: Duration,
) -> Result<Peer, LanError> {
    join_pair_with(identity, offer, now, timeout, |_| {})
}

/// `progress` receives `reaching` while addresses are tried and `awaiting`
/// after the pinned session is up and the other side has yet to accept.
pub fn join_pair_with(
    identity: &Identity,
    offer: &Offer,
    now: u64,
    timeout: Duration,
    mut progress: impl FnMut(&str),
) -> Result<Peer, LanError> {
    if offer.schema > LAN_SCHEMA {
        return Err(LanError::new("schema-newer"));
    }
    if offer.schema < LAN_SCHEMA {
        return Err(LanError::new("schema-older"));
    }
    if now >= offer.expires {
        return Err(LanError::new("pair-expired"));
    }
    let pin = pin_from_hex(&offer.spki_hex)?;
    progress("reaching");
    let mut last = LanError::new("pair-unreachable");
    let (tx, rx) = mpsc::channel();
    let mut started = 0u32;
    for host in &offer.hosts {
        let Some(addr) = socket_for(host, offer.port) else {
            continue;
        };
        started += 1;
        let tx = tx.clone();
        let host = host.clone();
        thread::spawn(move || {
            if let Ok(stream) = TcpStream::connect_timeout(&addr, timeout) {
                let _ = stream.set_read_timeout(Some(PREVIEW_WAIT));
                let _ = stream.set_write_timeout(Some(PREVIEW_WAIT));
                let _ = tx.send((host, stream));
            }
        });
    }
    drop(tx);
    if started == 0 {
        return Err(LanError::new("pair-unreachable"));
    }
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let (host, stream) = match rx.recv_timeout(remaining) {
            Ok(item) => item,
            Err(_) => break,
        };
        match pair_on_stream(stream, identity, offer, pin, &host, &mut progress) {
            Ok(peer) => return Ok(peer),
            Err(err) if matches!(err.code.as_str(), "tls" | "pair-pin" | "disconnected") => {
                last = err;
            }
            Err(err) => return Err(err),
        }
    }
    Err(last)
}

fn announced_hosts(value: Option<&Value>) -> Vec<String> {
    let mut hosts = Vec::new();
    let Some(list) = value.and_then(Value::as_array) else {
        return hosts;
    };
    for item in list.iter().take(8) {
        let Some(text) = item.as_str() else {
            continue;
        };
        let Ok(addr) = text.parse::<std::net::Ipv4Addr>() else {
            continue;
        };
        if addr.is_unspecified() || addr.is_broadcast() || addr.is_link_local() {
            continue;
        }
        let text = addr.to_string();
        if !hosts.contains(&text) {
            hosts.push(text);
        }
    }
    crate::discover::sort_hosts(&mut hosts);
    hosts
}

fn socket_for(host: &str, port: u16) -> Option<SocketAddr> {
    let text = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    text.parse().ok()
}

fn pair_on_stream(
    stream: TcpStream,
    identity: &Identity,
    offer: &Offer,
    pin: [u8; 32],
    host: &str,
    progress: &mut impl FnMut(&str),
) -> Result<Peer, LanError> {
    // The handshake is short. The wait after the hello is the other person
    // reading the request and pressing Accept.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(4)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(4)));
    let mut tls = connect_tls(stream, identity, pin)?;
    write_json(
        &mut tls,
        &json!({
            "t": "pair-hello",
            "token": offer.token,
            "schema": LAN_SCHEMA,
            "device_id": identity.device_id,
            "name": identity.name,
            "hosts": crate::discover::tailscale_hosts(),
        }),
    )?;
    let _ = tls.sock.set_read_timeout(Some(PREVIEW_WAIT));
    let _ = tls.sock.set_write_timeout(Some(PREVIEW_WAIT));
    progress("awaiting");
    let reply = read_json(&mut tls)?;
    let kind = reply.get("t").and_then(Value::as_str).unwrap_or("");
    if kind == "pair-reject" {
        return Err(LanError::new(
            reply
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("pair-invalid"),
        ));
    }
    if kind == "pair-denied" {
        return Err(LanError::new("pair-denied"));
    }
    if kind != "pair-accept" {
        return Err(LanError::new("disconnected"));
    }
    let schema = reply.get("schema").and_then(Value::as_i64).unwrap_or(0);
    if schema != LAN_SCHEMA {
        return Err(LanError::new(if schema > LAN_SCHEMA {
            "schema-newer"
        } else {
            "schema-older"
        }));
    }
    write_json(&mut tls, &json!({"t": "pair-ack"}))?;
    Ok(Peer {
        device_id: reply
            .get("device_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        name: reply
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(&offer.name)
            .to_string(),
        spki_hex: offer.spki_hex.clone(),
        spki: pin,
        host: host.to_string(),
        port: offer.port,
    })
}

pub struct SyncListener {
    listener: TcpListener,
    identity: Identity,
    pins: Vec<[u8; 32]>,
    config: Arc<rustls::ServerConfig>,
    pub port: u16,
    tailscale_only: bool,
}

impl SyncListener {
    pub fn bind(identity: Identity, pins: Vec<[u8; 32]>) -> Result<Self, LanError> {
        Self::bind_on(identity, pins, 0)
    }

    pub fn bind_on(
        identity: Identity,
        pins: Vec<[u8; 32]>,
        preferred_port: u16,
    ) -> Result<Self, LanError> {
        let (listener, port) = bind_listener(preferred_port)?;
        let config = server_config(&identity)?;
        Ok(Self {
            listener,
            identity,
            pins,
            config,
            port,
            tailscale_only: false,
        })
    }

    /// Drop TCP connections that are not from 100.64.0.0/10 before the handshake.
    pub fn accept_tailscale_only(&mut self) {
        self.tailscale_only = true;
    }

    pub fn serve_one<C: Catalog>(
        &mut self,
        catalog: &mut C,
        timeout: Duration,
    ) -> Result<(), LanError> {
        let tcp = accept_one(&self.listener, timeout)?;
        if self.tailscale_only && !stream_is_tailscale(&tcp) {
            drop(tcp);
            return Err(LanError::new("not-tailscale"));
        }
        let mut tls = match accept_tls(tcp, self.config.clone()) {
            Ok(tls) => tls,
            Err(_) => return Err(LanError::new("tls")),
        };
        let peer = match peer_spki_of(tls.conn.peer_certificates()) {
            Ok(peer) => peer,
            Err(_) => return Err(LanError::new("tls")),
        };
        let hello = read_json(&mut tls)?;
        if hello.get("t").and_then(Value::as_str) != Some("sync-hello") {
            write_json(
                &mut tls,
                &json!({"t": "sync-refuse", "code": "unauthorized"}),
            )?;
            return Err(LanError::new("unauthorized"));
        }
        if !self.pins.iter().any(|pin| pin == &peer) {
            write_json(
                &mut tls,
                &json!({"t": "sync-refuse", "code": "unauthorized"}),
            )?;
            return Err(LanError::new("unauthorized"));
        }
        let schema = hello.get("schema").and_then(Value::as_i64).unwrap_or(0);
        if schema > LAN_SCHEMA {
            write_json(
                &mut tls,
                &json!({"t": "sync-refuse", "code": "schema-newer"}),
            )?;
            return Err(LanError::new("schema-newer"));
        }
        if schema < LAN_SCHEMA {
            write_json(
                &mut tls,
                &json!({"t": "sync-refuse", "code": "schema-older"}),
            )?;
            return Err(LanError::new("schema-older"));
        }
        if catalog.schema() != LAN_SCHEMA {
            let code = if catalog.schema() > LAN_SCHEMA {
                "schema-newer"
            } else {
                "schema-older"
            };
            write_json(&mut tls, &json!({"t": "sync-refuse", "code": code}))?;
            return Err(LanError::new(code));
        }
        let include_location = hello
            .get("include_location")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        write_json(
            &mut tls,
            &json!({"t": "sync-hello", "schema": LAN_SCHEMA, "device_id": self.identity.device_id}),
        )?;
        let remote_msg = read_json(&mut tls)?;
        let remote = parse_library(remote_msg.get("library").unwrap_or(&Value::Null))?;
        let local = stripped(&catalog.library()?, include_location);
        write_json(
            &mut tls,
            &json!({"t": "snapshot", "library": library_value(&local)}),
        )?;
        let choice_msg = read_json(&mut tls)?;
        if choice_msg.get("t").and_then(Value::as_str) == Some("abort") {
            return Err(LanError::new(
                choice_msg
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("disconnected"),
            ));
        }
        let choices = parse_choices(choice_msg.get("choices").unwrap_or(&json!({})))?;
        let preview = lan_preview(&remote.records, &local.records, &remote.bases, &local.bases);
        let commit = match lan_commit(&preview, &choices) {
            Ok(commit) => commit,
            Err(_) => {
                write_json(&mut tls, &json!({"t": "abort", "code": "unresolved"}))?;
                return Err(LanError::new("unresolved"));
            }
        };
        let got = exchange_blobs(&mut tls, catalog, &commit)?;
        let go = read_json(&mut tls)?;
        if go.get("t").and_then(Value::as_str) != Some("commit-now") {
            return Err(LanError::new("disconnected"));
        }
        if let Err(err) = catalog.apply(&commit, &got, include_location) {
            let _ = write_json(&mut tls, &json!({"t": "failed", "code": err.code}));
            return Err(err);
        }
        write_json(&mut tls, &json!({"t": "committed"}))?;
        Ok(())
    }
}

pub struct ClientSync {
    stream: Option<ClientStream>,
    pub preview: LanPreview,
    include_location: bool,
    peer: Peer,
}

impl std::fmt::Debug for ClientSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientSync")
            .field("peer", &self.peer.device_id)
            .field("rows", &self.preview.rows.len())
            .finish()
    }
}

impl Drop for ClientSync {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.as_mut() {
            let _ = stream.sock.shutdown(Shutdown::Both);
        }
    }
}

impl ClientSync {
    pub fn peer(&self) -> &Peer {
        &self.peer
    }

    pub fn finish<C: Catalog>(
        mut self,
        catalog: &mut C,
        choices: &BTreeMap<String, LanChoice>,
    ) -> Result<(), LanError> {
        let mut stream = self
            .stream
            .take()
            .ok_or_else(|| LanError::new("disconnected"))?;
        let commit = match lan_commit(&self.preview, choices) {
            Ok(commit) => commit,
            Err(_) => {
                let _ = write_json(&mut stream, &json!({"t": "abort", "code": "unresolved"}));
                return Err(LanError::new("unresolved"));
            }
        };
        write_json(
            &mut stream,
            &json!({"t": "choices", "choices": choices_value(choices)}),
        )?;
        let reply = read_json(&mut stream)?;
        if reply.get("t").and_then(Value::as_str) == Some("abort") {
            return Err(LanError::new(
                reply
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("unresolved"),
            ));
        }
        let got = finish_blobs(&mut stream, catalog, &commit, &reply)?;
        write_json(&mut stream, &json!({"t": "commit-now"}))?;
        let done = read_json(&mut stream)?;
        if done.get("t").and_then(Value::as_str) != Some("committed") {
            return Err(LanError::new(
                done.get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("disconnected"),
            ));
        }
        catalog.apply(&commit, &got, self.include_location)?;
        Ok(())
    }
}

pub fn open_sync<C: Catalog>(
    catalog: &mut C,
    identity: &Identity,
    address: SocketAddr,
    server_pin: [u8; 32],
    include_location: bool,
    timeout: Duration,
) -> Result<ClientSync, LanError> {
    let tcp = dial(address, timeout)?;
    open_sync_connected(
        catalog,
        identity,
        tcp,
        address,
        server_pin,
        include_location,
    )
}

pub fn open_sync_many<C: Catalog>(
    catalog: &mut C,
    identity: &Identity,
    addresses: &[SocketAddr],
    server_pin: [u8; 32],
    include_location: bool,
    timeout: Duration,
) -> Result<ClientSync, LanError> {
    let targets: Vec<SocketAddr> = addresses
        .iter()
        .copied()
        .filter(|addr| addr.port() != 0)
        .collect();
    if targets.is_empty() {
        return Err(LanError::with_detail("sync-unreachable", "no port"));
    }
    let (tx, rx) = mpsc::channel();
    for address in &targets {
        let tx = tx.clone();
        let address = *address;
        thread::spawn(move || {
            let outcome = match TcpStream::connect_timeout(&address, timeout) {
                Ok(stream) => Ok((address, stream)),
                Err(err) => Err((address, dial_why(err.kind()))),
            };
            let _ = tx.send(outcome);
        });
    }
    drop(tx);
    let mut pending = targets;
    let mut tried: Vec<(SocketAddr, String)> = Vec::new();
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let outcome = if remaining.is_zero() {
            match rx.try_recv() {
                Ok(item) => item,
                Err(_) => break,
            }
        } else {
            match rx.recv_timeout(remaining) {
                Ok(item) => item,
                Err(_) => match rx.try_recv() {
                    Ok(item) => item,
                    Err(_) => break,
                },
            }
        };
        match outcome {
            Ok((address, stream)) => {
                pending.retain(|item| *item != address);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(4)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(4)));
                match open_sync_connected(
                    catalog,
                    identity,
                    stream,
                    address,
                    server_pin,
                    include_location,
                ) {
                    Ok(session) => return Ok(session),
                    Err(err)
                        if matches!(err.code.as_str(), "tls" | "disconnected" | "unauthorized") =>
                    {
                        tried.push((address, err.code));
                    }
                    Err(err) => return Err(err),
                }
            }
            Err((address, why)) => {
                pending.retain(|item| *item != address);
                tried.push((address, why.to_string()));
            }
        }
    }
    for address in pending {
        tried.push((address, "timeout".to_string()));
    }
    Err(LanError::with_detail(
        "sync-unreachable",
        describe_tries(&tried),
    ))
}

fn dial_why(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::TimedOut => "timeout",
        ErrorKind::ConnectionRefused => "refused",
        ErrorKind::ConnectionReset => "reset",
        ErrorKind::ConnectionAborted => "reset",
        _ => "unreachable",
    }
}

fn describe_tries(tries: &[(SocketAddr, String)]) -> String {
    if tries.is_empty() {
        return "no port".into();
    }
    tries
        .iter()
        .filter(|(addr, _)| addr.port() != 0)
        .map(|(addr, why)| format!("tried {addr} {why}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn open_sync_connected<C: Catalog>(
    catalog: &mut C,
    identity: &Identity,
    tcp: TcpStream,
    address: SocketAddr,
    server_pin: [u8; 32],
    include_location: bool,
) -> Result<ClientSync, LanError> {
    let mut tls = connect_tls(tcp, identity, server_pin).map_err(|err| {
        if err.code == "pair-pin" {
            LanError::new("unauthorized")
        } else {
            err
        }
    })?;
    write_json(
        &mut tls,
        &json!({
            "t": "sync-hello",
            "schema": catalog.schema(),
            "device_id": catalog.device_id(),
            "include_location": include_location,
        }),
    )?;
    let hello = read_json(&mut tls)?;
    match hello.get("t").and_then(Value::as_str) {
        Some("sync-refuse") => {
            return Err(LanError::new(
                hello
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("unauthorized"),
            ));
        }
        Some("sync-hello") => {}
        _ => return Err(LanError::new("disconnected")),
    }
    let schema = hello.get("schema").and_then(Value::as_i64).unwrap_or(0);
    if schema > LAN_SCHEMA {
        return Err(LanError::new("schema-newer"));
    }
    if schema < LAN_SCHEMA {
        return Err(LanError::new("schema-older"));
    }
    let local = stripped(&catalog.library()?, include_location);
    write_json(
        &mut tls,
        &json!({"t": "snapshot", "library": library_value(&local)}),
    )?;
    let remote_msg = read_json(&mut tls)?;
    if remote_msg.get("t").and_then(Value::as_str) != Some("snapshot") {
        return Err(LanError::new("disconnected"));
    }
    let remote = parse_library(remote_msg.get("library").unwrap_or(&Value::Null))?;
    // The initiator's copy is "local" in the preview the person resolves.
    let preview = lan_preview(&local.records, &remote.records, &local.bases, &remote.bases);
    let peer_spki = server_pin;
    Ok(ClientSync {
        stream: Some(tls),
        preview,
        include_location,
        peer: Peer {
            device_id: hello
                .get("device_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            name: String::new(),
            spki_hex: hex_encode(&peer_spki),
            spki: peer_spki,
            host: address.ip().to_string(),
            port: address.port(),
        },
    })
}

fn exchange_blobs<C: Catalog>(
    stream: &mut ServerStream,
    catalog: &C,
    commit: &LanCommit,
) -> Result<Vec<(String, Vec<u8>)>, LanError> {
    let need = missing(catalog, commit)?;
    write_json(stream, &json!({"t": "need", "hashes": need}))?;
    let incoming = read_json(stream)?;
    let got = blobs_from_message(&incoming)?;
    let ask = read_json(stream)?;
    let asked = ask
        .get("hashes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut send = Vec::new();
    for hash in asked {
        match catalog.blob(&hash)? {
            Some(bytes) if sha256_hex(&bytes) == hash && attachment_ok(&bytes) => {
                send.push((hash, bytes))
            }
            _ => {
                write_json(stream, &json!({"t": "failed", "code": "blob"}))?;
                return Err(LanError::new("blob"));
            }
        }
    }
    write_json(stream, &blob_message(&send))?;
    Ok(got)
}

fn finish_blobs<C: Catalog>(
    stream: &mut ClientStream,
    catalog: &C,
    commit: &LanCommit,
    first: &Value,
) -> Result<Vec<(String, Vec<u8>)>, LanError> {
    if first.get("t").and_then(Value::as_str) != Some("need") {
        return Err(LanError::new("disconnected"));
    }
    let asked = first
        .get("hashes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut send = Vec::new();
    for hash in asked.iter().filter_map(Value::as_str) {
        match catalog.blob(hash)? {
            Some(bytes) if sha256_hex(&bytes) == hash && attachment_ok(&bytes) => {
                send.push((hash.to_string(), bytes));
            }
            _ => {
                let _ = write_json(stream, &json!({"t": "failed", "code": "blob"}));
                return Err(LanError::new("blob"));
            }
        }
    }
    write_json(stream, &blob_message(&send))?;
    let need = missing(catalog, commit)?;
    write_json(stream, &json!({"t": "need", "hashes": need}))?;
    let incoming = read_json(stream)?;
    if incoming.get("t").and_then(Value::as_str) == Some("failed") {
        return Err(LanError::new(
            incoming
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("blob"),
        ));
    }
    blobs_from_message(&incoming)
}

fn missing<C: Catalog>(catalog: &C, commit: &LanCommit) -> Result<Vec<String>, LanError> {
    let records: Vec<SyncRecord> = commit.writes.iter().map(|row| row.record.clone()).collect();
    let mut need = Vec::new();
    for hash in referenced_blobs(&records) {
        match catalog.blob(&hash)? {
            Some(bytes) if sha256_hex(&bytes) == hash => {}
            _ => need.push(hash),
        }
    }
    Ok(need)
}

fn blob_message(items: &[(String, Vec<u8>)]) -> Value {
    let blobs: Vec<Value> = items
        .iter()
        .map(|(hash, bytes)| {
            json!({
                "hash": hash,
                "b64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes),
            })
        })
        .collect();
    json!({"t": "blobs", "items": blobs})
}

fn blobs_from_message(value: &Value) -> Result<Vec<(String, Vec<u8>)>, LanError> {
    if value.get("t").and_then(Value::as_str) != Some("blobs") {
        return Err(LanError::new("disconnected"));
    }
    let mut out = Vec::new();
    for item in value
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let hash = item.get("hash").and_then(Value::as_str).unwrap_or("");
        let b64 = item.get("b64").and_then(Value::as_str).unwrap_or("");
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
            .map_err(|_| LanError::new("blob"))?;
        if sha256_hex(&bytes) != hash || !attachment_ok(&bytes) {
            return Err(LanError::new("blob"));
        }
        out.push((hash.to_string(), bytes));
    }
    Ok(out)
}

fn attachment_ok(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF")
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xFF, 0xD8, 0xFF])
}

fn refuse(stream: &mut ServerStream, code: &str) -> Result<(), LanError> {
    write_json(stream, &json!({"t": "pair-reject", "code": code}))
}

fn stream_is_tailscale(stream: &TcpStream) -> bool {
    match stream.peer_addr() {
        Ok(addr) => match addr.ip() {
            IpAddr::V4(ip) => crate::discover::is_tailscale(&ip.to_string()),
            IpAddr::V6(_) => false,
        },
        Err(_) => false,
    }
}

fn accept_one(listener: &TcpListener, timeout: Duration) -> Result<TcpStream, LanError> {
    listener
        .set_nonblocking(true)
        .map_err(|_| LanError::new("disconnected"))?;
    let start = Instant::now();
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                if start.elapsed() > timeout {
                    return Err(LanError::new("disconnected"));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return Err(LanError::new("disconnected")),
        }
    };
    stream
        .set_nonblocking(false)
        .map_err(|_| LanError::new("disconnected"))?;
    // The accept wait is short so a cancelled listen can stop. The sync itself
    // stays open long enough for a person to read the preview.
    stream
        .set_read_timeout(Some(PREVIEW_WAIT))
        .map_err(|_| LanError::new("disconnected"))?;
    stream
        .set_write_timeout(Some(PREVIEW_WAIT))
        .map_err(|_| LanError::new("disconnected"))?;
    Ok(stream)
}

fn dial(addr: SocketAddr, timeout: Duration) -> Result<TcpStream, LanError> {
    let stream =
        TcpStream::connect_timeout(&addr, timeout).map_err(|_| LanError::new("disconnected"))?;
    stream.set_read_timeout(Some(PREVIEW_WAIT)).ok();
    stream.set_write_timeout(Some(PREVIEW_WAIT)).ok();
    Ok(stream)
}

pub fn stripped(library: &Library, include_location: bool) -> Library {
    let records = library
        .records
        .iter()
        .map(|row| {
            let mut copy = row.clone();
            strip_private(&copy.record.kind, &mut copy.record.body, include_location);
            let live = holdover_core::field_values(&copy.record.body);
            copy.fields.retain(|key, _| live.contains_key(key));
            copy
        })
        .collect();
    Library {
        records,
        bases: library.bases.clone(),
    }
}

fn library_value(library: &Library) -> Value {
    json!({
        "records": library.records.iter().map(stamped_value).collect::<Vec<_>>(),
        "bases": bases_value(&library.bases),
    })
}

fn stamped_value(row: &Stamped) -> Value {
    let mut value = record_value(&row.record);
    let mut fields = Map::new();
    for (key, stamp) in &row.fields {
        fields.insert(
            key.clone(),
            json!({"updated_at": stamp.updated_at, "device_id": stamp.device_id, "revision": stamp.revision}),
        );
    }
    value["fields"] = Value::Object(fields);
    value
}

fn parse_library(value: &Value) -> Result<Library, LanError> {
    let records = value
        .get("records")
        .and_then(Value::as_array)
        .ok_or_else(|| LanError::new("corrupt"))?;
    let mut parsed = Vec::new();
    for item in records {
        parsed.push(parse_stamped(item)?);
    }
    let mut bases = BTreeMap::new();
    if let Some(map) = value.get("bases").and_then(Value::as_object) {
        for (id, base) in map {
            bases.insert(id.clone(), parse_base(base)?);
        }
    }
    Ok(Library {
        records: parsed,
        bases,
    })
}

fn parse_stamped(value: &Value) -> Result<Stamped, LanError> {
    let object = value.as_object().ok_or_else(|| LanError::new("corrupt"))?;
    let kind = text(object, "kind")?;
    if !RECORD_KINDS.contains(&kind.as_str()) {
        return Err(LanError::new("corrupt"));
    }
    let revision = object
        .get("revision")
        .and_then(Value::as_i64)
        .filter(|rev| *rev >= 1)
        .ok_or_else(|| LanError::new("corrupt"))?;
    let deleted_at = match object.get("deleted_at") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) if text.is_empty() => None,
        Some(Value::String(text)) => Some(text.clone()),
        _ => return Err(LanError::new("corrupt")),
    };
    let body = object
        .get("body")
        .filter(|item| item.is_object())
        .cloned()
        .ok_or_else(|| LanError::new("corrupt"))?;
    let mut fields = BTreeMap::new();
    if let Some(map) = object.get("fields").and_then(Value::as_object) {
        for (key, stamp) in map {
            fields.insert(
                key.clone(),
                FieldStamp {
                    updated_at: stamp
                        .get("updated_at")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    device_id: stamp
                        .get("device_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    revision: stamp.get("revision").and_then(Value::as_i64).unwrap_or(1),
                },
            );
        }
    }
    Ok(Stamped {
        record: SyncRecord {
            id: text(object, "id")?,
            kind,
            created_at: text(object, "created_at")?,
            updated_at: text(object, "updated_at")?,
            revision,
            deleted_at,
            device_id: text(object, "device_id")?,
            body,
        },
        fields,
    })
}

fn parse_base(value: &Value) -> Result<SyncBase, LanError> {
    let object = value.as_object().ok_or_else(|| LanError::new("corrupt"))?;
    let mut fields = BTreeMap::new();
    if let Some(map) = object.get("fields").and_then(Value::as_object) {
        for (key, stamp) in map {
            fields.insert(
                key.clone(),
                FieldStamp {
                    updated_at: stamp
                        .get("updated_at")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    device_id: stamp
                        .get("device_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    revision: stamp.get("revision").and_then(Value::as_i64).unwrap_or(1),
                },
            );
        }
    }
    Ok(SyncBase {
        device_id: text(object, "device_id")?,
        revision: object.get("revision").and_then(Value::as_i64).unwrap_or(1),
        updated_at: text(object, "updated_at")?,
        deleted: object
            .get("deleted")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        devices: Vec::new(),
        seen: Vec::new(),
        fields,
    })
}

fn bases_value(bases: &BTreeMap<String, SyncBase>) -> Value {
    let mut map = Map::new();
    for (id, base) in bases {
        let mut fields = Map::new();
        for (key, stamp) in &base.fields {
            fields.insert(
                key.clone(),
                json!({"updated_at": stamp.updated_at, "device_id": stamp.device_id, "revision": stamp.revision}),
            );
        }
        map.insert(
            id.clone(),
            json!({
                "device_id": base.device_id,
                "revision": base.revision,
                "updated_at": base.updated_at,
                "deleted": base.deleted,
                "fields": Value::Object(fields),
            }),
        );
    }
    Value::Object(map)
}

fn choices_value(choices: &BTreeMap<String, LanChoice>) -> Value {
    let mut map = Map::new();
    for (id, choice) in choices {
        map.insert(
            id.clone(),
            match choice {
                LanChoice::KeepMine => json!({"choice": "keep-mine"}),
                LanChoice::TakeTheirs => json!({"choice": "take-theirs"}),
                LanChoice::KeepBoth { copy_id } => {
                    json!({"choice": "keep-both", "copy_id": copy_id})
                }
                LanChoice::Fields(fields) => {
                    let mut picked = Map::new();
                    for (key, side) in fields {
                        picked.insert(key.clone(), json!(side.as_str()));
                    }
                    json!({"choice": "fields", "fields": Value::Object(picked)})
                }
            },
        );
    }
    Value::Object(map)
}

fn parse_choices(value: &Value) -> Result<BTreeMap<String, LanChoice>, LanError> {
    let mut out = BTreeMap::new();
    let Some(map) = value.as_object() else {
        return Ok(out);
    };
    for (id, choice) in map {
        let parsed = LanChoice::parse(choice).ok_or_else(|| LanError::new("unresolved"))?;
        out.insert(id.clone(), parsed);
    }
    Ok(out)
}

fn text(object: &Map<String, Value>, key: &str) -> Result<String, LanError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| LanError::new("corrupt"))
}

pub fn preview_value(preview: &LanPreview) -> Value {
    let rows: Vec<Value> = preview
        .rows
        .iter()
        .filter(|row| row.status != MergeStatus::Unchanged)
        .map(|row| {
            json!({
                "id": row.id,
                "kind": row.kind,
                "status": row.status.as_str(),
                "push": row.push,
                "label": row.label,
                "local_label": row.local_label,
                "remote_label": row.remote_label,
                "keep_both": row.keep_both,
                "conflicts": row.conflicts.iter().map(|item| json!({
                    "field": item.field,
                    "local": item.local_text,
                    "remote": item.remote_text,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "schema_version": preview.schema,
        "unchanged": unchanged_count(preview),
        "rows": rows,
    })
}

pub fn unchanged_count(preview: &LanPreview) -> usize {
    preview
        .rows
        .iter()
        .filter(|row| row.status == MergeStatus::Unchanged)
        .count()
}

#[allow(dead_code)]
fn _deleted(record: &SyncRecord) -> bool {
    is_deleted(record)
}

#[cfg(test)]
mod tests {
    use super::socket_for;

    #[test]
    fn an_ipv6_host_is_bracketed_before_the_port() {
        let addr = socket_for("::1", 9).expect("address");
        assert!(addr.is_ipv6());
        assert_eq!(addr.port(), 9);
        assert_eq!(socket_for("127.0.0.1", 443).unwrap().port(), 443);
        assert!(socket_for("not a host", 1).is_none());
    }
}
