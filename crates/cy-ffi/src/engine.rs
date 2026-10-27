//! The engine object Swift holds.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use cy_engine::Engine;
use cy_model::ItemId;

use crate::keystore::ShellKeystore;
use crate::{
    ClipboardSnapshot, IngestResult, ItemKind, ItemSummary, Keystore, PasteRepresentation,
    PinsImport, SearchMode, Settings, SupercyError,
};

/// The version of the core library.
#[uniffi::export]
#[must_use]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// One open history. Safe to call from any thread; calls are serialized.
#[derive(uniffi::Object)]
pub struct SupercyEngine {
    inner: Mutex<Engine>,
}

impl std::fmt::Debug for SupercyEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SupercyEngine(..)")
    }
}

fn parse_id(id: &str) -> Result<ItemId, SupercyError> {
    ItemId::from_hex(id).map_err(|e| SupercyError::InvalidId {
        message: e.to_string(),
    })
}

fn to_count(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

impl SupercyEngine {
    pub(crate) fn engine(&self) -> Result<MutexGuard<'_, Engine>, SupercyError> {
        self.inner.lock().map_err(|_| SupercyError::Internal {
            message: "the engine stopped after an earlier failure; restart Supercy".into(),
        })
    }
}

#[uniffi::export]
#[allow(
    clippy::needless_pass_by_value,
    reason = "UniFFI hands over owned values"
)]
impl SupercyEngine {
    /// Opens (or creates) the history in `data_dir`, with its key in `keystore`.
    ///
    /// # Errors
    ///
    /// `KeyMissing`, `Locked`, `TooNew`, `Keystore` or `Internal`.
    #[uniffi::constructor]
    pub fn open(
        data_dir: String,
        keystore: Arc<dyn Keystore>,
        now_ms: u64,
    ) -> Result<Arc<Self>, SupercyError> {
        let engine = Engine::open(Path::new(&data_dir), &ShellKeystore(keystore), now_ms)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(engine),
        }))
    }

    /// Filters and stores a clipboard change.
    ///
    /// # Errors
    ///
    /// `Internal` when storage fails.
    pub fn ingest(&self, snapshot: ClipboardSnapshot) -> Result<IngestResult, SupercyError> {
        Ok(self.engine()?.ingest(&snapshot.into())?.into())
    }

    /// Items in list order (pinned first, then most recent), optionally of one kind.
    ///
    /// # Errors
    ///
    /// `Internal` after an earlier failure.
    pub fn list(
        &self,
        kind: Option<ItemKind>,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<ItemSummary>, SupercyError> {
        let engine = self.engine()?;
        Ok(engine
            .list(kind.map(Into::into), to_count(offset), to_count(limit))
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// The best matches for `query` (an empty query lists everything).
    ///
    /// # Errors
    ///
    /// `Internal` after an earlier failure.
    pub fn search(
        &self,
        query: String,
        mode: SearchMode,
        kind: Option<ItemKind>,
        limit: u32,
    ) -> Result<Vec<ItemSummary>, SupercyError> {
        let engine = self.engine()?;
        let found = engine.search(&query, mode.into(), kind.map(Into::into), to_count(limit));
        Ok(found.into_iter().map(Into::into).collect())
    }

    /// Every form of an item, to put back on the clipboard.
    ///
    /// # Errors
    ///
    /// `InvalidId`, `NotFound` or `Internal`.
    pub fn payload(&self, id: String) -> Result<Vec<PasteRepresentation>, SupercyError> {
        let reps = self.engine()?.payload(parse_id(&id)?)?;
        Ok(reps.into_iter().map(Into::into).collect())
    }

    /// Pins or unpins an item.
    ///
    /// # Errors
    ///
    /// `InvalidId`, `NotFound` or `Internal`.
    pub fn set_pinned(&self, id: String, pinned: bool, now_ms: u64) -> Result<(), SupercyError> {
        Ok(self.engine()?.set_pinned(parse_id(&id)?, pinned, now_ms)?)
    }

    /// Records that an item was pasted, moving it to the top.
    ///
    /// # Errors
    ///
    /// `InvalidId`, `NotFound` or `Internal`.
    pub fn mark_used(&self, id: String, now_ms: u64) -> Result<(), SupercyError> {
        Ok(self.engine()?.mark_used(parse_id(&id)?, now_ms)?)
    }

    /// Deletes an item; returns whether it existed.
    ///
    /// # Errors
    ///
    /// `InvalidId` or `Internal`.
    pub fn delete(&self, id: String) -> Result<bool, SupercyError> {
        Ok(self.engine()?.delete(parse_id(&id)?)?)
    }

    /// Deletes the history (optionally keeping pinned items); returns how many items were deleted.
    ///
    /// # Errors
    ///
    /// `Internal` when storage fails.
    pub fn clear(&self, keep_pinned: bool) -> Result<u64, SupercyError> {
        Ok(self.engine()?.clear(keep_pinned)? as u64)
    }

    /// Deletes expired sensitive items; returns when to call again, if ever. Call it at the time
    /// `ingest` or the previous `tick` returned, and when the history window opens.
    ///
    /// # Errors
    ///
    /// `Internal` when storage fails.
    pub fn tick(&self, now_ms: u64) -> Result<Option<u64>, SupercyError> {
        Ok(self.engine()?.tick(now_ms)?)
    }

    /// The current settings.
    ///
    /// # Errors
    ///
    /// `Internal` after an earlier failure.
    pub fn settings(&self) -> Result<Settings, SupercyError> {
        Ok(self.engine()?.settings().clone().into())
    }

    /// Checks, stores and applies new settings; returns them.
    ///
    /// # Errors
    ///
    /// `InvalidSetting` (nothing changes) or `Internal`.
    pub fn update_settings(&self, settings: Settings) -> Result<Settings, SupercyError> {
        Ok(self
            .engine()?
            .update_settings(settings.into())?
            .clone()
            .into())
    }

    /// Number of items in the history.
    ///
    /// # Errors
    ///
    /// `Internal` after an earlier failure.
    pub fn item_count(&self) -> Result<u64, SupercyError> {
        Ok(self.engine()?.len() as u64)
    }

    /// The stored thumbnail (PNG) of an image item, if it has one: small, for the history list.
    ///
    /// # Errors
    ///
    /// `InvalidId` or `Internal`.
    pub fn thumbnail(&self, id: String) -> Result<Option<Vec<u8>>, SupercyError> {
        Ok(self.engine()?.thumbnail(parse_id(&id)?)?)
    }

    /// Pauses recording until `until_ms`, or until `resume` when `nil`. Copies made while
    /// paused are never read.
    ///
    /// # Errors
    ///
    /// `Internal` after an earlier failure.
    pub fn pause(&self, until_ms: Option<u64>) -> Result<(), SupercyError> {
        self.engine()?.pause(until_ms);
        Ok(())
    }

    /// Resumes recording.
    ///
    /// # Errors
    ///
    /// `Internal` after an earlier failure.
    pub fn resume(&self) -> Result<(), SupercyError> {
        self.engine()?.resume();
        Ok(())
    }

    /// Ignores the next copy.
    ///
    /// # Errors
    ///
    /// `Internal` after an earlier failure.
    pub fn skip_next(&self) -> Result<(), SupercyError> {
        self.engine()?.skip_next();
        Ok(())
    }

    /// Whether recording is paused at `now_ms`, and how many copies will be skipped.
    ///
    /// # Errors
    ///
    /// `Internal` after an earlier failure.
    pub fn pause_status(&self, now_ms: u64) -> Result<PauseStatus, SupercyError> {
        let state = self.engine()?.pause_state(now_ms);
        Ok(PauseStatus {
            paused: state.is_paused(now_ms),
            paused_until_ms: state.paused_until_ms.filter(|until| *until != u64::MAX),
            skip_next: state.skip_next,
        })
    }

    /// Gives pinned item `id` the letter `key` (`a`–`z`), or removes it with `nil`.
    ///
    /// # Errors
    ///
    /// `InvalidId`, `NotFound`, `InvalidPinKey` (not pinned, not a letter, or taken) or `Internal`.
    pub fn set_pin_key(&self, id: String, key: Option<String>) -> Result<(), SupercyError> {
        let key = key.map(|k| single_char(&k)).transpose()?;
        Ok(self.engine()?.set_pin_key(parse_id(&id)?, key)?)
    }

    /// Moves pinned item `id` to place `to_index` among the pinned items (0 is the first; past
    /// the end means last).
    ///
    /// # Errors
    ///
    /// `InvalidId`, `NotFound`, `InvalidPinKey` (not pinned) or `Internal`.
    pub fn move_pin(&self, id: String, to_index: u32, now_ms: u64) -> Result<(), SupercyError> {
        let to = usize::try_from(to_index).unwrap_or(usize::MAX);
        Ok(self.engine()?.move_pin(parse_id(&id)?, to, now_ms)?)
    }

    /// Writes the pinned items to the file at `path` (JSON, not encrypted, readable by this
    /// user only); returns how many were written.
    ///
    /// # Errors
    ///
    /// `PinsFile` when the file cannot be written; `Internal`.
    pub fn export_pins(&self, path: String) -> Result<u32, SupercyError> {
        let count = self.engine()?.export_pins(Path::new(&path))?;
        Ok(u32::try_from(count).unwrap_or(u32::MAX))
    }

    /// Pins the items of the pins file at `path`, after the pinned items already here.
    ///
    /// # Errors
    ///
    /// `PinsFile` when it cannot be read or is not a pins file; `Internal`.
    pub fn import_pins(&self, path: String, now_ms: u64) -> Result<PinsImport, SupercyError> {
        Ok(self.engine()?.import_pins(Path::new(&path), now_ms)?.into())
    }

    /// The pinned item with letter `key`, if any.
    ///
    /// # Errors
    ///
    /// `InvalidPinKey` when `key` is not one character; `Internal` after an earlier failure.
    pub fn item_for_pin_key(&self, key: String) -> Result<Option<String>, SupercyError> {
        let key = single_char(&key)?;
        Ok(self
            .engine()?
            .item_for_pin_key(key)
            .map(|id| id.to_string()))
    }
}

/// Whether recording is paused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct PauseStatus {
    /// Paused now.
    pub paused: bool,
    /// When a timed pause ends; `nil` when not paused or paused until resumed.
    pub paused_until_ms: Option<u64>,
    /// Coming copies that will be skipped.
    pub skip_next: u32,
}

fn single_char(text: &str) -> Result<char, SupercyError> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(SupercyError::InvalidPinKey {
            message: "a pin key is a letter from a to z".into(),
        }),
    }
}

/// Why `query` is not a valid regular expression in `mode`, if it is not (for the search field).
#[uniffi::export]
#[must_use]
#[allow(
    clippy::needless_pass_by_value,
    reason = "UniFFI hands over owned values"
)]
pub fn query_error(query: String, mode: SearchMode) -> Option<String> {
    Engine::query_error(&query, mode.into())
}

/// The clipboard type under which the shell passes an image's thumbnail with a snapshot.
#[uniffi::export]
#[must_use]
pub fn thumbnail_type() -> String {
    cy_engine::THUMBNAIL_TYPE.to_owned()
}

/// Why an ignore pattern is not valid, if it is not (for the settings screen).
#[uniffi::export]
#[must_use]
#[allow(
    clippy::needless_pass_by_value,
    reason = "UniFFI hands over owned values"
)]
pub fn check_ignore_pattern(pattern: String) -> Option<String> {
    cy_engine::check_pattern(&pattern).err()
}
