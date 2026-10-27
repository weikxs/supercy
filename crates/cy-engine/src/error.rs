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
    /// An internal consistency check failed (a bug, never user data).
    #[error("an internal check failed; please report this")]
    Internal,
    /// The system random number generator failed.
    #[error("the system random number generator failed")]
    Random,
}
