//! The engine object Swift holds.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use cy_engine::Engine;
use cy_model::ItemId;

use crate::keystore::ShellKeystore;
use crate::{
    ClipboardSnapshot, IngestResult, ItemKind, ItemSummary, Keystore, PasteRepresentation,
    SearchMode, Settings, SupercyError,
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
    fn engine(&self) -> Result<MutexGuard<'_, Engine>, SupercyError> {
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
}
