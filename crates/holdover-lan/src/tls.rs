//! TLS 1.3 with a pinned key. There is no certificate authority.

use std::io::{Read, Write};
use std::sync::{Arc, Once};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, Error, ServerConfig, ServerConnection,
    SignatureScheme, StreamOwned,
};
use sha2::{Digest, Sha256};
use x509_parser::certificate::X509Certificate;
use x509_parser::prelude::FromDer;

use crate::identity::Identity;

const MAX_FRAME: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanError {
    pub code: String,
    /// Addresses and ports that were tried, for the on-screen error. Empty
    /// when the failure has no dial list.
    pub detail: String,
}

impl LanError {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            detail: String::new(),
        }
    }

    pub fn with_detail(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for LanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code)
    }
}

impl std::error::Error for LanError {}

pub fn install_crypto() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

pub fn spki_sha256(cert_der: &[u8]) -> Result<[u8; 32], LanError> {
    let (_, cert) = X509Certificate::from_der(cert_der).map_err(|_| LanError::new("tls"))?;
    let digest = Sha256::digest(cert.tbs_certificate.subject_pki.raw);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    Ok(out)
}

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub fn hex_decode(text: &str) -> Result<Vec<u8>, LanError> {
    if text.len() % 2 != 0 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(LanError::new("pair-invalid"));
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let hi = (bytes[index] as char).to_digit(16).unwrap();
        let lo = (bytes[index + 1] as char).to_digit(16).unwrap();
        out.push(((hi << 4) | lo) as u8);
        index += 2;
    }
    Ok(out)
}

#[derive(Debug)]
struct PinServer {
    pin: [u8; 32],
}

impl ServerCertVerifier for PinServer {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        match spki_sha256(end_entity.as_ref()) {
            Ok(spki) if spki == self.pin => Ok(ServerCertVerified::assertion()),
            _ => Err(Error::General("pair-pin".into())),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[derive(Debug)]
struct AnyClient;

impl ClientCertVerifier for AnyClient {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        // The pairing code, or the pinned key checked after the handshake,
        // is what admits the peer. A certificate that does not parse is not
        // a key we can pin.
        spki_sha256(end_entity.as_ref()).map_err(|_| Error::General("tls".into()))?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    install_crypto();
    Arc::new(rustls::crypto::ring::default_provider())
}

fn cert_and_key(
    identity: &Identity,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), LanError> {
    let cert = CertificateDer::from(identity.cert_der());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(identity.key_der()));
    Ok((vec![cert], key))
}

pub fn server_config(identity: &Identity) -> Result<Arc<ServerConfig>, LanError> {
    let (certs, key) = cert_and_key(identity)?;
    let config = ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| LanError::new("tls"))?
        .with_client_cert_verifier(Arc::new(AnyClient))
        .with_single_cert(certs, key)
        .map_err(|_| LanError::new("tls"))?;
    Ok(Arc::new(config))
}

pub fn client_config(
    identity: &Identity,
    server_pin: [u8; 32],
) -> Result<Arc<ClientConfig>, LanError> {
    let (certs, key) = cert_and_key(identity)?;
    let config = ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| LanError::new("tls"))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinServer { pin: server_pin }))
        .with_client_auth_cert(certs, key)
        .map_err(|_| LanError::new("tls"))?;
    Ok(Arc::new(config))
}

pub type ServerStream = StreamOwned<ServerConnection, std::net::TcpStream>;
pub type ClientStream = StreamOwned<ClientConnection, std::net::TcpStream>;

pub fn accept_tls(
    tcp: std::net::TcpStream,
    config: Arc<ServerConfig>,
) -> Result<ServerStream, LanError> {
    let conn = ServerConnection::new(config).map_err(|_| LanError::new("tls"))?;
    let mut stream = StreamOwned::new(conn, tcp);
    // Complete the handshake now so a plaintext client fails before any JSON.
    let _ = stream.flush();
    stream
        .conn
        .complete_io(&mut stream.sock)
        .map_err(|_| LanError::new("tls"))?;
    if stream.conn.peer_certificates().is_none() {
        return Err(LanError::new("tls"));
    }
    Ok(stream)
}

pub fn connect_tls(
    tcp: std::net::TcpStream,
    identity: &Identity,
    server_pin: [u8; 32],
) -> Result<ClientStream, LanError> {
    let name = ServerName::try_from("holdover.local").map_err(|_| LanError::new("tls"))?;
    let conn = ClientConnection::new(client_config(identity, server_pin)?, name)
        .map_err(|_| LanError::new("tls"))?;
    let mut stream = StreamOwned::new(conn, tcp);
    stream
        .conn
        .complete_io(&mut stream.sock)
        .map_err(|_| LanError::new("pair-pin"))?;
    Ok(stream)
}

pub fn peer_spki_of(certs: Option<&[CertificateDer<'_>]>) -> Result<[u8; 32], LanError> {
    let cert = certs
        .and_then(|items| items.first())
        .ok_or_else(|| LanError::new("tls"))?;
    spki_sha256(cert.as_ref())
}

pub fn write_json(stream: &mut impl Write, value: &serde_json::Value) -> Result<(), LanError> {
    let bytes = serde_json::to_vec(value).map_err(|_| LanError::new("corrupt"))?;
    if bytes.len() > MAX_FRAME {
        return Err(LanError::new("corrupt"));
    }
    let len = (bytes.len() as u32).to_be_bytes();
    stream
        .write_all(&len)
        .map_err(|_| LanError::new("disconnected"))?;
    stream
        .write_all(&bytes)
        .map_err(|_| LanError::new("disconnected"))?;
    stream.flush().map_err(|_| LanError::new("disconnected"))?;
    Ok(())
}

pub fn read_json(stream: &mut impl Read) -> Result<serde_json::Value, LanError> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|_| LanError::new("disconnected"))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err(LanError::new("corrupt"));
    }
    let mut buf = vec![0u8; len];
    stream
        .read_exact(&mut buf)
        .map_err(|_| LanError::new("disconnected"))?;
    serde_json::from_slice(&buf).map_err(|_| LanError::new("corrupt"))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex_encode(&Sha256::digest(bytes))
}

pub fn token_hash(token: &str) -> [u8; 32] {
    let digest = Sha256::digest(token.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub fn ct_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}
