//! In-memory search over the clipboard history: fuzzy matching, exact matching and pinyin.
//!
//! The index holds only what search needs: the first [`INDEX_TEXT_BYTES`] of each item's text,
//! folded to lowercase, and for text with Chinese characters its pinyin forms (at most
//! [`PINYIN_BYTES`] each). Images contribute no text. Memory therefore grows with the number of
//! items, not with their size.

mod fold;
mod index;
mod matcher;
mod pinyin_forms;

pub use index::{Hit, IndexDoc, SearchIndex};
pub use matcher::{MatchMode, Query};

/// Bytes of folded text kept per item.
pub const INDEX_TEXT_BYTES: usize = 512;
/// Bytes kept per pinyin form.
pub const PINYIN_BYTES: usize = 384;
