//! Mapping between items and table rows.

use cy_model::{DeviceTag, Fingerprint, Hlc, Item, ItemId, Sensitivity};
use rusqlite::{Row, Transaction, params};

use crate::StoreError;
use crate::codec::{BodyRef, decode, encode};
use crate::crypto::{open, seal};
use crate::keys::Keys;

/// Version of the sealed body's layout; part of the associated data.
const BODY_VERSION: u32 = 1;

/// Columns read by [`read_item`], in order.
pub(crate) const ITEM_COLUMNS: &str = "id, fingerprint, first_copied, last_used, pinned, pinned_at, expires_at, origin, body, copy_count, pin_key";

/// Associated data: the item id and the body version, so a body cannot move to another row.
fn aad(id: ItemId) -> Vec<u8> {
    [id.as_bytes().as_slice(), &BODY_VERSION.to_be_bytes()].concat()
}

pub(crate) fn to_i64(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::OutOfRange)
}

fn to_u64(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::Corrupt)
}

/// Inserts `item`, or replaces the row with the same id.
pub(crate) fn upsert(tx: &Transaction<'_>, keys: &Keys, item: &Item) -> Result<(), StoreError> {
    let (expires_at, sensitive) = match item.sensitivity {
        Sensitivity::Normal => (None, None),
        Sensitivity::Sensitive {
            expires_at_ms,
            reason,
        } => (Some(to_i64(expires_at_ms)?), Some(reason)),
    };
    let body = encode(&BodyRef {
        reps: &item.reps,
        source: item.source.as_ref(),
        truncated: item.truncated,
        sensitive,
    })?;
    let sealed = seal(&keys.content, &aad(item.id), &body)?;
    tx.execute(
        "INSERT INTO items (id, fingerprint, kind, first_copied, last_used, pinned, pinned_at, expires_at, origin, size,
             body, copy_count, pin_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(id) DO UPDATE SET fingerprint = ?2, kind = ?3, first_copied = ?4, last_used = ?5, pinned = ?6,
             pinned_at = ?7, expires_at = ?8, origin = ?9, size = ?10, body = ?11, copy_count = ?12, pin_key = ?13",
        params![
            item.id.as_bytes().as_slice(),
            item.fingerprint.0.as_slice(),
            item.kind().code(),
            to_i64(item.first_copied_ms)?,
            item.last_used.to_bytes().as_slice(),
            item.pinned,
            item.pinned_at.map(|at| at.to_bytes().to_vec()),
            expires_at,
            item.origin.0.as_slice(),
            to_i64(item.size() as u64)?,
            sealed,
            item.copy_count,
            item.pin_key.map(String::from),
        ],
    )
    .map_err(StoreError::from_sql)?;
    Ok(())
}

/// Reads one row selected with [`ITEM_COLUMNS`]. The outer error is a database failure; the inner
/// one marks a row that cannot be read (damaged or sealed with another key) by its id.
pub(crate) fn read_item(row: &Row<'_>, keys: &Keys) -> Result<Result<Item, ItemId>, StoreError> {
    let id_bytes: Vec<u8> = row.get(0).map_err(StoreError::from_sql)?;
    let id = ItemId::from_slice(&id_bytes).map_err(|_| StoreError::Corrupt)?;
    Ok(decode_row(row, keys, id).map_err(|_| id))
}

fn decode_row(row: &Row<'_>, keys: &Keys, id: ItemId) -> Result<Item, StoreError> {
    let get_blob = |at: usize| row.get::<_, Vec<u8>>(at).map_err(StoreError::from_sql);
    let fingerprint =
        <[u8; 32]>::try_from(get_blob(1)?.as_slice()).map_err(|_| StoreError::Corrupt)?;
    let last_used = Hlc::from_bytes(&get_blob(3)?).ok_or(StoreError::Corrupt)?;
    let pinned_at: Option<Vec<u8>> = row.get(5).map_err(StoreError::from_sql)?;
    let pinned_at = pinned_at
        .map(|b| Hlc::from_bytes(&b).ok_or(StoreError::Corrupt))
        .transpose()?;
    let expires_at: Option<i64> = row.get(6).map_err(StoreError::from_sql)?;
    let origin = <[u8; 8]>::try_from(get_blob(7)?.as_slice()).map_err(|_| StoreError::Corrupt)?;
    let plaintext = open(&keys.content, &aad(id), &get_blob(8)?)?;
    let body = decode(&plaintext)?;
    let sensitivity = match (expires_at, body.sensitive) {
        (Some(at), Some(reason)) => Sensitivity::Sensitive {
            expires_at_ms: to_u64(at)?,
            reason,
        },
        (None, None) => Sensitivity::Normal,
        _ => return Err(StoreError::Corrupt),
    };
    Ok(Item {
        id,
        fingerprint: Fingerprint(fingerprint),
        reps: body.reps,
        source: body.source,
        origin: DeviceTag(origin),
        first_copied_ms: to_u64(row.get(2).map_err(StoreError::from_sql)?)?,
        last_used,
        pinned: row.get(4).map_err(StoreError::from_sql)?,
        pinned_at,
        sensitivity,
        truncated: body.truncated,
        copy_count: row.get(9).map_err(StoreError::from_sql)?,
        pin_key: read_pin_key(row.get(10).map_err(StoreError::from_sql)?)?,
    })
}

/// A stored letter key: one character `a`–`z`.
fn read_pin_key(stored: Option<String>) -> Result<Option<char>, StoreError> {
    let Some(text) = stored else { return Ok(None) };
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(key), None) if key.is_ascii_lowercase() => Ok(Some(key)),
        _ => Err(StoreError::Corrupt),
    }
}
