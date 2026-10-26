//! What a platform shell reads from the clipboard and hands to the core.

use crate::{SecretBytes, SourceApp};

/// One type on the clipboard.
///
/// `type_id` is the platform's own name (`public.utf8-plain-text`, `CF_UNICODETEXT`,
/// `text/plain;charset=utf-8`, ...). Text types are passed as UTF-8, whatever the platform's own
/// encoding. When a type is larger than the shell's read limit, `data` is `None` and `size` holds
/// its size.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClipEntry {
    /// Platform type name.
    pub type_id: String,
    /// The data, unless it was too large to read or the type is only a marker.
    pub data: Option<SecretBytes>,
    /// Size of the data in bytes.
    pub size: u64,
}

impl ClipEntry {
    /// A type with its data.
    #[must_use]
    pub fn with_data(type_id: impl Into<String>, data: Vec<u8>) -> Self {
        let size = data.len() as u64;
        Self {
            type_id: type_id.into(),
            data: Some(SecretBytes::new(data)),
            size,
        }
    }

    /// A type present without data (markers such as `org.nspasteboard.ConcealedType`).
    #[must_use]
    pub fn marker(type_id: impl Into<String>) -> Self {
        Self {
            type_id: type_id.into(),
            data: None,
            size: 0,
        }
    }

    /// A type too large for the shell to read.
    #[must_use]
    pub fn oversized(type_id: impl Into<String>, size: u64) -> Self {
        Self {
            type_id: type_id.into(),
            data: None,
            size,
        }
    }
}

/// The clipboard's content at one change.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ClipboardSnapshot {
    /// Every type on the clipboard.
    pub entries: Vec<ClipEntry>,
    /// The application in front when the change was seen.
    pub front_app: Option<SourceApp>,
    /// When the change was seen (milliseconds since the Unix epoch).
    pub captured_at_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_set_size_and_data() {
        let text = ClipEntry::with_data("public.utf8-plain-text", b"demo".to_vec());
        let marker = ClipEntry::marker("org.nspasteboard.ConcealedType");
        let big = ClipEntry::oversized("public.png", 10_000_000);

        assert_eq!(text.size, 4);
        assert_eq!(
            text.data.as_ref().map(SecretBytes::expose),
            Some(&b"demo"[..])
        );
        assert_eq!((marker.data, marker.size), (None, 0));
        assert_eq!((big.data, big.size), (None, 10_000_000));
    }
}
