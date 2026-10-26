//! Storage errors. Messages never contain clipboard content.

use cy_platform::KeyStoreError;

/// What went wrong in storage.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database exists but its key is not in the keystore, so the history cannot be read.
    /// The database is left untouched.
    #[error(
        "the history cannot be decrypted because its key is missing from the system keystore; clear the history to start again"
    )]
    KeyMissing,
    /// The key in the keystore has the wrong length.
    #[error("the history key in the system keystore is damaged; clear the history to start again")]
    KeyInvalid,
    /// The keystore failed.
    #[error(transparent)]
    KeyStore(#[from] KeyStoreError),
    /// Another Supercy process has the history open.
    #[error("the history is open in another Supercy process; quit it and try again")]
    Locked,
    /// The database was written by a newer Supercy.
    #[error(
        "the history was written by a newer Supercy (format {found}, this version reads up to {supported}); update Supercy"
    )]
    TooNew {
        /// Version found in the file.
        found: i64,
        /// Newest version this build understands.
        supported: i64,
    },
    /// The system random number generator failed.
    #[error("the system random number generator failed")]
    Random,
    /// A value does not fit the storage format.
    #[error("a value is out of range for storage")]
    OutOfRange,
    /// Encryption failed.
    #[error("encrypting an item failed")]
    Seal,
    /// A sealed item could not be opened (wrong key or tampered data).
    #[error("an item could not be decrypted")]
    Unseal,
    /// A stored record could not be decoded.
    #[error("a stored item is damaged")]
    Corrupt,
    /// The data directory could not be created or secured.
    #[error("the data directory is not usable: {0}")]
    Io(#[from] std::io::Error),
    /// SQLite reported an error.
    #[error("the history database failed: {0}")]
    Sql(#[from] rusqlite::Error),
}

impl StoreError {
    /// `Locked` for SQLite's busy and locked errors, otherwise `Sql`.
    pub(crate) fn from_sql(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
                Self::Locked
            }
            _ => Self::Sql(error),
        }
    }
}
