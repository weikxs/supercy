//! The keychain, implemented by the shell.

use std::sync::Arc;

use cy_model::SecretBytes;
use cy_platform::{KeyStore, KeyStoreError};

use crate::KeystoreError;

/// Secret storage the shell provides over the system keychain (macOS Keychain first).
#[uniffi::export(with_foreign)]
pub trait Keystore: Send + Sync {
    /// The secret stored under `account`, or `nil` when there is none.
    ///
    /// # Errors
    ///
    /// When the keychain is unavailable or refuses access.
    fn read(&self, account: String) -> Result<Option<Vec<u8>>, KeystoreError>;

    /// Stores `secret` under `account`, replacing any previous value.
    ///
    /// # Errors
    ///
    /// When the keychain is unavailable or refuses access.
    fn write(&self, account: String, secret: Vec<u8>) -> Result<(), KeystoreError>;

    /// Deletes the secret under `account`; deleting a missing secret succeeds.
    ///
    /// # Errors
    ///
    /// When the keychain is unavailable or refuses access.
    fn delete(&self, account: String) -> Result<(), KeystoreError>;
}

/// Adapts the shell's keystore to the core's [`KeyStore`] trait.
pub(crate) struct ShellKeystore(pub(crate) Arc<dyn Keystore>);

impl KeyStore for ShellKeystore {
    fn read(&self, account: &str) -> Result<Option<SecretBytes>, KeyStoreError> {
        Ok(self.0.read(account.to_owned())?.map(SecretBytes::new))
    }

    fn write(&self, account: &str, secret: &SecretBytes) -> Result<(), KeyStoreError> {
        Ok(self.0.write(account.to_owned(), secret.expose().to_vec())?)
    }

    fn delete(&self, account: &str) -> Result<(), KeyStoreError> {
        Ok(self.0.delete(account.to_owned())?)
    }
}
