//! History items.

use serde::{Deserialize, Serialize};

use crate::{DeviceTag, Fingerprint, Hlc, ItemId, RepKind, Representation, primary_rep};

/// The application an item was copied from. Kept on this device only; never synced.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SourceApp {
    /// Platform identifier, such as `com.example.editor`.
    pub bundle_id: String,
    /// Display name, such as `Demo Editor`.
    pub name: String,
}

/// Why content was judged sensitive.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SensitiveReason {
    /// A PEM private key.
    PrivateKey,
    /// An access token or API key of a well-known form.
    AccessToken,
    /// A payment card number (passes the Luhn check).
    CardNumber,
    /// A single random-looking word, likely a password.
    HighEntropy,
}

/// Whether an item is sensitive. Sensitive items are deleted when they expire and never synced.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sensitivity {
    /// Ordinary content.
    #[default]
    Normal,
    /// Sensitive content, deleted at `expires_at_ms` unless pinned.
    Sensitive {
        /// Expiry time in milliseconds since the Unix epoch.
        expires_at_ms: u64,
        /// Why it is sensitive.
        reason: SensitiveReason,
    },
}

/// One entry of the clipboard history.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Item {
    /// Identifier, unique across devices.
    pub id: ItemId,
    /// Keyed hash of the normalized primary representation.
    pub fingerprint: Fingerprint,
    /// The forms of the content, in paste-preference order. Never empty.
    pub reps: Vec<Representation>,
    /// Where it was copied, on this device only.
    pub source: Option<SourceApp>,
    /// The device where it was copied.
    pub origin: DeviceTag,
    /// When it was first copied (milliseconds since the Unix epoch).
    pub first_copied_ms: u64,
    /// The last copy or paste; the list is ordered by it.
    pub last_used: Hlc,
    /// Pinned items stay on top and are never removed by limits or expiry.
    pub pinned: bool,
    /// When `pinned` last changed.
    pub pinned_at: Option<Hlc>,
    /// Whether the content is sensitive.
    pub sensitivity: Sensitivity,
    /// Whether text was cut to the size limit.
    pub truncated: bool,
    /// How many times this content was copied (at least 1).
    pub copy_count: u32,
    /// The letter key of a pinned item (`a`–`z`), if it has one.
    pub pin_key: Option<char>,
}

impl Item {
    /// The representation used for previews and search.
    #[must_use]
    pub fn primary(&self) -> Option<&Representation> {
        primary_rep(&self.reps)
    }

    /// The kind of the primary representation.
    #[must_use]
    pub fn kind(&self) -> RepKind {
        self.primary().map_or(RepKind::Text, Representation::kind)
    }

    /// Bytes of content held by all representations.
    #[must_use]
    pub fn size(&self) -> usize {
        self.reps.iter().map(Representation::size).sum()
    }

    /// A copy used again at `at` (copied again or pasted).
    #[must_use]
    pub fn touched(&self, at: Hlc) -> Self {
        Self {
            last_used: at,
            ..self.clone()
        }
    }

    /// A copy with `pinned` set at `at`. Unpinning drops the letter key.
    #[must_use]
    pub fn with_pinned(&self, pinned: bool, at: Hlc) -> Self {
        Self {
            pinned,
            pinned_at: Some(at),
            pin_key: if pinned { self.pin_key } else { None },
            ..self.clone()
        }
    }

    /// A copy with the letter key `key`.
    #[must_use]
    pub fn with_pin_key(&self, key: Option<char>) -> Self {
        Self {
            pin_key: key,
            ..self.clone()
        }
    }

    /// The same content copied again at `at`: counted once more, and holding the kinds from
    /// `reps` it was missing.
    #[must_use]
    pub fn merged_with(&self, reps: &[Representation], at: Hlc) -> Self {
        let missing = reps
            .iter()
            .filter(|rep| self.reps.iter().all(|own| own.kind() != rep.kind()));
        Self {
            reps: self.reps.iter().chain(missing).cloned().collect(),
            last_used: at,
            copy_count: self.copy_count.saturating_add(1),
            ..self.clone()
        }
    }

    /// Whether a sensitive, unpinned item has expired at `now_ms`.
    #[must_use]
    pub fn is_expired(&self, now_ms: u64) -> bool {
        match self.sensitivity {
            Sensitivity::Sensitive { expires_at_ms, .. } => !self.pinned && expires_at_ms <= now_ms,
            Sensitivity::Normal => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FingerprintKey, SecretString};

    const DEVICE: DeviceTag = DeviceTag([2; 8]);

    fn demo_item(reps: Vec<Representation>) -> Item {
        let key = FingerprintKey::from_bytes([1; 32]);
        Item {
            id: ItemId::from_bytes([5; 16]),
            fingerprint: Fingerprint::of(&key, &reps).unwrap(),
            reps,
            source: Some(SourceApp {
                bundle_id: "com.example.editor".into(),
                name: "Demo Editor".into(),
            }),
            origin: DEVICE,
            first_copied_ms: 1_000,
            last_used: Hlc::new(1_000, 0, DEVICE),
            pinned: false,
            pinned_at: None,
            sensitivity: Sensitivity::Normal,
            truncated: false,
            copy_count: 1,
            pin_key: None,
        }
    }

    fn plain(value: &str) -> Representation {
        Representation::PlainText(SecretString::from(value))
    }

    #[test]
    fn touched_and_pinned_return_new_values_and_keep_the_original() {
        let item = demo_item(vec![plain("demo")]);
        let later = Hlc::new(2_000, 0, DEVICE);

        let touched = item.touched(later);
        let pinned = item.with_pinned(true, later);

        assert_eq!(item.last_used.wall_ms(), 1_000);
        assert!(!item.pinned);
        assert_eq!(touched.last_used, later);
        assert!(pinned.pinned);
        assert_eq!(pinned.pinned_at, Some(later));
    }

    #[test]
    fn merged_with_adds_only_missing_kinds() {
        let item = demo_item(vec![plain("demo")]);
        let html = Representation::RichText {
            rtf: None,
            html: None,
            plain: SecretString::from("demo"),
        };
        let later = Hlc::new(3_000, 0, DEVICE);

        let merged = item.merged_with(&[plain("other"), html.clone()], later);

        assert_eq!(merged.reps, vec![plain("demo"), html]);
        assert_eq!(merged.last_used, later);
        assert_eq!(merged.copy_count, 2);
    }

    #[test]
    fn unpinning_drops_the_letter_key() {
        let at = Hlc::new(2_000, 0, DEVICE);
        let pinned = demo_item(vec![plain("demo")])
            .with_pinned(true, at)
            .with_pin_key(Some('a'));

        assert_eq!(pinned.pin_key, Some('a'));
        assert_eq!(pinned.with_pinned(true, at).pin_key, Some('a'));
        assert_eq!(pinned.with_pinned(false, at).pin_key, None);
    }

    #[test]
    fn kind_size_and_primary() {
        let item = demo_item(vec![plain("Hello from Supercy")]);
        let empty = Item {
            reps: Vec::new(),
            ..item.clone()
        };

        assert_eq!(item.kind(), RepKind::Text);
        assert_eq!(item.size(), 18);
        assert_eq!(empty.kind(), RepKind::Text);
        assert_eq!(empty.primary(), None);
    }

    #[test]
    fn only_unpinned_sensitive_items_expire() {
        let sensitive = Sensitivity::Sensitive {
            expires_at_ms: 5_000,
            reason: SensitiveReason::HighEntropy,
        };
        let item = Item {
            sensitivity: sensitive,
            ..demo_item(vec![plain("demo")])
        };
        let pinned = item.with_pinned(true, Hlc::new(2_000, 0, DEVICE));

        assert!(!item.is_expired(4_999));
        assert!(item.is_expired(5_000));
        assert!(!pinned.is_expired(9_000));
        assert!(!demo_item(vec![plain("demo")]).is_expired(u64::MAX));
    }
}
