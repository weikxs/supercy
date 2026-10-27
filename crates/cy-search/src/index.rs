//! The search index.

use std::borrow::Cow;
use std::cmp::Ordering;

use cy_model::{Hlc, Item, ItemId, RepKind, Representation};

use crate::fold::fold;
use crate::matcher::{MatchMode, Query, char_mask, regex_score, score};
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
    /// When it was pinned (or moved among the pinned items): pinned items are listed in this
    /// order.
    pub pinned_at: Option<Hlc>,
    /// Its last use.
    pub last_used: Hlc,
    /// When it was first copied.
    pub first_copied_ms: u64,
    /// How many times it was copied.
    pub copy_count: u32,
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
            pinned_at: item.pinned_at,
            last_used: item.last_used,
            first_copied_ms: item.first_copied_ms,
            copy_count: item.copy_count,
            text: item
                .primary()
                .map_or(Cow::Borrowed(""), Representation::search_text),
        }
    }
}

/// How unpinned items are ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Most recently copied or pasted first.
    #[default]
    LastUsed,
    /// Most recently first copied first.
    FirstCopied,
    /// Most often copied first.
    CopyCount,
}

/// Where pinned items go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PinPosition {
    /// Above the other items.
    #[default]
    Top,
    /// Below the other items.
    Bottom,
}

/// The list order: pinned items as a group at the top or bottom, each group sorted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ListOrder {
    /// How items are sorted within their group.
    pub sort: SortOrder,
    /// Where the pinned group goes.
    pub pins: PinPosition,
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
    pinned_at: Option<Hlc>,
    last_used: Hlc,
    first_copied_ms: u64,
    copy_count: u32,
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
            pinned_at: doc.pinned_at,
            last_used: doc.last_used,
            first_copied_ms: doc.first_copied_ms,
            copy_count: doc.copy_count,
            text: text.into_boxed_str(),
            pinyin,
            mask,
        }
    }

    fn haystacks(&self, with_pinyin: bool) -> impl Iterator<Item = &str> {
        let pinyin = self.pinyin.iter().filter(move |_| with_pinyin);
        std::iter::once(&*self.text)
            .chain(pinyin.clone().map(|p| &*p.primary))
            .chain(pinyin.filter_map(|p| p.alternate.as_deref()))
    }

    /// Every term matches the text (or, for subsequence matching, a pinyin form).
    fn term_score(&self, query: &Query, subsequence: bool) -> Option<u32> {
        query.terms().iter().try_fold(0u32, |total, term| {
            let best = self
                .haystacks(subsequence)
                .filter_map(|hay| score(hay, term, subsequence))
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

    fn compare(&self, other: &Self, order: ListOrder) -> Ordering {
        let group = match order.pins {
            PinPosition::Top => other.pinned.cmp(&self.pinned),
            PinPosition::Bottom => self.pinned.cmp(&other.pinned),
        };
        let sorted = if self.pinned && other.pinned {
            // Pinned items keep their place (the order they were pinned or arranged in): using
            // one never moves it.
            self.pinned_at.cmp(&other.pinned_at)
        } else {
            match order.sort {
                SortOrder::LastUsed => Ordering::Equal,
                SortOrder::FirstCopied => other.first_copied_ms.cmp(&self.first_copied_ms),
                SortOrder::CopyCount => other.copy_count.cmp(&self.copy_count),
            }
        };
        group
            .then(sorted)
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

    /// Adds `doc`, which must not be in the index yet (building an index from storage, where
    /// ids are unique); use [`SearchIndex::upsert`] otherwise.
    pub fn push(&mut self, doc: &IndexDoc<'_>) {
        self.entries.push(Entry::new(doc));
    }

    /// Sets when item `id` was pinned (its place among the pinned items); returns whether it is
    /// in the index.
    pub fn set_pinned_at(&mut self, id: ItemId, at: Hlc) -> bool {
        self.entries
            .iter_mut()
            .find(|e| e.id == id)
            .map(|e| e.pinned_at = Some(at))
            .is_some()
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

    /// Items in list order, optionally of one kind.
    #[must_use]
    pub fn list(
        &self,
        kind: Option<RepKind>,
        order: ListOrder,
        offset: usize,
        limit: usize,
    ) -> Vec<ItemId> {
        let mut entries: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|e| kind_matches(kind, e.kind))
            .collect();
        entries.sort_unstable_by(|a, b| a.compare(b, order));
        entries
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|e| e.id)
            .collect()
    }

    /// The best `limit` matches for `query`: by score, then in list order. An empty query
    /// returns the list order.
    #[must_use]
    pub fn search(
        &self,
        query: &Query,
        kind: Option<RepKind>,
        order: ListOrder,
        limit: usize,
    ) -> Vec<Hit> {
        if query.is_empty() {
            return self
                .list(kind, order, 0, limit)
                .into_iter()
                .map(|id| Hit { id, score: 0 })
                .collect();
        }
        let mut hits = match query.mode() {
            MatchMode::Fuzzy => self.scored(query, kind, |e| e.term_score(query, true)),
            MatchMode::Exact => self.scored(query, kind, |e| e.term_score(query, false)),
            MatchMode::Regex => self.regex_hits(query, kind),
            MatchMode::Mixed => {
                let exact = self.scored(query, kind, |e| e.term_score(query, false));
                if exact.is_empty() {
                    let regex = self.regex_hits(query, kind);
                    if regex.is_empty() {
                        self.scored(query, kind, |e| e.term_score(query, true))
                    } else {
                        regex
                    }
                } else {
                    exact
                }
            }
        };
        hits.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.compare(b.1, order)));
        hits.into_iter()
            .take(limit)
            .map(|(score, e)| Hit { id: e.id, score })
            .collect()
    }

    fn scored<'a>(
        &'a self,
        query: &Query,
        kind: Option<RepKind>,
        score_of: impl Fn(&Entry) -> Option<u32>,
    ) -> Vec<(u32, &'a Entry)> {
        let masks: Vec<u64> = query
            .terms()
            .iter()
            .map(|t| char_mask(t.iter().copied()))
            .collect();
        self.entries
            .iter()
            .filter(|e| kind_matches(kind, e.kind) && masks.iter().all(|m| e.mask & m == *m))
            .filter_map(|e| score_of(e).map(|s| (s, e)))
            .collect()
    }

    fn regex_hits(&self, query: &Query, kind: Option<RepKind>) -> Vec<(u32, &Entry)> {
        let Some(regex) = query.regex() else {
            return Vec::new();
        };
        self.entries
            .iter()
            .filter(|e| kind_matches(kind, e.kind))
            .filter_map(|e| regex_score(&e.text, regex).map(|s| (s, e)))
            .collect()
    }

    /// Approximate heap memory held by the index, in bytes.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.entries.capacity() * std::mem::size_of::<Entry>()
            + self.entries.iter().map(Entry::heap_bytes).sum::<usize>()
    }
}
