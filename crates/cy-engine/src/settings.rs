//! User settings: validated, stored sealed in the history database.

use std::collections::BTreeSet;

use cy_filter::FilterPolicy;
use cy_store::{Limits, MAX_ITEMS, MIN_ITEMS};
use serde::{Deserialize, Serialize};

const KIB: u32 = 1024;
const MIB: u32 = 1024 * KIB;
const MIB_U64: u64 = 1024 * 1024;

/// Default number of unpinned items kept.
pub const DEFAULT_MAX_ITEMS: u32 = 500;
/// Default text limit: 1 MiB.
pub const DEFAULT_MAX_TEXT_BYTES: u32 = MIB;
/// Default limit per image: 5 MiB.
pub const DEFAULT_MAX_IMAGE_BYTES: u32 = 5 * MIB;
/// Default limit for all unpinned images together: 64 MiB.
pub const DEFAULT_MAX_IMAGE_TOTAL_BYTES: u64 = 64 * MIB_U64;
/// Default lifetime of sensitive items: 2 minutes.
pub const DEFAULT_SENSITIVE_TTL_MS: u64 = 2 * 60 * 1000;

/// Password managers ignored out of the box, by bundle identifier. Most of them also mark
/// their copies as concealed; this list covers versions that do not.
pub const DEFAULT_IGNORED_APPS: &[&str] = &[
    "com.1password.1password",
    "com.agilebits.onepassword7",
    "com.bitwarden.desktop",
    "org.keepassxc.keepassxc",
    "com.apple.keychainaccess",
    "com.apple.Passwords",
];

const TEXT_RANGE: std::ops::RangeInclusive<u32> = KIB..=2 * MIB;
const IMAGE_RANGE: std::ops::RangeInclusive<u32> = 64 * KIB..=8 * MIB;
const IMAGE_TOTAL_RANGE: std::ops::RangeInclusive<u64> = MIB_U64..=1024 * MIB_U64;
const TTL_RANGE: std::ops::RangeInclusive<u64> = 10_000..=24 * 60 * 60 * 1000;
const MAX_IGNORED_APPS: usize = 256;
const MAX_BUNDLE_ID_LEN: usize = 255;

/// A setting outside its allowed range.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("the setting `{field}` is out of range ({allowed})")]
pub struct SettingsError {
    /// The setting's name.
    pub field: &'static str,
    /// The allowed values, for the message.
    pub allowed: &'static str,
}

/// What Supercy keeps and for how long.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Unpinned items kept (50–10 000).
    pub max_items: u32,
    /// Longest text kept, in bytes (1 KiB–2 MiB); longer text is cut.
    pub max_text_bytes: u32,
    /// Largest image kept, in bytes (64 KiB–8 MiB).
    pub max_image_bytes: u32,
    /// All unpinned images together, in bytes (1 MiB–1 GiB).
    pub max_image_total_bytes: u64,
    /// Whether sensitive content is detected and expired.
    pub detect_sensitive: bool,
    /// Lifetime of sensitive items in milliseconds (10 s–24 h).
    pub sensitive_ttl_ms: u64,
    /// Whether content marked auto-generated is kept.
    pub keep_auto_generated: bool,
    /// Apps whose copies are never kept, by bundle identifier.
    pub ignored_apps: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_items: DEFAULT_MAX_ITEMS,
            max_text_bytes: DEFAULT_MAX_TEXT_BYTES,
            max_image_bytes: DEFAULT_MAX_IMAGE_BYTES,
            max_image_total_bytes: DEFAULT_MAX_IMAGE_TOTAL_BYTES,
            detect_sensitive: true,
            sensitive_ttl_ms: DEFAULT_SENSITIVE_TTL_MS,
            keep_auto_generated: false,
            ignored_apps: DEFAULT_IGNORED_APPS
                .iter()
                .map(|app| (*app).to_owned())
                .collect(),
        }
    }
}

fn check(ok: bool, field: &'static str, allowed: &'static str) -> Result<(), SettingsError> {
    if ok {
        Ok(())
    } else {
        Err(SettingsError { field, allowed })
    }
}

fn valid_bundle_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= MAX_BUNDLE_ID_LEN && !id.chars().any(char::is_control)
}

impl Settings {
    /// Checks every setting against its range.
    ///
    /// # Errors
    ///
    /// The first setting out of range.
    pub fn validate(&self) -> Result<(), SettingsError> {
        let items =
            usize::try_from(self.max_items).is_ok_and(|n| (MIN_ITEMS..=MAX_ITEMS).contains(&n));
        check(items, "max_items", "50 to 10000")?;
        check(
            TEXT_RANGE.contains(&self.max_text_bytes),
            "max_text_bytes",
            "1 KiB to 2 MiB",
        )?;
        check(
            IMAGE_RANGE.contains(&self.max_image_bytes),
            "max_image_bytes",
            "64 KiB to 8 MiB",
        )?;
        check(
            IMAGE_TOTAL_RANGE.contains(&self.max_image_total_bytes),
            "max_image_total_bytes",
            "1 MiB to 1 GiB",
        )?;
        check(
            TTL_RANGE.contains(&self.sensitive_ttl_ms),
            "sensitive_ttl_ms",
            "10 seconds to 24 hours",
        )?;
        let apps_ok = self.ignored_apps.len() <= MAX_IGNORED_APPS
            && self.ignored_apps.iter().all(|a| valid_bundle_id(a));
        check(
            apps_ok,
            "ignored_apps",
            "at most 256 bundle identifiers of 1 to 255 characters",
        )
    }

    /// The capture filter's policy for these settings.
    #[must_use]
    pub fn filter_policy(&self) -> FilterPolicy {
        FilterPolicy {
            ignored_apps: self
                .ignored_apps
                .iter()
                .map(|app| app.trim().to_owned())
                .collect::<BTreeSet<_>>(),
            max_text_bytes: usize::try_from(self.max_text_bytes).unwrap_or(usize::MAX),
            max_image_bytes: usize::try_from(self.max_image_bytes).unwrap_or(usize::MAX),
            detect_sensitive: self.detect_sensitive,
            sensitive_ttl_ms: self.sensitive_ttl_ms,
            keep_auto_generated: self.keep_auto_generated,
        }
    }

    /// The storage limits for these settings.
    #[must_use]
    pub fn limits(&self) -> Limits {
        Limits {
            max_items: usize::try_from(self.max_items).unwrap_or(MAX_ITEMS),
            max_image_total_bytes: self.max_image_total_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_match_the_agreed_values() {
        let settings = Settings::default();

        assert_eq!(settings.validate(), Ok(()));
        assert_eq!(settings.sensitive_ttl_ms, 120_000);
        assert_eq!(settings.max_image_bytes, 5 * 1024 * 1024);
        assert_eq!(settings.max_image_total_bytes, 64 * 1024 * 1024);
        assert!(
            settings
                .filter_policy()
                .ignored_apps
                .contains("org.keepassxc.keepassxc")
        );
        assert_eq!(settings.limits().max_items, 500);
    }

    #[test]
    fn each_out_of_range_setting_is_named() {
        let base = Settings::default();
        let cases = [
            (
                Settings {
                    max_items: 49,
                    ..base.clone()
                },
                "max_items",
            ),
            (
                Settings {
                    max_items: 10_001,
                    ..base.clone()
                },
                "max_items",
            ),
            (
                Settings {
                    max_text_bytes: 10,
                    ..base.clone()
                },
                "max_text_bytes",
            ),
            (
                Settings {
                    max_image_bytes: 9 * MIB,
                    ..base.clone()
                },
                "max_image_bytes",
            ),
            (
                Settings {
                    max_image_total_bytes: 0,
                    ..base.clone()
                },
                "max_image_total_bytes",
            ),
            (
                Settings {
                    sensitive_ttl_ms: 1,
                    ..base.clone()
                },
                "sensitive_ttl_ms",
            ),
            (
                Settings {
                    ignored_apps: vec![" ".into()],
                    ..base.clone()
                },
                "ignored_apps",
            ),
            (
                Settings {
                    ignored_apps: vec!["com.example\n".into()],
                    ..base.clone()
                },
                "ignored_apps",
            ),
            (
                Settings {
                    ignored_apps: vec!["a".into(); 257],
                    ..base
                },
                "ignored_apps",
            ),
        ];

        for (settings, field) in cases {
            assert_eq!(settings.validate().map_err(|e| e.field), Err(field));
        }
    }
}
