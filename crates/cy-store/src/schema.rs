//! Schema and migrations. The version lives in SQLite's `user_version`.

use rusqlite::Connection;

use crate::StoreError;

/// The newest schema this build reads and writes.
pub const SCHEMA_VERSION: i64 = 3;

/// Migration `n` (0-based) moves the schema from version `n` to `n + 1`.
const MIGRATIONS: &[&str] = &[V1, V2, V3];

const V1: &str = "
CREATE TABLE items (
    id           BLOB PRIMARY KEY NOT NULL,
    fingerprint  BLOB NOT NULL,
    kind         INTEGER NOT NULL,
    first_copied INTEGER NOT NULL,
    last_used    BLOB NOT NULL,
    pinned       INTEGER NOT NULL,
    pinned_at    BLOB,
    expires_at   INTEGER,
    origin       BLOB NOT NULL,
    size         INTEGER NOT NULL,
    body         BLOB NOT NULL
) STRICT;
CREATE UNIQUE INDEX items_fingerprint ON items(fingerprint);
CREATE INDEX items_order ON items(pinned, last_used);
CREATE INDEX items_expiry ON items(expires_at) WHERE expires_at IS NOT NULL;
";

/// Version 2: sealed key-value records (settings, device tag), and an expiry index that matches
/// the expiry queries exactly, so finding due items never scans the history.
const V2: &str = "
CREATE TABLE meta (
    key   TEXT PRIMARY KEY NOT NULL,
    value BLOB NOT NULL
) STRICT;
DROP INDEX items_expiry;
CREATE INDEX items_expiry ON items(expires_at) WHERE expires_at IS NOT NULL AND pinned = 0;
";

/// Version 3: how often each item was copied (for sorting) and the letter keys of pinned items.
const V3: &str = "
ALTER TABLE items ADD COLUMN copy_count INTEGER NOT NULL DEFAULT 1;
ALTER TABLE items ADD COLUMN pin_key TEXT;
CREATE UNIQUE INDEX items_pin_key ON items(pin_key) WHERE pin_key IS NOT NULL;
";

/// The schema version stored in the database.
pub(crate) fn version(conn: &Connection) -> Result<i64, StoreError> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(StoreError::from_sql)
}

/// Brings the schema to [`SCHEMA_VERSION`], one transaction per step. A newer database is left
/// untouched.
pub(crate) fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let current = version(conn)?;
    if current > SCHEMA_VERSION {
        return Err(StoreError::TooNew {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    let done = usize::try_from(current).map_err(|_| StoreError::Corrupt)?;
    for (step, sql) in MIGRATIONS.iter().enumerate().skip(done) {
        let tx = conn.transaction().map_err(StoreError::from_sql)?;
        tx.execute_batch(sql).map_err(StoreError::from_sql)?;
        tx.pragma_update(None, "user_version", step + 1)
            .map_err(StoreError::from_sql)?;
        tx.commit().map_err(StoreError::from_sql)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_version_has_a_migration() {
        assert_eq!(i64::try_from(MIGRATIONS.len()).unwrap(), SCHEMA_VERSION);
    }
}
