//! Item identifiers.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Length of an [`ItemId`] in bytes.
pub const ITEM_ID_LEN: usize = 16;

/// Identifies one history item on every device: 128 random bits.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ItemId([u8; ITEM_ID_LEN]);

/// Errors from creating or decoding an [`ItemId`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdError {
    /// The operating system's random number generator failed.
    #[error("the system random number generator failed")]
    Random,
    /// The input did not have [`ITEM_ID_LEN`] bytes.
    #[error("an item id has 16 bytes, not {0}")]
    Length(usize),
}

impl ItemId {
    /// A new random id.
    ///
    /// # Errors
    ///
    /// [`IdError::Random`] when the system random number generator fails.
    pub fn random() -> Result<Self, IdError> {
        let mut bytes = [0u8; ITEM_ID_LEN];
        getrandom::fill(&mut bytes).map_err(|_| IdError::Random)?;
        Ok(Self(bytes))
    }

    /// An id from its bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; ITEM_ID_LEN]) -> Self {
        Self(bytes)
    }

    /// An id from a slice, as read from storage.
    ///
    /// # Errors
    ///
    /// [`IdError::Length`] when the slice does not have [`ITEM_ID_LEN`] bytes.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, IdError> {
        <[u8; ITEM_ID_LEN]>::try_from(bytes)
            .map(Self)
            .map_err(|_| IdError::Length(bytes.len()))
    }

    /// The id's bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ITEM_ID_LEN] {
        &self.0
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl fmt::Debug for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ItemId({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_ids_differ() {
        let a = ItemId::random().unwrap();
        let b = ItemId::random().unwrap();

        assert_ne!(a, b);
    }

    #[test]
    fn slice_round_trip_and_length_check() {
        let id = ItemId::from_bytes([7; ITEM_ID_LEN]);

        assert_eq!(ItemId::from_slice(id.as_bytes()), Ok(id));
        assert_eq!(ItemId::from_slice(&[1, 2, 3]), Err(IdError::Length(3)));
    }

    #[test]
    fn displays_as_lowercase_hex() {
        let id = ItemId::from_bytes([0xab; ITEM_ID_LEN]);

        assert_eq!(id.to_string(), "ab".repeat(ITEM_ID_LEN));
        assert!(format!("{id:?}").starts_with("ItemId(abab"));
    }
}
