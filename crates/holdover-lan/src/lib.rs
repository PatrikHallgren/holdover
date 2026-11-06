//! Local pairing and sync.
//!
//! No account, relay, or cloud. A one-time code in a QR payload authenticates
//! the first connection. After that, both sides require the peer's pinned
//! key inside TLS 1.3. Nothing is written until both sides have the same
//! preview and the initiating person has resolved every conflict.

mod discover;
mod identity;
mod session;
mod tls;

pub use discover::{advertise, browse, local_hosts, Advertisement, Found, SERVICE_TYPE};
pub use identity::{
    decode_qr_image, identity_from_parts, new_identity, offer_qr, offer_qr_png, parse_offer,
    unix_now, Identity, Offer,
};
pub use session::{
    join_pair, join_pair_with, open_sync, preview_value, Catalog, ClientSync, Library, PairWait,
    Peer, SyncListener, PREVIEW_WAIT,
};
pub use tls::LanError;

/// How long a pairing code stays valid.
pub const PAIR_TTL_SECS: u64 = 600;
