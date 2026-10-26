//! Platform services behind traits, so the core is testable without I/O.
//!
//! Real implementations (the system keychain, ...) live in the platform shells or arrive with them;
//! this crate ships the traits and in-memory implementations for tests and development.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cy_model::SecretBytes;

/// Errors from a [`KeyStore`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyStoreError {
    /// The keystore could not be reached.
    #[error("the system keystore is unavailable: {0}")]
    Unavailable(String),
    /// The user or the system refused access.
    #[error("access to the system keystore was refused")]
    Denied,
}

/// Secret storage provided by the operating system (Keychain, Credential Manager, Secret Service).
pub trait KeyStore: Send + Sync {
    /// Reads the secret stored under `account`, if any.
    ///
    /// # Errors
    ///
    /// When the keystore is unavailable or refuses access.
    fn read(&self, account: &str) -> Result<Option<SecretBytes>, KeyStoreError>;

    /// Stores `secret` under `account`, replacing any previous value.
    ///
    /// # Errors
    ///
    /// When the keystore is unavailable or refuses access.
    fn write(&self, account: &str, secret: &SecretBytes) -> Result<(), KeyStoreError>;

    /// Deletes the secret under `account`; deleting a missing secret succeeds.
    ///
    /// # Errors
    ///
    /// When the keystore is unavailable or refuses access.
    fn delete(&self, account: &str) -> Result<(), KeyStoreError>;
}

/// A keystore in memory, for tests and development. Never use it for real data.
#[derive(Default)]
pub struct MemoryKeyStore {
    entries: Mutex<BTreeMap<String, SecretBytes>>,
}

impl MemoryKeyStore {
    /// An empty keystore.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, SecretBytes>>, KeyStoreError> {
        self.entries.lock().map_err(|_| {
            KeyStoreError::Unavailable("the in-memory keystore lock is poisoned".into())
        })
    }
}

impl KeyStore for MemoryKeyStore {
    fn read(&self, account: &str) -> Result<Option<SecretBytes>, KeyStoreError> {
        Ok(self.lock()?.get(account).cloned())
    }

    fn write(&self, account: &str, secret: &SecretBytes) -> Result<(), KeyStoreError> {
        self.lock()?.insert(account.to_owned(), secret.clone());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), KeyStoreError> {
        self.lock()?.remove(account);
        Ok(())
    }
}

/// A source of the current time.
pub trait Clock: Send + Sync {
    /// Milliseconds since the Unix epoch.
    fn now_ms(&self) -> u64;
}

/// The system's wall clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            })
    }
}

/// A clock that moves only when told to, for tests.
#[derive(Debug, Default)]
pub struct ManualClock {
    now_ms: AtomicU64,
}

impl ManualClock {
    /// A clock showing `now_ms`.
    #[must_use]
    pub fn new(now_ms: u64) -> Self {
        Self {
            now_ms: AtomicU64::new(now_ms),
        }
    }

    /// Moves the clock forward by `ms`.
    pub fn advance(&self, ms: u64) {
        self.now_ms.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.now_ms.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_keystore_reads_back_what_was_written_and_deletes() {
        let store = MemoryKeyStore::new();
        let secret = SecretBytes::from(&b"demo-key"[..]);

        assert_eq!(store.read("data-key"), Ok(None));
        store.write("data-key", &secret).unwrap();
        assert_eq!(store.read("data-key"), Ok(Some(secret)));
        store.delete("data-key").unwrap();
        store.delete("data-key").unwrap();
        assert_eq!(store.read("data-key"), Ok(None));
    }

    #[test]
    fn manual_clock_moves_only_when_advanced() {
        let clock = ManualClock::new(1_000);

        clock.advance(500);

        assert_eq!(clock.now_ms(), 1_500);
    }

    #[test]
    fn system_clock_is_after_2020() {
        const JAN_2020_MS: u64 = 1_577_836_800_000;

        assert!(SystemClock.now_ms() > JAN_2020_MS);
    }
}
