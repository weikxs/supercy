//! Errors as Swift sees them. Messages are user-facing and never contain clipboard content.

use cy_engine::EngineError;
use cy_platform::KeyStoreError;
use cy_store::StoreError;

/// What went wrong, with a message the shell can show.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SupercyError {
    /// The history exists but its key is missing from the keychain.
    #[error("{message}")]
    KeyMissing {
        /// What happened and what to do.
        message: String,
    },
    /// Another Supercy process has the history open.
    #[error("{message}")]
    Locked {
        /// What happened and what to do.
        message: String,
    },
    /// The history was written by a newer Supercy.
    #[error("{message}")]
    TooNew {
        /// What happened and what to do.
        message: String,
    },
    /// A setting is out of range.
    #[error("{message}")]
    InvalidSetting {
        /// Which setting and its range.
        message: String,
    },
    /// The item no longer exists.
    #[error("{message}")]
    NotFound {
        /// What happened.
        message: String,
    },
    /// A pin key is not a free letter from a to z, or the item is not pinned.
    #[error("{message}")]
    InvalidPinKey {
        /// What is wrong.
        message: String,
    },
    /// An item id was not 32 hexadecimal digits.
    #[error("{message}")]
    InvalidId {
        /// What happened.
        message: String,
    },
    /// The keychain failed.
    #[error("{message}")]
    Keystore {
        /// What happened.
        message: String,
    },
    /// Storage or another internal failure.
    #[error("{message}")]
    Internal {
        /// What happened.
        message: String,
    },
}

impl From<EngineError> for SupercyError {
    fn from(error: EngineError) -> Self {
        let message = error.to_string();
        match error {
            EngineError::Store(StoreError::KeyMissing | StoreError::KeyInvalid) => {
                Self::KeyMissing { message }
            }
            EngineError::Store(StoreError::Locked) => Self::Locked { message },
            EngineError::Store(StoreError::TooNew { .. }) => Self::TooNew { message },
            EngineError::Store(StoreError::KeyStore(_)) => Self::Keystore { message },
            EngineError::Setting(_) => Self::InvalidSetting { message },
            EngineError::NotFound => Self::NotFound { message },
            EngineError::NotPinned | EngineError::InvalidPinKey | EngineError::PinKeyTaken => {
                Self::InvalidPinKey { message }
            }
            _ => Self::Internal { message },
        }
    }
}

/// A failure reported by the shell's [`crate::Keystore`].
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum KeystoreError {
    /// The keychain could not be reached.
    #[error("the system keychain is unavailable: {message}")]
    Unavailable {
        /// The shell's description.
        message: String,
    },
    /// The user or the system refused access.
    #[error("access to the system keychain was refused")]
    Denied,
}

impl From<uniffi::UnexpectedUniFFICallbackError> for KeystoreError {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Unavailable {
            message: error.reason,
        }
    }
}

impl From<KeystoreError> for KeyStoreError {
    fn from(error: KeystoreError) -> Self {
        match error {
            KeystoreError::Unavailable { message } => Self::Unavailable(message),
            KeystoreError::Denied => Self::Denied,
        }
    }
}
