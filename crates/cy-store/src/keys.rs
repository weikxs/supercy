//! The data key and the keys derived from it.

use cy_model::{FINGERPRINT_LEN, FingerprintKey, SecretBytes};
use cy_platform::KeyStore;
use zeroize::Zeroizing;

use crate::StoreError;

/// Keystore account holding the data key.
pub const DATA_KEY_ACCOUNT: &str = "data-key";

/// Length of the data key and of derived keys.
pub(crate) const KEY_LEN: usize = 32;

/// BLAKE3 key-derivation contexts; changing one makes existing data unreadable.
const CONTENT_CONTEXT: &str = "Supercy 2026-10 history content key v1";
const FINGERPRINT_CONTEXT: &str = "Supercy 2026-10 content fingerprint key v1";

/// Keys derived from the data key, held only in memory.
pub(crate) struct Keys {
    pub(crate) content: Zeroizing<[u8; KEY_LEN]>,
    pub(crate) fingerprint: FingerprintKey,
}

impl Keys {
    pub(crate) fn derive(data_key: &[u8; KEY_LEN]) -> Self {
        Self {
            content: Zeroizing::new(blake3::derive_key(CONTENT_CONTEXT, data_key)),
            fingerprint: FingerprintKey::from_bytes(blake3::derive_key(
                FINGERPRINT_CONTEXT,
                data_key,
            )),
        }
    }
}

const _: () = assert!(FINGERPRINT_LEN == KEY_LEN);

/// Reads the data key, or creates one when there is no history yet.
pub(crate) fn load_or_create(
    keystore: &dyn KeyStore,
    history_exists: bool,
) -> Result<Keys, StoreError> {
    let data_key = match keystore.read(DATA_KEY_ACCOUNT)? {
        Some(stored) => Zeroizing::new(
            <[u8; KEY_LEN]>::try_from(stored.expose()).map_err(|_| StoreError::KeyInvalid)?,
        ),
        None if history_exists => return Err(StoreError::KeyMissing),
        None => {
            let mut fresh = Zeroizing::new([0u8; KEY_LEN]);
            getrandom::fill(fresh.as_mut()).map_err(|_| StoreError::Random)?;
            keystore.write(DATA_KEY_ACCOUNT, &SecretBytes::new(fresh.to_vec()))?;
            fresh
        }
    };
    Ok(Keys::derive(&data_key))
}
