//! Encrypted storage for the Supercy history.
//!
//! One SQLite database in WAL mode. Content, previews and source apps are sealed per item with
//! XChaCha20-Poly1305; the key is derived from a random data key kept in the system keystore.
//! Only counts, kinds, times, sizes and the pinned flag are stored in the clear (design 02).

mod codec;
mod crypto;
mod error;
mod keys;
mod rows;
mod schema;
mod store;

pub use error::StoreError;
pub use keys::DATA_KEY_ACCOUNT;
pub use schema::SCHEMA_VERSION;
pub use store::{
    DATABASE_FILE, DEFAULT_MAX_IMAGE_TOTAL_BYTES, DEFAULT_MAX_ITEMS, Limits, LoadReport, MAX_ITEMS,
    MIN_ITEMS, Store,
};
