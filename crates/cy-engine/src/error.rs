//! Engine errors. Messages never contain clipboard content.

use cy_store::StoreError;

use crate::SettingsError;

/// What went wrong in the engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Storage failed; see the inner error for what to do.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A setting is out of range.
    #[error(transparent)]
    Setting(#[from] SettingsError),
    /// No item has this id.
    #[error("the item no longer exists")]
    NotFound,
    /// Only pinned items have letter keys.
    #[error("only pinned items have a letter key; pin the item first")]
    NotPinned,
    /// Letter keys are `a` to `z`.
    #[error("a pin key is a letter from a to z")]
    InvalidPinKey,
    /// Another pinned item has this letter.
    #[error("another pinned item already uses this letter; choose another one")]
    PinKeyTaken,
    /// A file to import is not a Supercy pins file this version reads, or is damaged.
    #[error("this is not a Supercy pins file, or it is damaged or from a newer Supercy")]
    InvalidPinsFile,
    /// A file could not be read or written.
    #[error("the file could not be read or written ({0})")]
    File(std::io::ErrorKind),
    /// An internal consistency check failed (a bug, never user data).
    #[error("an internal check failed; please report this")]
    Internal,
    /// The system random number generator failed.
    #[error("the system random number generator failed")]
    Random,
}
