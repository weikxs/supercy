//! The engine: one open history with its search index and in-memory summaries.

use std::collections::HashMap;
use std::path::Path;

use cy_filter::{Capture, DropReason, FilterDecision, decide};
use cy_model::{
    ClipboardSnapshot, DeviceTag, Fingerprint, Hlc, HlcClock, Item, ItemId, RepKind, Representation,
};
use cy_platform::KeyStore;
use cy_search::{IndexDoc, MatchMode, Query, SearchIndex};
use cy_store::Store;

use crate::{EngineError, ItemSummary, Settings};

/// Meta record holding the settings.
const SETTINGS_KEY: &str = "settings";
/// Meta record holding this device's tag.
const DEVICE_KEY: &str = "device-tag";

/// What happened to a snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IngestOutcome {
    /// A new item was stored; `evicted` items were removed by the limits.
    Stored {
        /// The new item.
        id: ItemId,
        /// Items removed to stay within the limits.
        evicted: Vec<ItemId>,
    },
    /// The content was already in the history; that item moved to the top.
    Updated {
        /// The existing item.
        id: ItemId,
    },
    /// Nothing was kept.
    Dropped {
        /// Why.
        reason: DropReason,
    },
}

/// The result of [`Engine::ingest`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IngestReport {
    /// What happened.
    pub outcome: IngestOutcome,
    /// When the shell should call [`Engine::tick`] next, if ever.
    pub next_expiry_ms: Option<u64>,
}

/// One open history. Methods that change it take `&mut self`; wrap it in a mutex to share it.
pub struct Engine {
    store: Store,
    index: SearchIndex,
    summaries: HashMap<ItemId, ItemSummary>,
    clock: HlcClock,
    settings: Settings,
    unreadable: usize,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("items", &self.summaries.len())
            .finish_non_exhaustive()
    }
}

impl Engine {
    /// Opens the history in `dir`, expires what expired by `now_ms`, and loads the rest.
    ///
    /// # Errors
    ///
    /// Storage errors (missing key, locked, newer format, ...) and random-number failures.
    pub fn open(dir: &Path, keystore: &dyn KeyStore, now_ms: u64) -> Result<Self, EngineError> {
        let mut store = Store::open(dir, keystore)?;
        let device = load_or_create_device(&mut store)?;
        let settings = load_settings(&store)?;
        store.expire(now_ms)?;
        let report = store.load()?;
        let newest = report.items.iter().map(|item| item.last_used).max();
        let clock = HlcClock::resume(newest.map_or(Hlc::new(0, 0, device), |h| {
            Hlc::new(h.wall_ms(), h.counter(), device)
        }));
        Ok(Self {
            index: SearchIndex::from_items(&report.items),
            summaries: report
                .items
                .iter()
                .map(|item| (item.id, ItemSummary::of(item)))
                .collect(),
            store,
            clock,
            settings,
            unreadable: report.unreadable.len(),
        })
    }

    /// This device's tag, stamped on the items it copies.
    #[must_use]
    pub fn device(&self) -> DeviceTag {
        self.clock.last().device()
    }

    /// The current settings.
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Number of readable items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.summaries.len()
    }

    /// Whether the history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.summaries.is_empty()
    }

    /// Rows that could not be read when the history was opened (kept on disk untouched).
    #[must_use]
    pub fn unreadable(&self) -> usize {
        self.unreadable
    }

    /// Filters `snapshot` and stores what is kept: a new item, or the existing item with the
    /// same content moved to the top.
    ///
    /// # Errors
    ///
    /// Storage failures; the history is unchanged then.
    pub fn ingest(&mut self, snapshot: &ClipboardSnapshot) -> Result<IngestReport, EngineError> {
        let outcome = match decide(snapshot, &self.settings.filter_policy()) {
            FilterDecision::Drop(reason) => IngestOutcome::Dropped { reason },
            FilterDecision::Keep(capture) => self.keep(capture, snapshot.captured_at_ms)?,
        };
        Ok(IngestReport {
            outcome,
            next_expiry_ms: self.next_expiry(),
        })
    }

    fn keep(&mut self, capture: Capture, now_ms: u64) -> Result<IngestOutcome, EngineError> {
        let fingerprint = Fingerprint::of(self.store.fingerprint_key(), &capture.reps)
            .ok_or(EngineError::Internal)?;
        let at = self.stamp(now_ms);
        let existing = match self.store.find_by_fingerprint(&fingerprint)? {
            Some(id) => self.readable_or_remove(id)?,
            None => None,
        };
        let (item, is_new) = match existing {
            Some(old) => (refreshed(&old, capture, at), false),
            None => (self.new_item(capture, fingerprint, at, now_ms)?, true),
        };
        let evicted = self.store.save(&item, &self.settings.limits())?;
        self.remember(&item);
        self.forget(&evicted);
        Ok(if is_new {
            IngestOutcome::Stored {
                id: item.id,
                evicted,
            }
        } else {
            IngestOutcome::Updated { id: item.id }
        })
    }

    /// The stored item `id`, or `None` after removing a row that can no longer be read (its
    /// content is lost either way; a fresh copy replaces it).
    fn readable_or_remove(&mut self, id: ItemId) -> Result<Option<Item>, EngineError> {
        match self.store.get(id) {
            Ok(item) => Ok(item),
            Err(cy_store::StoreError::Unseal) => {
                self.store.delete(id)?;
                self.forget(&[id]);
                Ok(None)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn new_item(
        &self,
        capture: Capture,
        fingerprint: Fingerprint,
        at: Hlc,
        now_ms: u64,
    ) -> Result<Item, EngineError> {
        Ok(Item {
            id: ItemId::random().map_err(|_| EngineError::Random)?,
            fingerprint,
            reps: capture.reps,
            source: capture.source,
            origin: self.device(),
            first_copied_ms: now_ms,
            last_used: at,
            pinned: false,
            pinned_at: None,
            sensitivity: capture.sensitivity,
            truncated: capture.truncated,
        })
    }

    /// Items in list order: pinned first, then most recently used; optionally of one kind.
    #[must_use]
    pub fn list(&self, kind: Option<RepKind>, offset: usize, limit: usize) -> Vec<ItemSummary> {
        self.summaries_of(self.index.list(kind, offset, limit))
    }

    /// The best `limit` matches for `query`; an empty query lists the history.
    #[must_use]
    pub fn search(
        &self,
        query: &str,
        mode: MatchMode,
        kind: Option<RepKind>,
        limit: usize,
    ) -> Vec<ItemSummary> {
        let hits = self.index.search(&Query::parse(query, mode), kind, limit);
        self.summaries_of(hits.into_iter().map(|hit| hit.id).collect())
    }

    /// The full content of `id`, for pasting.
    ///
    /// # Errors
    ///
    /// [`EngineError::NotFound`] for an unknown id; storage failures.
    pub fn payload(&self, id: ItemId) -> Result<Vec<Representation>, EngineError> {
        Ok(self.store.get(id)?.ok_or(EngineError::NotFound)?.reps)
    }

    /// Pins or unpins `id`.
    ///
    /// # Errors
    ///
    /// [`EngineError::NotFound`] for an unknown id; storage failures.
    pub fn set_pinned(&mut self, id: ItemId, pinned: bool, now_ms: u64) -> Result<(), EngineError> {
        let at = self.stamp(now_ms);
        self.update(id, |item| item.with_pinned(pinned, at))
    }

    /// Records that `id` was pasted, moving it to the top.
    ///
    /// # Errors
    ///
    /// [`EngineError::NotFound`] for an unknown id; storage failures.
    pub fn mark_used(&mut self, id: ItemId, now_ms: u64) -> Result<(), EngineError> {
        let at = self.stamp(now_ms);
        self.update(id, |item| item.touched(at))
    }

    fn update(
        &mut self,
        id: ItemId,
        change: impl FnOnce(&Item) -> Item,
    ) -> Result<(), EngineError> {
        let item = change(&self.store.get(id)?.ok_or(EngineError::NotFound)?);
        let evicted = self.store.save(&item, &self.settings.limits())?;
        self.remember(&item);
        self.forget(&evicted);
        Ok(())
    }

    /// Deletes `id`; returns whether it existed.
    ///
    /// # Errors
    ///
    /// Storage failures.
    pub fn delete(&mut self, id: ItemId) -> Result<bool, EngineError> {
        let existed = self.store.delete(id)?;
        self.forget(&[id]);
        Ok(existed)
    }

    /// Deletes everything, or everything but the pinned items; returns how many were deleted.
    ///
    /// # Errors
    ///
    /// Storage failures.
    pub fn clear(&mut self, keep_pinned: bool) -> Result<usize, EngineError> {
        let deleted = self.store.clear(keep_pinned)?;
        let remaining = self.store.load()?;
        self.index = SearchIndex::from_items(&remaining.items);
        self.summaries = remaining
            .items
            .iter()
            .map(|item| (item.id, ItemSummary::of(item)))
            .collect();
        Ok(deleted)
    }

    /// Deletes sensitive items that expired by `now_ms`; returns when to call again, if ever.
    ///
    /// # Errors
    ///
    /// Storage failures.
    pub fn tick(&mut self, now_ms: u64) -> Result<Option<u64>, EngineError> {
        // The summaries know every expiry, so the database is touched only when something is due.
        if self.next_expiry().is_some_and(|at| at <= now_ms) {
            let expired = self.store.expire(now_ms)?;
            self.forget(&expired);
        }
        Ok(self.next_expiry())
    }

    /// The earliest expiry of an unpinned sensitive item, from the in-memory summaries.
    fn next_expiry(&self) -> Option<u64> {
        self.summaries
            .values()
            .filter(|s| !s.pinned)
            .filter_map(|s| s.sensitive_until_ms)
            .min()
    }

    /// Validates, stores and applies `settings`; lowered limits take effect at once.
    ///
    /// # Errors
    ///
    /// [`EngineError::Setting`] for a value out of range (nothing changes); storage failures.
    pub fn update_settings(&mut self, settings: Settings) -> Result<&Settings, EngineError> {
        settings.validate()?;
        let mut encoded = Vec::new();
        ciborium::into_writer(&settings, &mut encoded)
            .map_err(|_| cy_store::StoreError::Corrupt)?;
        self.store.write_meta(SETTINGS_KEY, &encoded)?;
        let removed = self.store.apply_limits(&settings.limits())?;
        self.forget(&removed);
        self.settings = settings;
        Ok(&self.settings)
    }

    /// Approximate heap memory held by the engine's in-memory state (index and summaries).
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        let per_entry = std::mem::size_of::<(ItemId, ItemSummary)>() + std::mem::size_of::<u64>();
        self.index.heap_bytes()
            + self.summaries.capacity() * per_entry
            + self
                .summaries
                .values()
                .map(ItemSummary::heap_bytes)
                .sum::<usize>()
    }

    fn stamp(&mut self, now_ms: u64) -> Hlc {
        let (clock, stamp) = self.clock.tick(now_ms);
        self.clock = clock;
        stamp
    }

    fn remember(&mut self, item: &Item) {
        self.index.upsert(&IndexDoc::from_item(item));
        self.summaries.insert(item.id, ItemSummary::of(item));
    }

    fn forget(&mut self, ids: &[ItemId]) {
        for id in ids {
            self.index.remove(*id);
            self.summaries.remove(id);
        }
    }

    fn summaries_of(&self, ids: Vec<ItemId>) -> Vec<ItemSummary> {
        ids.into_iter()
            .filter_map(|id| self.summaries.get(&id).cloned())
            .collect()
    }
}

/// The existing item copied again: new last use, missing forms added, and the source app,
/// sensitivity and truncation of the latest copy.
fn refreshed(old: &Item, capture: Capture, at: Hlc) -> Item {
    Item {
        source: capture.source,
        sensitivity: capture.sensitivity,
        truncated: capture.truncated,
        ..old.merged_with(&capture.reps, at)
    }
}

fn load_or_create_device(store: &mut Store) -> Result<DeviceTag, EngineError> {
    if let Some(stored) = store.read_meta(DEVICE_KEY)? {
        let bytes =
            <[u8; 8]>::try_from(stored.expose()).map_err(|_| cy_store::StoreError::Corrupt)?;
        return Ok(DeviceTag(bytes));
    }
    let random = ItemId::random().map_err(|_| EngineError::Random)?;
    let mut tag = [0u8; 8];
    tag.copy_from_slice(&random.as_bytes()[..8]);
    store.write_meta(DEVICE_KEY, &tag)?;
    Ok(DeviceTag(tag))
}

/// Stored settings, or the defaults when there are none. Settings that no longer validate (for
/// example after ranges changed) fall back to the defaults rather than blocking the history.
fn load_settings(store: &Store) -> Result<Settings, EngineError> {
    let Some(stored) = store.read_meta(SETTINGS_KEY)? else {
        return Ok(Settings::default());
    };
    let settings: Settings = ciborium::from_reader(stored.expose()).unwrap_or_default();
    Ok(if settings.validate().is_ok() {
        settings
    } else {
        Settings::default()
    })
}
