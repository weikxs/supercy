//! Representations: the forms one copied item can take.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::{SecretBytes, SecretString};

/// The kind of a [`Representation`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum RepKind {
    /// Plain text.
    Text,
    /// Rich text (RTF or HTML) with its plain-text form.
    RichText,
    /// A link.
    Url,
    /// An image.
    Image,
    /// References to copied files (names only, never contents).
    FileRef,
}

impl RepKind {
    /// Stable numeric code used by storage.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Text => 1,
            Self::RichText => 2,
            Self::Url => 3,
            Self::Image => 4,
            Self::FileRef => 5,
        }
    }

    /// The kind for a storage code.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Text),
            2 => Some(Self::RichText),
            3 => Some(Self::Url),
            4 => Some(Self::Image),
            5 => Some(Self::FileRef),
            _ => None,
        }
    }
}

/// Image encodings Supercy keeps.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ImageFormat {
    /// PNG.
    Png,
    /// TIFF (the macOS clipboard's native image type).
    Tiff,
    /// JPEG.
    Jpeg,
    /// GIF.
    Gif,
}

/// One form of a copied item.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Representation {
    /// Plain text.
    PlainText(SecretString),
    /// Rich text; `plain` is what search and plain-text paste use.
    RichText {
        /// RTF source, if the clipboard had it.
        rtf: Option<SecretBytes>,
        /// HTML source, if the clipboard had it.
        html: Option<SecretString>,
        /// The plain-text form.
        plain: SecretString,
    },
    /// A link.
    Url {
        /// The URL.
        url: SecretString,
        /// A title, if the source app supplied one.
        title: Option<SecretString>,
    },
    /// An image; `width` and `height` are 0 when the format's header was not read.
    Image {
        /// Encoding.
        format: ImageFormat,
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// The encoded image.
        bytes: SecretBytes,
    },
    /// Copied files, by name only.
    FileRef {
        /// File names.
        names: Vec<String>,
    },
}

impl Representation {
    /// The kind of this representation.
    #[must_use]
    pub const fn kind(&self) -> RepKind {
        match self {
            Self::PlainText(_) => RepKind::Text,
            Self::RichText { .. } => RepKind::RichText,
            Self::Url { .. } => RepKind::Url,
            Self::Image { .. } => RepKind::Image,
            Self::FileRef { .. } => RepKind::FileRef,
        }
    }

    /// Bytes of content held, used for size limits.
    #[must_use]
    pub fn size(&self) -> usize {
        match self {
            Self::PlainText(text) => text.len(),
            Self::RichText { rtf, html, plain } => {
                plain.len()
                    + rtf.as_ref().map_or(0, SecretBytes::len)
                    + html.as_ref().map_or(0, SecretString::len)
            }
            Self::Url { url, title } => url.len() + title.as_ref().map_or(0, SecretString::len),
            Self::Image { bytes, .. } => bytes.len(),
            Self::FileRef { names } => names.iter().map(String::len).sum(),
        }
    }

    /// The text that search and previews use; empty for images.
    #[must_use]
    pub fn search_text(&self) -> Cow<'_, str> {
        match self {
            Self::PlainText(text) | Self::RichText { plain: text, .. } => {
                Cow::Borrowed(text.expose())
            }
            Self::Url { url, .. } => Cow::Borrowed(url.expose()),
            Self::Image { .. } => Cow::Borrowed(""),
            Self::FileRef { names } => Cow::Owned(names.join("\n")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_kinds() -> [RepKind; 5] {
        [
            RepKind::Text,
            RepKind::RichText,
            RepKind::Url,
            RepKind::Image,
            RepKind::FileRef,
        ]
    }

    #[test]
    fn kind_codes_round_trip_and_unknown_codes_are_rejected() {
        for kind in all_kinds() {
            assert_eq!(RepKind::from_code(kind.code()), Some(kind));
        }
        assert_eq!(RepKind::from_code(0), None);
        assert_eq!(RepKind::from_code(99), None);
    }

    #[test]
    fn size_counts_every_part() {
        let rich = Representation::RichText {
            rtf: Some(SecretBytes::from(&b"{\\rtf1 demo}"[..])),
            html: Some(SecretString::from("<b>demo</b>")),
            plain: SecretString::from("demo"),
        };
        let url = Representation::Url {
            url: SecretString::from("https://example.com"),
            title: Some(SecretString::from("Example")),
        };
        let files = Representation::FileRef {
            names: vec!["a.txt".into(), "bb.txt".into()],
        };

        assert_eq!(rich.size(), 12 + 11 + 4);
        assert_eq!(url.size(), 19 + 7);
        assert_eq!(files.size(), 11);
    }

    #[test]
    fn search_text_per_kind() {
        let image = Representation::Image {
            format: ImageFormat::Png,
            width: 1,
            height: 1,
            bytes: SecretBytes::default(),
        };
        let files = Representation::FileRef {
            names: vec!["a.txt".into(), "b.txt".into()],
        };
        let text = Representation::PlainText(SecretString::from("Hello from Supercy"));

        assert_eq!(image.search_text(), "");
        assert_eq!(files.search_text(), "a.txt\nb.txt");
        assert_eq!(text.search_text(), "Hello from Supercy");
        assert_eq!(image.kind(), RepKind::Image);
        assert_eq!(files.kind(), RepKind::FileRef);
    }
}
