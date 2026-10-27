//! Content fingerprints for deduplication.
//!
//! A fingerprint is a keyed hash of an item's normalized primary representation. The key comes
//! from the data key, so stored fingerprints cannot confirm whether some known text was copied.

use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{RepKind, Representation};

/// Length of a fingerprint key and of a fingerprint.
pub const FINGERPRINT_LEN: usize = 32;

/// Domain tags so different kinds of content never share a fingerprint.
const TAG_TEXT: u8 = 1;
const TAG_URL: u8 = 2;
const TAG_IMAGE: u8 = 3;
const TAG_FILES: u8 = 4;

/// The key for content fingerprints. Zeroed on drop.
#[derive(Clone)]
pub struct FingerprintKey(Zeroizing<[u8; FINGERPRINT_LEN]>);

impl FingerprintKey {
    /// A key from its bytes (derived from the data key by storage).
    #[must_use]
    pub fn from_bytes(bytes: [u8; FINGERPRINT_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }
}

impl fmt::Debug for FingerprintKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FingerprintKey(..)")
    }
}

/// A keyed hash identifying an item's content.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Fingerprint(pub [u8; FINGERPRINT_LEN]);

impl Fingerprint {
    /// The fingerprint of `reps`, computed from their primary representation; `None` when there
    /// is none.
    #[must_use]
    pub fn of(key: &FingerprintKey, reps: &[Representation]) -> Option<Self> {
        let primary = primary_rep(reps)?;
        let mut hasher = blake3::Hasher::new_keyed(&key.0);
        match primary {
            Representation::PlainText(text) | Representation::RichText { plain: text, .. } => {
                hash_part(
                    &mut hasher,
                    TAG_TEXT,
                    normalize_text(text.expose()).as_bytes(),
                );
            }
            Representation::Url { url, .. } => {
                hash_part(&mut hasher, TAG_URL, url.expose().trim().as_bytes());
            }
            Representation::Image { format, bytes, .. } => {
                hash_part(&mut hasher, TAG_IMAGE, format!("{format:?}").as_bytes());
                hash_part(&mut hasher, TAG_IMAGE, bytes.expose());
            }
            Representation::FileRef { names } => {
                for name in names {
                    hash_part(&mut hasher, TAG_FILES, name.as_bytes());
                }
            }
        }
        Some(Self(*hasher.finalize().as_bytes()))
    }
}

/// Hashes a tagged, length-prefixed part, so parts can never run into each other.
fn hash_part(hasher: &mut blake3::Hasher, tag: u8, bytes: &[u8]) {
    hasher.update(&[tag]);
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

/// The representation used for previews, search and deduplication: a link first, then copied
/// files (their text form is only their names), then text, then an image.
#[must_use]
pub fn primary_rep(reps: &[Representation]) -> Option<&Representation> {
    const ORDER: [&[RepKind]; 4] = [
        &[RepKind::Url],
        &[RepKind::FileRef],
        &[RepKind::Text, RepKind::RichText],
        &[RepKind::Image],
    ];
    ORDER
        .iter()
        .find_map(|kinds| reps.iter().find(|rep| kinds.contains(&rep.kind())))
}

/// Normalizes text for fingerprints only (stored content is never changed): line endings become
/// `\n` and trailing whitespace is removed. Leading whitespace is kept, because indented code is
/// different content.
#[must_use]
pub fn normalize_text(text: &str) -> Zeroizing<String> {
    Zeroizing::new(
        text.replace("\r\n", "\n")
            .replace('\r', "\n")
            .trim_end()
            .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImageFormat, SecretBytes, SecretString};
    use proptest::prelude::*;

    fn key(byte: u8) -> FingerprintKey {
        FingerprintKey::from_bytes([byte; FINGERPRINT_LEN])
    }

    fn text(value: &str) -> Vec<Representation> {
        vec![Representation::PlainText(SecretString::from(value))]
    }

    #[test]
    fn same_text_with_different_line_endings_has_one_fingerprint() {
        let unix = Fingerprint::of(&key(1), &text("line one\nline two\n"));
        let windows = Fingerprint::of(&key(1), &text("line one\r\nline two  \r\n"));

        assert_eq!(unix, windows);
    }

    #[test]
    fn leading_whitespace_is_significant() {
        assert_ne!(
            Fingerprint::of(&key(1), &text("  indented")),
            Fingerprint::of(&key(1), &text("indented"))
        );
    }

    #[test]
    fn rich_and_plain_copies_of_the_same_text_match() {
        let rich = vec![Representation::RichText {
            rtf: None,
            html: Some(SecretString::from("<p>Hello from Supercy</p>")),
            plain: SecretString::from("Hello from Supercy"),
        }];

        assert_eq!(
            Fingerprint::of(&key(1), &rich),
            Fingerprint::of(&key(1), &text("Hello from Supercy"))
        );
    }

    #[test]
    fn different_keys_give_different_fingerprints() {
        assert_ne!(
            Fingerprint::of(&key(1), &text("demo")),
            Fingerprint::of(&key(2), &text("demo"))
        );
    }

    #[test]
    fn a_link_and_the_same_text_differ() {
        let url = vec![Representation::Url {
            url: SecretString::from("https://example.com"),
            title: None,
        }];

        assert_ne!(
            Fingerprint::of(&key(1), &url),
            Fingerprint::of(&key(1), &text("https://example.com"))
        );
    }

    #[test]
    fn images_and_files_have_fingerprints_and_empty_input_has_none() {
        let png = |bytes: &[u8]| {
            vec![Representation::Image {
                format: ImageFormat::Png,
                width: 1,
                height: 1,
                bytes: SecretBytes::from(bytes),
            }]
        };
        let files = vec![Representation::FileRef {
            names: vec!["ab".into(), "c".into()],
        }];
        let other_files = vec![Representation::FileRef {
            names: vec!["a".into(), "bc".into()],
        }];

        assert_ne!(
            Fingerprint::of(&key(1), &png(b"one")),
            Fingerprint::of(&key(1), &png(b"two"))
        );
        assert_ne!(
            Fingerprint::of(&key(1), &files),
            Fingerprint::of(&key(1), &other_files)
        );
        assert_eq!(Fingerprint::of(&key(1), &[]), None);
        assert_eq!(format!("{:?}", key(9)), "FingerprintKey(..)");
    }

    #[test]
    fn primary_prefers_link_then_text_then_image() {
        let image = Representation::Image {
            format: ImageFormat::Tiff,
            width: 0,
            height: 0,
            bytes: SecretBytes::default(),
        };
        let plain = Representation::PlainText(SecretString::from("https://example.com"));
        let url = Representation::Url {
            url: SecretString::from("https://example.com"),
            title: None,
        };

        let reps = [image.clone(), plain.clone(), url.clone()];

        assert_eq!(primary_rep(&reps), Some(&url));
        assert_eq!(primary_rep(&reps[..2]), Some(&plain));
        assert_eq!(primary_rep(&reps[..1]), Some(&image));
    }

    proptest! {
        #[test]
        fn normalization_is_idempotent(input in ".*") {
            let once = normalize_text(&input);
            let twice = normalize_text(&once);
            prop_assert_eq!(once.as_str(), twice.as_str());
        }

        #[test]
        fn same_content_same_fingerprint(input in ".*") {
            prop_assert_eq!(Fingerprint::of(&key(3), &text(&input)), Fingerprint::of(&key(3), &text(&input)));
        }
    }
}
