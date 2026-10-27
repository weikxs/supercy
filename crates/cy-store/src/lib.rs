//! Encrypted storage for the Supercy history.
//!
//! One SQLite database in WAL mode. Content, previews and source apps are sealed per item with
//! XChaCha20-Poly1305; the key is derived from a random data key kept in the system keystore.
//! Only counts, kinds, times, sizes and the pinned flag are stored in the clear (design 02).

mod codec;
mod crypto;
mod error;
mod keys;
mod meta;
mod rows;
mod schema;
mod store;

pub use error::StoreError;

/// Entry points for the fuzz targets; not part of the public interface.
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing {
    /// Decodes a stored body; returns whether it was valid.
    #[must_use]
    pub fn decode_body(bytes: &[u8]) -> bool {
        crate::codec::decode(bytes).is_ok()
    }
}
pub use keys::DATA_KEY_ACCOUNT;
pub use schema::SCHEMA_VERSION;
pub use store::{
    DATABASE_FILE, DEFAULT_MAX_IMAGE_TOTAL_BYTES, DEFAULT_MAX_ITEMS, Limits, LoadReport, MAX_ITEMS,
    MIN_ITEMS, Store,
};
