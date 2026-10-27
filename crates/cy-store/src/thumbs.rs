//! Small image thumbnails, sealed in their own column so the history list can show them without
//! decrypting (or holding) the full image.

use cy_model::{ItemId, SecretBytes};
use rusqlite::{Connection, OptionalExtension, params};

use crate::StoreError;
use crate::crypto::{open, seal};
use crate::keys::Keys;

/// Largest thumbnail kept, in bytes.
pub const MAX_THUMBNAIL_BYTES: usize = 256 * 1024;

/// Associated data: the item id and a fixed tag, so a thumbnail cannot move to another item or be
/// mistaken for a body.
fn aad(id: ItemId) -> Vec<u8> {
    [id.as_bytes().as_slice(), b"\0supercy-thumbnail-v1"].concat()
}

pub(crate) fn write(
    conn: &Connection,
    keys: &Keys,
    id: ItemId,
    png: &[u8],
) -> Result<bool, StoreError> {
    if png.is_empty() || png.len() > MAX_THUMBNAIL_BYTES {
        return Err(StoreError::OutOfRange);
    }
    let sealed = seal(&keys.content, &aad(id), png)?;
    let changed = conn
        .execute(
            "UPDATE items SET thumb = ?1 WHERE id = ?2",
            params![sealed, id.as_bytes().as_slice()],
        )
        .map_err(StoreError::from_sql)?;
    Ok(changed > 0)
}

pub(crate) fn read(
    conn: &Connection,
    keys: &Keys,
    id: ItemId,
) -> Result<Option<SecretBytes>, StoreError> {
    let sealed: Option<Option<Vec<u8>>> = conn
        .query_row(
            "SELECT thumb FROM items WHERE id = ?1",
            params![id.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::from_sql)?;
    sealed
        .flatten()
        .map(|sealed| {
            open(&keys.content, &aad(id), &sealed).map(|plain| SecretBytes::new(plain.to_vec()))
        })
        .transpose()
}
