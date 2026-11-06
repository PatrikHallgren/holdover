//! Long-term device key and the one-time pairing payload.

use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use qrcode::render::svg;
use qrcode::QrCode;
use rcgen::{CertificateParams, DnType, KeyPair, PKCS_ECDSA_P256_SHA256};

use crate::tls::{hex_decode, hex_encode, LanError};
use crate::PAIR_TTL_SECS;

/// P-256 key the device keeps. The private bytes are not printed.
#[derive(Clone)]
pub struct Identity {
    pub device_id: String,
    pub name: String,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    pub spki: [u8; 32],
    pub spki_hex: String,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("device_id", &self.device_id)
            .field("name", &self.name)
            .field("spki_hex", &self.spki_hex)
            .finish()
    }
}

impl Identity {
    pub fn cert_der(&self) -> Vec<u8> {
        self.cert_der.clone()
    }

    pub fn key_der(&self) -> Vec<u8> {
        self.key_der.clone()
    }

    pub fn cert_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(&self.cert_der)
    }

    pub fn key_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(&self.key_der)
    }
}

pub fn new_identity(device_id: &str, name: &str) -> Result<Identity, LanError> {
    let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(|_| LanError::new("tls"))?;
    let san = format!("{device_id}.holdover.local");
    let mut params = CertificateParams::new(vec![san]).map_err(|_| LanError::new("tls"))?;
    params
        .distinguished_name
        .push(DnType::CommonName, "Holdover");
    params.not_before = rcgen::date_time_ymd(2026, 1, 1);
    params.not_after = rcgen::date_time_ymd(2036, 1, 1);
    let cert = params.self_signed(&key).map_err(|_| LanError::new("tls"))?;
    let cert_der = cert.der().to_vec();
    let spki = crate::tls::spki_sha256(&cert_der)?;
    Ok(Identity {
        device_id: device_id.to_string(),
        name: name.to_string(),
        key_der: key.serialize_der(),
        cert_der,
        spki_hex: hex_encode(&spki),
        spki,
    })
}

pub fn identity_from_parts(
    device_id: &str,
    name: &str,
    cert_b64: &str,
    key_b64: &str,
) -> Result<Identity, LanError> {
    let cert_der = URL_SAFE_NO_PAD
        .decode(cert_b64.trim())
        .map_err(|_| LanError::new("tls"))?;
    let key_der = URL_SAFE_NO_PAD
        .decode(key_b64.trim())
        .map_err(|_| LanError::new("tls"))?;
    let spki = crate::tls::spki_sha256(&cert_der)?;
    Ok(Identity {
        device_id: device_id.to_string(),
        name: name.to_string(),
        cert_der,
        key_der,
        spki_hex: hex_encode(&spki),
        spki,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub schema: i64,
    pub expires: u64,
    pub port: u16,
    pub token: String,
    pub spki_hex: String,
    pub hosts: Vec<String>,
    pub name: String,
}

impl Offer {
    pub fn url(&self) -> String {
        let hosts = self.hosts.join(",");
        format!(
            "holdover://pair?v=1&s={}&e={}&p={}&t={}&k={}&h={}&n={}",
            self.schema,
            self.expires,
            self.port,
            self.token,
            self.spki_hex,
            hosts,
            url_encode(&self.name),
        )
    }
}

pub fn fresh_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom_fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn getrandom_fill(bytes: &mut [u8]) {
    // ring's SystemRandom is the same CSPRNG the TLS stack uses.
    use ring::rand::{SecureRandom, SystemRandom};
    let rng = SystemRandom::new();
    rng.fill(bytes)
        .expect("the system random source is required for pairing");
}

pub fn make_offer(identity: &Identity, port: u16, hosts: Vec<String>, now: u64) -> Offer {
    Offer {
        schema: holdover_core::LAN_SCHEMA,
        expires: now.saturating_add(PAIR_TTL_SECS),
        port,
        token: fresh_token(),
        spki_hex: identity.spki_hex.clone(),
        hosts: if hosts.is_empty() {
            vec!["127.0.0.1".into()]
        } else {
            hosts
        },
        name: identity.name.clone(),
    }
}

pub fn parse_offer(text: &str) -> Result<Offer, LanError> {
    let text = text.trim();
    let query = text
        .strip_prefix("holdover://pair?")
        .ok_or_else(|| LanError::new("pair-invalid"))?;
    let mut schema = None;
    let mut expires = None;
    let mut port = None;
    let mut token = None;
    let mut spki = None;
    let mut hosts = None;
    let mut name = String::new();
    for part in query.split('&') {
        let Some((key, value)) = part.split_once('=') else {
            return Err(LanError::new("pair-invalid"));
        };
        match key {
            "v" if value != "1" => return Err(LanError::new("pair-invalid")),
            "v" => {}
            "s" => schema = value.parse::<i64>().ok(),
            "e" => expires = value.parse::<u64>().ok(),
            "p" => port = value.parse::<u16>().ok(),
            "t" => token = Some(value.to_string()),
            "k" => spki = Some(value.to_string()),
            "h" => {
                hosts = Some(
                    value
                        .split(',')
                        .filter(|item| !item.is_empty())
                        .map(str::to_string)
                        .collect(),
                )
            }
            "n" => name = url_decode(value),
            _ => return Err(LanError::new("pair-invalid")),
        }
    }
    let schema = schema.ok_or_else(|| LanError::new("pair-invalid"))?;
    let expires = expires.ok_or_else(|| LanError::new("pair-invalid"))?;
    let port = port
        .filter(|port| *port != 0)
        .ok_or_else(|| LanError::new("pair-invalid"))?;
    let token = token
        .filter(|token| token.len() >= 32)
        .ok_or_else(|| LanError::new("pair-invalid"))?;
    let spki_hex = spki.ok_or_else(|| LanError::new("pair-invalid"))?;
    let raw = hex_decode(&spki_hex)?;
    if raw.len() != 32 {
        return Err(LanError::new("pair-invalid"));
    }
    let hosts = hosts
        .filter(|items: &Vec<String>| !items.is_empty())
        .ok_or_else(|| LanError::new("pair-invalid"))?;
    Ok(Offer {
        schema,
        expires,
        port,
        token,
        spki_hex,
        hosts,
        name,
    })
}

pub fn offer_qr_png(offer: &Offer) -> Result<Vec<u8>, LanError> {
    let code = QrCode::new(offer.url().as_bytes()).map_err(|_| LanError::new("pair-invalid"))?;
    let image = code
        .render::<image::Luma<u8>>()
        .quiet_zone(true)
        .min_dimensions(240, 240)
        .build();
    let mut bytes = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    image
        .write_with_encoder(encoder)
        .map_err(|_| LanError::new("pair-invalid"))?;
    Ok(bytes)
}

pub fn offer_qr(offer: &Offer) -> Result<String, LanError> {
    let code = QrCode::new(offer.url().as_bytes()).map_err(|_| LanError::new("pair-invalid"))?;
    let image = code
        .render::<svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .build();
    Ok(image)
}

pub fn decode_qr_image(bytes: &[u8]) -> Result<String, LanError> {
    let image = image::load_from_memory(bytes).map_err(|_| LanError::new("pair-invalid"))?;
    let gray = image.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(gray);
    let grids = prepared.detect_grids();
    let grid = grids.first().ok_or_else(|| LanError::new("pair-invalid"))?;
    let (_meta, text) = grid.decode().map_err(|_| LanError::new("pair-invalid"))?;
    Ok(text)
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn pin_from_hex(text: &str) -> Result<[u8; 32], LanError> {
    let raw = hex_decode(text)?;
    if raw.len() != 32 {
        return Err(LanError::new("pair-pin"));
    }
    let mut pin = [0u8; 32];
    pin.copy_from_slice(&raw);
    Ok(pin)
}

fn url_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (
                (bytes[index + 1] as char).to_digit(16),
                (bytes[index + 2] as char).to_digit(16),
            ) {
                out.push(((hi << 4) | lo) as u8);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
