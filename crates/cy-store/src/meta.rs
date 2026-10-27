//! Sealed key-value records: settings and other small values that are not history items.

use cy_model::SecretBytes;
use rusqlite::{Connection, OptionalExtension, params};

use crate::StoreError;
use crate::crypto::{open, seal};
use crate::keys::Keys;

/// Associated data: a fixed prefix and the key, so a record cannot move to another key or be
/// mistaken for an item body.
fn aad(key: &str) -> Vec<u8> {
    [b"supercy-meta\0".as_slice(), key.as_bytes()].concat()
}

pub(crate) fn read(
    conn: &Connection,
    keys: &Keys,
    key: &str,
) -> Result<Option<SecretBytes>, StoreError> {
    let sealed: Option<Vec<u8>> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::from_sql)?;
    sealed
        .map(|sealed| {
            open(&keys.content, &aad(key), &sealed).map(|plain| SecretBytes::new(plain.to_vec()))
        })
        .transpose()
}

pub(crate) fn write(
    conn: &Connection,
    keys: &Keys,
    key: &str,
    value: &[u8],
) -> Result<(), StoreError> {
    let sealed = seal(&keys.content, &aad(key), value)?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![key, sealed],
    )
    .map_err(StoreError::from_sql)?;
    Ok(())
}
