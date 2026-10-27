//! What lists and search results show about an item, kept in memory without the full content.

use cy_model::{Hlc, Item, ItemId, RepKind, Representation, Sensitivity, SourceApp};

/// Characters of text kept for previews.
pub const PREVIEW_CHARS: usize = 300;

/// An item as lists show it. Holds a short preview, never the full content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemSummary {
    /// The item.
    pub id: ItemId,
    /// Kind of its primary representation.
    pub kind: RepKind,
    /// The first [`PREVIEW_CHARS`] characters of its text; empty for images.
    pub preview: String,
    /// Whether it is pinned.
    pub pinned: bool,
    /// Its last use.
    pub last_used: Hlc,
    /// When it was first copied (milliseconds since the Unix epoch).
    pub first_copied_ms: u64,
    /// The app it was copied from.
    pub source: Option<SourceApp>,
    /// Width and height of an image item, when known.
    pub image_size: Option<(u32, u32)>,
    /// Bytes of content.
    pub size_bytes: u64,
    /// When a sensitive item expires.
    pub sensitive_until_ms: Option<u64>,
    /// Whether text was cut to the limit.
    pub truncated: bool,
}

impl ItemSummary {
    /// The summary of `item`.
    #[must_use]
    pub fn of(item: &Item) -> Self {
        let preview = item
            .primary()
            .map(|rep| rep.search_text().chars().take(PREVIEW_CHARS).collect())
            .unwrap_or_default();
        let image_size = item.reps.iter().find_map(|rep| match rep {
            Representation::Image { width, height, .. } => Some((*width, *height)),
            _ => None,
        });
        Self {
            id: item.id,
            kind: item.kind(),
            preview,
            pinned: item.pinned,
            last_used: item.last_used,
            first_copied_ms: item.first_copied_ms,
            source: item.source.clone(),
            image_size,
            size_bytes: item.size() as u64,
            sensitive_until_ms: match item.sensitivity {
                Sensitivity::Sensitive { expires_at_ms, .. } => Some(expires_at_ms),
                Sensitivity::Normal => None,
            },
            truncated: item.truncated,
        }
    }

    /// Approximate heap memory held by this summary.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.preview.capacity()
            + self
                .source
                .as_ref()
                .map_or(0, |s| s.bundle_id.capacity() + s.name.capacity())
    }
}
