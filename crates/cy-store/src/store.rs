//! The history store.

use std::path::Path;

use cy_model::{Fingerprint, FingerprintKey, Item, ItemId, SecretBytes};
use cy_platform::KeyStore;
use rusqlite::{Connection, OptionalExtension, params};

use crate::keys::{Keys, load_or_create};
use crate::rows::{ITEM_COLUMNS, read_item, to_i64, upsert};
use crate::{StoreError, schema};

/// File name of the database inside the data directory.
pub const DATABASE_FILE: &str = "history.db";
/// Default number of unpinned items kept.
pub const DEFAULT_MAX_ITEMS: usize = 500;
/// Smallest allowed `max_items`.
pub const MIN_ITEMS: usize = 50;
/// Largest allowed `max_items`.
pub const MAX_ITEMS: usize = 10_000;
/// Default total size of unpinned images kept: 64 MiB.
pub const DEFAULT_MAX_IMAGE_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

/// Permissions of the data directory: owner only.
#[cfg(unix)]
const DIR_MODE: u32 = 0o700;
/// Permissions of the database files: owner only.
#[cfg(unix)]
const FILE_MODE: u32 = 0o600;

/// How much history to keep. Pinned items do not count and are never removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Most unpinned items kept, clamped to [`MIN_ITEMS`]..=[`MAX_ITEMS`].
    pub max_items: usize,
    /// Largest total size of unpinned images.
    pub max_image_total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_items: DEFAULT_MAX_ITEMS,
            max_image_total_bytes: DEFAULT_MAX_IMAGE_TOTAL_BYTES,
        }
    }
}

/// The history as loaded: readable items in list order, and the ids of rows that could not be
/// read. Unreadable rows stay in the database untouched.
#[derive(Debug, Default)]
pub struct LoadReport {
    /// Readable items, pinned first, then most recently used.
    pub items: Vec<Item>,
    /// Rows that could not be decrypted or decoded.
    pub unreadable: Vec<ItemId>,
}

/// The open history database. Only one process can hold it open.
pub struct Store {
    conn: Connection,
    keys: Keys,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Store(..)")
    }
}

impl Store {
    /// Opens the history in `dir`, creating the directory, the database and the data key when
    /// there is no history yet.
    ///
    /// # Errors
    ///
    /// [`StoreError::KeyMissing`] when the database exists but its key does not (the database is
    /// left untouched), [`StoreError::Locked`] when another process has it open,
    /// [`StoreError::TooNew`] for a newer format, and keystore, file system or database failures.
    pub fn open(dir: &Path, keystore: &dyn KeyStore) -> Result<Self, StoreError> {
        create_private_dir(dir)?;
        let path = dir.join(DATABASE_FILE);
        let keys = load_or_create(keystore, path.exists())?;
        let mut conn = Connection::open(&path).map_err(StoreError::from_sql)?;
        configure(&conn)?;
        schema::migrate(&mut conn)?;
        restrict_files(&path)?;
        Ok(Self { conn, keys })
    }

    /// The key for content fingerprints of this history.
    #[must_use]
    pub fn fingerprint_key(&self) -> &FingerprintKey {
        &self.keys.fingerprint
    }

    /// Loads every item.
    ///
    /// # Errors
    ///
    /// Database failures. Rows that cannot be read are reported in [`LoadReport::unreadable`].
    pub fn load(&self) -> Result<LoadReport, StoreError> {
        let mut report = LoadReport::default();
        self.for_each(|read| match read {
            Ok(item) => report.items.push(item),
            Err(id) => report.unreadable.push(id),
        })?;
        Ok(report)
    }

    /// Calls `visit` for every item in list order, one at a time, so a large history (with
    /// images) is never held in memory at once. Unreadable rows arrive as `Err(id)`.
    ///
    /// # Errors
    ///
    /// Database failures.
    pub fn for_each(&self, mut visit: impl FnMut(Result<Item, ItemId>)) -> Result<(), StoreError> {
        let sql = format!("SELECT {ITEM_COLUMNS} FROM items ORDER BY pinned DESC, last_used DESC");
        let mut statement = self.conn.prepare(&sql).map_err(StoreError::from_sql)?;
        let mut rows = statement.query([]).map_err(StoreError::from_sql)?;
        while let Some(row) = rows.next().map_err(StoreError::from_sql)? {
            visit(read_item(row, &self.keys)?);
        }
        Ok(())
    }

    /// The item `id`, if it exists and can be read.
    ///
    /// # Errors
    ///
    /// Database failures, or [`StoreError::Unseal`] when the row cannot be read.
    pub fn get(&self, id: ItemId) -> Result<Option<Item>, StoreError> {
        let sql = format!("SELECT {ITEM_COLUMNS} FROM items WHERE id = ?1");
        let row = self
            .conn
            .query_row(&sql, params![id.as_bytes().as_slice()], |row| {
                Ok(read_item(row, &self.keys))
            })
            .optional()
            .map_err(StoreError::from_sql)?;
        row.transpose()?
            .map(|read| read.map_err(|_| StoreError::Unseal))
            .transpose()
    }

    /// The id of the item with `fingerprint`, if any.
    ///
    /// # Errors
    ///
    /// Database failures.
    pub fn find_by_fingerprint(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<Option<ItemId>, StoreError> {
        let found: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT id FROM items WHERE fingerprint = ?1",
                params![fingerprint.0.as_slice()],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::from_sql)?;
        found
            .map(|bytes| ItemId::from_slice(&bytes).map_err(|_| StoreError::Corrupt))
            .transpose()
    }

    /// Saves `item` (insert or replace) and applies `limits`, in one transaction. Returns the ids
    /// removed by the limits.
    ///
    /// # Errors
    ///
    /// Encryption or database failures; nothing is changed then.
    pub fn save(&mut self, item: &Item, limits: &Limits) -> Result<Vec<ItemId>, StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::from_sql)?;
        upsert(&tx, &self.keys, item)?;
        let removed = enforce_limits(&tx, limits)?;
        tx.commit().map_err(StoreError::from_sql)?;
        Ok(removed)
    }

    /// Deletes `id`; returns whether it existed.
    ///
    /// # Errors
    ///
    /// Database failures.
    pub fn delete(&mut self, id: ItemId) -> Result<bool, StoreError> {
        let changed = self
            .conn
            .execute(
                "DELETE FROM items WHERE id = ?1",
                params![id.as_bytes().as_slice()],
            )
            .map_err(StoreError::from_sql)?;
        Ok(changed > 0)
    }

    /// Deletes all items (or all unpinned ones) and compacts the file so deleted data does not
    /// linger in free pages or the write-ahead log. Returns the number deleted.
    ///
    /// # Errors
    ///
    /// Database failures.
    pub fn clear(&mut self, keep_pinned: bool) -> Result<usize, StoreError> {
        let sql = if keep_pinned {
            "DELETE FROM items WHERE pinned = 0"
        } else {
            "DELETE FROM items"
        };
        let deleted = self.conn.execute(sql, []).map_err(StoreError::from_sql)?;
        self.conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .map_err(StoreError::from_sql)?;
        self.conn
            .execute_batch("VACUUM")
            .map_err(StoreError::from_sql)?;
        Ok(deleted)
    }

    /// Deletes unpinned sensitive items that expired at `now_ms`; returns their ids.
    ///
    /// # Errors
    ///
    /// Database failures.
    pub fn expire(&mut self, now_ms: u64) -> Result<Vec<ItemId>, StoreError> {
        let ids = select_ids(
            &self.conn,
            "SELECT id FROM items WHERE expires_at IS NOT NULL AND expires_at <= ?1 AND pinned = 0",
            params![to_i64(now_ms)?],
        )?;
        // Nothing due is the common case: no write transaction then.
        if ids.is_empty() {
            return Ok(ids);
        }
        let tx = self.conn.transaction().map_err(StoreError::from_sql)?;
        delete_ids(&tx, &ids)?;
        tx.commit().map_err(StoreError::from_sql)?;
        Ok(ids)
    }

    /// Applies `limits` now (for example after the user lowered them); returns the removed ids.
    ///
    /// # Errors
    ///
    /// Database failures; nothing is changed then.
    pub fn apply_limits(&mut self, limits: &Limits) -> Result<Vec<ItemId>, StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::from_sql)?;
        let removed = enforce_limits(&tx, limits)?;
        tx.commit().map_err(StoreError::from_sql)?;
        Ok(removed)
    }

    /// The sealed record stored under `key`, if any.
    ///
    /// # Errors
    ///
    /// Database failures, or [`StoreError::Unseal`] when the record cannot be decrypted.
    pub fn read_meta(&self, key: &str) -> Result<Option<SecretBytes>, StoreError> {
        crate::meta::read(&self.conn, &self.keys, key)
    }

    /// Seals `value` and stores it under `key`, replacing any previous record.
    ///
    /// # Errors
    ///
    /// Encryption or database failures.
    pub fn write_meta(&mut self, key: &str, value: &[u8]) -> Result<(), StoreError> {
        crate::meta::write(&self.conn, &self.keys, key, value)
    }

    /// When the next unpinned sensitive item expires, if any. The caller schedules one timer for
    /// it; the store itself never runs timers.
    ///
    /// # Errors
    ///
    /// Database failures.
    pub fn next_expiry(&self) -> Result<Option<u64>, StoreError> {
        let next: Option<i64> = self
            .conn
            .query_row(
                "SELECT MIN(expires_at) FROM items WHERE expires_at IS NOT NULL AND pinned = 0",
                [],
                |row| row.get(0),
            )
            .map_err(StoreError::from_sql)?;
        next.map(|at| u64::try_from(at).map_err(|_| StoreError::Corrupt))
            .transpose()
    }
}

/// Removes the oldest unpinned items beyond `max_items`, then the oldest unpinned images while
/// their total is above `max_image_total_bytes`.
pub(crate) fn enforce_limits(
    tx: &rusqlite::Transaction<'_>,
    limits: &Limits,
) -> Result<Vec<ItemId>, StoreError> {
    let keep = to_i64(limits.max_items.clamp(MIN_ITEMS, MAX_ITEMS) as u64)?;
    let mut removed = select_ids(
        tx,
        "SELECT id FROM items WHERE pinned = 0 ORDER BY last_used DESC LIMIT -1 OFFSET ?1",
        params![keep],
    )?;
    delete_ids(tx, &removed)?;
    let image_code = cy_model::RepKind::Image.code();
    let mut statement = tx
        .prepare(
            "SELECT id, size FROM items WHERE pinned = 0 AND kind = ?1 ORDER BY last_used DESC",
        )
        .map_err(StoreError::from_sql)?;
    let images: Vec<(Vec<u8>, i64)> = statement
        .query_map(params![image_code], |row| Ok((row.get(0)?, row.get(1)?)))
        .and_then(|rows| rows.collect())
        .map_err(StoreError::from_sql)?;
    let mut total: u64 = 0;
    let mut over = Vec::new();
    for (id, size) in images {
        total = total.saturating_add(u64::try_from(size).map_err(|_| StoreError::Corrupt)?);
        if total > limits.max_image_total_bytes {
            over.push(ItemId::from_slice(&id).map_err(|_| StoreError::Corrupt)?);
        }
    }
    delete_ids(tx, &over)?;
    removed.extend(over);
    Ok(removed)
}

fn select_ids(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<ItemId>, StoreError> {
    let mut statement = conn.prepare(sql).map_err(StoreError::from_sql)?;
    let raw: Vec<Vec<u8>> = statement
        .query_map(params, |row| row.get(0))
        .and_then(|rows| rows.collect())
        .map_err(StoreError::from_sql)?;
    raw.iter()
        .map(|bytes| ItemId::from_slice(bytes).map_err(|_| StoreError::Corrupt))
        .collect()
}

fn delete_ids(conn: &Connection, ids: &[ItemId]) -> Result<(), StoreError> {
    let mut statement = conn
        .prepare("DELETE FROM items WHERE id = ?1")
        .map_err(StoreError::from_sql)?;
    for id in ids {
        statement
            .execute(params![id.as_bytes().as_slice()])
            .map_err(StoreError::from_sql)?;
    }
    Ok(())
}

/// Exclusive locking (one process at a time, no shared-memory file), WAL, overwrite deleted
/// content, a small page cache (512 KiB; the history is read once at start), and take the lock
/// now so a second process fails at once.
fn configure(conn: &Connection) -> Result<(), StoreError> {
    conn.busy_timeout(std::time::Duration::ZERO)
        .map_err(StoreError::from_sql)?;
    let _: String = conn
        .query_row("PRAGMA locking_mode = EXCLUSIVE", [], |row| row.get(0))
        .map_err(StoreError::from_sql)?;
    let _: String = conn
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(StoreError::from_sql)?;
    conn.execute_batch(
        "PRAGMA secure_delete = ON; PRAGMA synchronous = NORMAL; PRAGMA cache_size = -512;
         BEGIN IMMEDIATE; COMMIT;",
    )
    .map_err(StoreError::from_sql)
}

fn create_private_dir(dir: &Path) -> Result<(), StoreError> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(DIR_MODE))?;
    }
    Ok(())
}

fn restrict_files(path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let wal = std::path::PathBuf::from(format!("{}-wal", path.display()));
        for file in [path, wal.as_path()] {
            if file.exists() {
                std::fs::set_permissions(file, std::fs::Permissions::from_mode(FILE_MODE))?;
            }
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
