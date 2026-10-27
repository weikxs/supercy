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
    /// The first [`PREVIEW_CHARS`] characters of its text (file names for copied files); empty for
    /// images.
    pub preview: String,
    /// Whether it is pinned.
    pub pinned: bool,
    /// The letter key of a pinned item.
    pub pin_key: Option<char>,
    /// Its last use.
    pub last_used: Hlc,
    /// When it was first copied (milliseconds since the Unix epoch).
    pub first_copied_ms: u64,
    /// How many times it was copied.
    pub copy_count: u32,
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
    /// Character ranges of `preview` that matched the search, for highlighting (search only).
    pub matches: Vec<(usize, usize)>,
}

impl ItemSummary {
    /// The summary of `item`.
    #[must_use]
    pub fn of(item: &Item) -> Self {
        let preview = match item.primary() {
            Some(Representation::FileRef { names }) => names
                .iter()
                .map(|url| file_name(url))
                .collect::<Vec<_>>()
                .join("\n"),
            Some(rep) => rep.search_text().chars().take(PREVIEW_CHARS).collect(),
            None => String::new(),
        };
        let image_size = item.reps.iter().find_map(|rep| match rep {
            Representation::Image { width, height, .. } => Some((*width, *height)),
            _ => None,
        });
        Self {
            id: item.id,
            kind: item.kind(),
            preview,
            pinned: item.pinned,
            pin_key: item.pin_key,
            last_used: item.last_used,
            first_copied_ms: item.first_copied_ms,
            copy_count: item.copy_count,
            source: item.source.clone(),
            image_size,
            size_bytes: item.size() as u64,
            sensitive_until_ms: match item.sensitivity {
                Sensitivity::Sensitive { expires_at_ms, .. } => Some(expires_at_ms),
                Sensitivity::Normal => None,
            },
            truncated: item.truncated,
            matches: Vec::new(),
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

/// The last path segment of a `file://` URL, percent-decoded.
fn file_name(url: &str) -> String {
    let path = url
        .strip_prefix("file://")
        .unwrap_or(url)
        .trim_end_matches('/');
    let last = path.rsplit('/').next().unwrap_or(path);
    percent_decode(last)
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_decoded_from_urls() {
        assert_eq!(
            file_name("file:///tmp/supercy-test/Demo%20File.txt"),
            "Demo File.txt"
        );
        assert_eq!(file_name("file:///tmp/supercy-test/folder/"), "folder");
        assert_eq!(file_name("file:///tmp/%E7%A4%BA%E4%BE%8B.txt"), "示例.txt");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
