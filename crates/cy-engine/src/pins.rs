//! Letter keys of pinned items: `a`–`z`, assigned when an item is pinned, changeable by hand.

use cy_model::ItemId;

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
}
