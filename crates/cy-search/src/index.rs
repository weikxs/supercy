//! The search index.

use std::borrow::Cow;
use std::cmp::Ordering;

use cy_model::{Hlc, Item, ItemId, RepKind, Representation};

use crate::fold::fold;
use crate::matcher::{MatchMode, Query, char_mask, score};
use crate::pinyin_forms::{PinyinForms, pinyin_forms};
use crate::{INDEX_TEXT_BYTES, PINYIN_BYTES};

/// What the index needs to know about one item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexDoc<'a> {
    /// The item.
    pub id: ItemId,
    /// Kind of its primary representation.
    pub kind: RepKind,
    /// Whether it is pinned.
    pub pinned: bool,
    /// Its last use, for ordering.
    pub last_used: Hlc,
    /// Its searchable text (cut and folded by the index).
    pub text: Cow<'a, str>,
}

impl<'a> IndexDoc<'a> {
    /// The document for `item`.
    #[must_use]
    pub fn from_item(item: &'a Item) -> Self {
        Self {
            id: item.id,
            kind: item.kind(),
            pinned: item.pinned,
            last_used: item.last_used,
            text: item
                .primary()
                .map_or(Cow::Borrowed(""), Representation::search_text),
        }
    }
}

/// A search result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The item.
    pub id: ItemId,
    /// How well it matched; higher is better. 0 for an empty query.
    pub score: u32,
}

#[derive(Clone, Debug)]
struct Entry {
    id: ItemId,
    kind: RepKind,
    pinned: bool,
    last_used: Hlc,
    text: Box<str>,
    pinyin: Option<Box<PinyinForms>>,
    mask: u64,
}

impl Entry {
    fn new(doc: &IndexDoc<'_>) -> Self {
        let text = fold(&doc.text, INDEX_TEXT_BYTES);
        let pinyin = pinyin_forms(&text, PINYIN_BYTES).map(Box::new);
        let pinyin_chars = pinyin.iter().flat_map(|p| {
            p.primary
                .chars()
                .chain(p.alternate.iter().flat_map(|a| a.chars()))
        });
        let mask = char_mask(text.chars().chain(pinyin_chars));
        Self {
            id: doc.id,
            kind: doc.kind,
            pinned: doc.pinned,
            last_used: doc.last_used,
            text: text.into_boxed_str(),
            pinyin,
            mask,
        }
    }

    fn haystacks(&self, mode: MatchMode) -> impl Iterator<Item = &str> {
        let pinyin = self.pinyin.iter().filter(move |_| mode == MatchMode::Fuzzy);
        std::iter::once(&*self.text)
            .chain(pinyin.clone().map(|p| &*p.primary))
            .chain(pinyin.filter_map(|p| p.alternate.as_deref()))
    }

    fn score(&self, query: &Query) -> Option<u32> {
        query.terms().iter().try_fold(0u32, |total, term| {
            let best = self
                .haystacks(query.mode())
                .filter_map(|hay| score(hay, term, query.mode()))
                .max()?;
            Some(total.saturating_add(best))
        })
    }

    fn heap_bytes(&self) -> usize {
        self.text.len()
            + self
                .pinyin
                .as_ref()
                .map_or(0, |p| std::mem::size_of::<PinyinForms>() + p.heap_bytes())
    }

    /// List order: pinned first, then most recently used.
    fn list_order(&self, other: &Self) -> Ordering {
        other
            .pinned
            .cmp(&self.pinned)
            .then(other.last_used.cmp(&self.last_used))
    }
}

/// Whether an item of `kind` passes a kind filter; rich text counts as text.
fn kind_matches(filter: Option<RepKind>, kind: RepKind) -> bool {
    let simplify = |k| {
        if k == RepKind::RichText {
            RepKind::Text
        } else {
            k
        }
    };
    filter.is_none_or(|f| simplify(f) == simplify(kind))
}

/// The in-memory search index. Building and searching allocate only for results.
#[derive(Clone, Debug, Default)]
pub struct SearchIndex {
    entries: Vec<Entry>,
}

impl SearchIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An index of `items`.
    #[must_use]
    pub fn from_items<'a>(items: impl IntoIterator<Item = &'a Item>) -> Self {
        let entries = items
            .into_iter()
            .map(|item| Entry::new(&IndexDoc::from_item(item)))
            .collect();
        Self { entries }
    }

    /// Number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Adds `doc`, or replaces the entry with the same id.
    pub fn upsert(&mut self, doc: &IndexDoc<'_>) {
        let entry = Entry::new(doc);
        match self.entries.iter_mut().find(|e| e.id == doc.id) {
            Some(existing) => *existing = entry,
            None => self.entries.push(entry),
        }
    }

    /// Removes the entry for `id`; returns whether there was one.
    pub fn remove(&mut self, id: ItemId) -> bool {
        let position = self.entries.iter().position(|e| e.id == id);
        position.map(|at| self.entries.swap_remove(at)).is_some()
    }

    /// Items in list order (pinned first, then most recent), optionally of one kind.
    #[must_use]
    pub fn list(&self, kind: Option<RepKind>, offset: usize, limit: usize) -> Vec<ItemId> {
        let mut entries: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|e| kind_matches(kind, e.kind))
            .collect();
        entries.sort_unstable_by(|a, b| a.list_order(b));
        entries
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|e| e.id)
            .collect()
    }

    /// The best `limit` matches for `query`: by score, then pinned, then most recent. An empty
    /// query returns the list order.
    #[must_use]
    pub fn search(&self, query: &Query, kind: Option<RepKind>, limit: usize) -> Vec<Hit> {
        if query.is_empty() {
            return self
                .list(kind, 0, limit)
                .into_iter()
                .map(|id| Hit { id, score: 0 })
                .collect();
        }
        let masks: Vec<u64> = query
            .terms()
            .iter()
            .map(|t| char_mask(t.iter().copied()))
            .collect();
        let mut hits: Vec<(u32, &Entry)> = self
            .entries
            .iter()
            .filter(|e| kind_matches(kind, e.kind) && masks.iter().all(|m| e.mask & m == *m))
            .filter_map(|e| e.score(query).map(|s| (s, e)))
            .collect();
        hits.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.list_order(b.1)));
        hits.into_iter()
            .take(limit)
            .map(|(score, e)| Hit { id: e.id, score })
            .collect()
    }

    /// Approximate heap memory held by the index, in bytes.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.entries.capacity() * std::mem::size_of::<Entry>()
            + self.entries.iter().map(Entry::heap_bytes).sum::<usize>()
    }
}
