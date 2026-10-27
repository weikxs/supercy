//! Pinned items: their letter keys (`a`–`z`, assigned when an item is pinned, changeable by
//! hand) and their order (the order they were pinned in, rearranged by hand).

use cy_model::{Hlc, ItemId};

use crate::{Engine, EngineError};

/// The letters available as pin keys, in assignment order.
const PIN_KEYS: std::ops::RangeInclusive<char> = 'a'..='z';

impl Engine {
    /// The first letter no pinned item uses, if any is left.
    pub(crate) fn free_pin_key(&self) -> Option<char> {
        PIN_KEYS
            .into_iter()
            .find(|key| self.item_for_pin_key(*key).is_none())
    }

    /// The pinned item with letter `key`, if any.
    #[must_use]
    pub fn item_for_pin_key(&self, key: char) -> Option<ItemId> {
        self.summaries
            .values()
            .find(|s| s.pinned && s.pin_key == Some(key))
            .map(|s| s.id)
    }

    /// Gives pinned item `id` the letter `key` (`a`–`z`), or removes its letter with `None`.
    ///
    /// # Errors
    ///
    /// [`EngineError::NotFound`], [`EngineError::NotPinned`], [`EngineError::InvalidPinKey`] for
    /// anything but `a`–`z`, [`EngineError::PinKeyTaken`] when another item has it; storage
    /// failures.
    pub fn set_pin_key(&mut self, id: ItemId, key: Option<char>) -> Result<(), EngineError> {
        let summary = self.summaries.get(&id).ok_or(EngineError::NotFound)?;
        if !summary.pinned {
            return Err(EngineError::NotPinned);
        }
        if let Some(key) = key {
            if !PIN_KEYS.contains(&key) {
                return Err(EngineError::InvalidPinKey);
            }
            if self.item_for_pin_key(key).is_some_and(|other| other != id) {
                return Err(EngineError::PinKeyTaken);
            }
        }
        self.update(id, |item| item.with_pin_key(key))
    }

    /// Moves pinned item `id` to place `to` among the pinned items (0 is the first; past the end
    /// means last). Pinned items are listed by `pinned_at`, so the items from the first place
    /// that changes onwards get new stamps in their new order; the ones before keep theirs.
    ///
    /// # Errors
    ///
    /// [`EngineError::NotFound`], [`EngineError::NotPinned`]; storage failures.
    pub fn move_pin(&mut self, id: ItemId, to: usize, now_ms: u64) -> Result<(), EngineError> {
        let summary = self.summaries.get(&id).ok_or(EngineError::NotFound)?;
        if !summary.pinned {
            return Err(EngineError::NotPinned);
        }
        let mut order = self.pinned_order();
        let from = order
            .iter()
            .position(|pinned| *pinned == id)
            .ok_or(EngineError::Internal)?;
        order.remove(from);
        let to = to.min(order.len());
        order.insert(to, id);
        let stamps: Vec<(ItemId, Hlc)> = order[from.min(to)..]
            .iter()
            .map(|pinned| (*pinned, self.stamp(now_ms)))
            .collect();
        self.store.set_pinned_at(&stamps)?;
        for (pinned, at) in stamps {
            self.index.set_pinned_at(pinned, at);
            if let Some(summary) = self.summaries.get_mut(&pinned) {
                summary.pinned_at = Some(at);
            }
        }
        Ok(())
    }

    /// The pinned items in list order.
    fn pinned_order(&self) -> Vec<ItemId> {
        let mut pinned: Vec<(Option<Hlc>, ItemId)> = self
            .summaries
            .values()
            .filter(|s| s.pinned)
            .map(|s| (s.pinned_at, s.id))
            .collect();
        pinned.sort_unstable();
        pinned.into_iter().map(|(_, id)| id).collect()
    }
}
