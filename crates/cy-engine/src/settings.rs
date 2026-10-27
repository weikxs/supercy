//! User settings: validated, stored sealed in the history database.
//!
//! Settings the shell alone acts on (shortcuts, popup position, appearance) live here too, so
//! there is one validated, encrypted place for everything the user chooses.

use std::collections::BTreeSet;

use cy_filter::{FilterPolicy, PatternSet, validate_pattern};
use cy_search::{ListOrder, MatchMode, PinPosition, SortOrder};
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
/// Default clipboard check interval (decision D-5).
pub const DEFAULT_POLL_INTERVAL_MS: u32 = 500;
/// Default shortcut for the history (decision D-4).
pub const DEFAULT_HISTORY_SHORTCUT: &str = "shift+cmd+v";
/// Default shortcut for a screenshot (decision D-4; used from milestone M4).
pub const DEFAULT_CAPTURE_SHORTCUT: &str = "shift+cmd+2";

/// Password managers ignored out of the box (decision D-3), by bundle identifier. Most of them
/// also mark their copies as concealed; this list covers versions that do not.
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
const POLL_RANGE: std::ops::RangeInclusive<u32> = 100..=2_000;
const VISIBLE_ITEMS_RANGE: std::ops::RangeInclusive<u32> = 5..=100;
const IMAGE_PREVIEW_RANGE: std::ops::RangeInclusive<u32> = 16..=400;
const MAX_LIST_ENTRIES: usize = 256;
const MAX_ENTRY_LEN: usize = 255;
const MAX_SHORTCUT_LEN: usize = 64;

/// A setting outside its allowed range.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("the setting `{field}` is out of range ({allowed})")]
pub struct SettingsError {
    /// The setting's name.
    pub field: &'static str,
    /// The allowed values, for the message.
    pub allowed: String,
}

/// How search matches (stored form of [`MatchMode`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SearchModeSetting {
    /// Characters in order, gaps allowed; pinyin included.
    #[default]
    Fuzzy,
    /// Substring.
    Exact,
    /// Regular expression.
    Regex,
    /// Exact, else regular expression, else fuzzy.
    Mixed,
}

impl From<SearchModeSetting> for MatchMode {
    fn from(mode: SearchModeSetting) -> Self {
        match mode {
            SearchModeSetting::Fuzzy => Self::Fuzzy,
            SearchModeSetting::Exact => Self::Exact,
            SearchModeSetting::Regex => Self::Regex,
            SearchModeSetting::Mixed => Self::Mixed,
        }
    }
}

/// How unpinned items are sorted (stored form of [`SortOrder`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortSetting {
    /// Most recently used first.
    #[default]
    LastUsed,
    /// Most recently first copied first.
    FirstCopied,
    /// Most often copied first.
    CopyCount,
}

/// Where pinned items go (stored form of [`PinPosition`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PinPositionSetting {
    /// Above the other items.
    #[default]
    Top,
    /// Below the other items.
    Bottom,
}

/// Where the history opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PopupPosition {
    /// At the mouse pointer.
    #[default]
    Cursor,
    /// Below the menu bar icon.
    MenuIcon,
    /// In the middle of the screen.
    ScreenCenter,
    /// Where it was last time.
    LastPosition,
}

/// The interface language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Language {
    /// The system language when it is Chinese or English, otherwise English.
    #[default]
    System,
    /// Simplified Chinese.
    SimplifiedChinese,
    /// English.
    English,
}

/// What Supercy keeps, for how long, and how it looks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "user switches are naturally on/off values"
)]
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
    /// Allow-list mode: keep only copies from `allowed_apps`.
    pub allow_list_mode: bool,
    /// The allow-list.
    pub allowed_apps: Vec<String>,
    /// Clipboard types that mean "do not keep" (in addition to the built-in markers).
    pub ignored_types: Vec<String>,
    /// Regular expressions: matching text is not kept.
    pub ignore_patterns: Vec<String>,
    /// Keep text, rich text and links.
    pub record_text: bool,
    /// Keep images.
    pub record_images: bool,
    /// Keep copied files (references only).
    pub record_files: bool,
    /// Search mode.
    pub search_mode: SearchModeSetting,
    /// Sort order.
    pub sort: SortSetting,
    /// Where pinned items go.
    pub pin_position: PinPositionSetting,
    /// Paste into the front app after choosing an item (needs Accessibility); else only copy.
    pub paste_automatically: bool,
    /// Paste without formatting by default.
    pub paste_plain_by_default: bool,
    /// Where the history opens.
    pub popup_position: PopupPosition,
    /// Shortcut for the history, such as `shift+cmd+v`.
    pub history_shortcut: String,
    /// Shortcut for a screenshot (milestone M4).
    pub capture_shortcut: String,
    /// Clipboard check interval in milliseconds (100–2000).
    pub poll_interval_ms: u32,
    /// Show spaces, tabs and line breaks in previews.
    pub show_special_chars: bool,
    /// Show the menu bar icon (when hidden, open with the shortcut).
    pub show_menu_icon: bool,
    /// Show the latest copy next to the menu bar icon.
    pub show_recent_in_menu_bar: bool,
    /// Clear the history (except pins) when Supercy quits.
    pub clear_on_quit: bool,
    /// Rows visible in the history before scrolling (5–100).
    pub visible_items: u32,
    /// Height of image previews in the list, in points (16–400).
    pub image_preview_height: u32,
    /// Interface language.
    pub language: Language,
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
            allow_list_mode: false,
            allowed_apps: Vec::new(),
            ignored_types: Vec::new(),
            ignore_patterns: Vec::new(),
            record_text: true,
            record_images: true,
            record_files: true,
            search_mode: SearchModeSetting::default(),
            sort: SortSetting::default(),
            pin_position: PinPositionSetting::default(),
            paste_automatically: true,
            paste_plain_by_default: false,
            popup_position: PopupPosition::default(),
            history_shortcut: DEFAULT_HISTORY_SHORTCUT.to_owned(),
            capture_shortcut: DEFAULT_CAPTURE_SHORTCUT.to_owned(),
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
            show_special_chars: false,
            show_menu_icon: true,
            show_recent_in_menu_bar: false,
            clear_on_quit: false,
            visible_items: 20,
            image_preview_height: 40,
            language: Language::default(),
        }
    }
}

fn check(ok: bool, field: &'static str, allowed: &str) -> Result<(), SettingsError> {
    if ok {
        Ok(())
    } else {
        Err(SettingsError {
            field,
            allowed: allowed.to_owned(),
        })
    }
}

fn valid_entry(entry: &str) -> bool {
    !entry.trim().is_empty() && entry.len() <= MAX_ENTRY_LEN && !entry.chars().any(char::is_control)
}

fn check_list(list: &[String], field: &'static str) -> Result<(), SettingsError> {
    let ok = list.len() <= MAX_LIST_ENTRIES && list.iter().all(|entry| valid_entry(entry));
    check(ok, field, "at most 256 entries of 1 to 255 characters")
}

fn valid_shortcut(shortcut: &str) -> bool {
    !shortcut.is_empty()
        && shortcut.len() <= MAX_SHORTCUT_LEN
        && shortcut.chars().all(|c| c.is_ascii_graphic())
}

impl Settings {
    /// Checks every setting against its range.
    ///
    /// # Errors
    ///
    /// The first setting out of range; for patterns, which one and why.
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
        check(
            POLL_RANGE.contains(&self.poll_interval_ms),
            "poll_interval_ms",
            "100 to 2000 ms",
        )?;
        check(
            VISIBLE_ITEMS_RANGE.contains(&self.visible_items),
            "visible_items",
            "5 to 100",
        )?;
        check(
            IMAGE_PREVIEW_RANGE.contains(&self.image_preview_height),
            "image_preview_height",
            "16 to 400",
        )?;
        check(
            valid_shortcut(&self.history_shortcut),
            "history_shortcut",
            "1 to 64 printable characters",
        )?;
        check(
            valid_shortcut(&self.capture_shortcut),
            "capture_shortcut",
            "1 to 64 printable characters",
        )?;
        check_list(&self.ignored_apps, "ignored_apps")?;
        check_list(&self.allowed_apps, "allowed_apps")?;
        check_list(&self.ignored_types, "ignored_types")?;
        PatternSet::new(&self.ignore_patterns)
            .map(|_| ())
            .map_err(|e| SettingsError {
                field: "ignore_patterns",
                allowed: e.to_string(),
            })
    }

    /// The capture filter's policy for these (validated) settings.
    #[must_use]
    pub fn filter_policy(&self) -> FilterPolicy {
        let set = |list: &[String]| {
            list.iter()
                .map(|e| e.trim().to_owned())
                .collect::<BTreeSet<_>>()
        };
        FilterPolicy {
            ignored_apps: set(&self.ignored_apps),
            allowed_apps: self.allow_list_mode.then(|| set(&self.allowed_apps)),
            ignored_types: set(&self.ignored_types),
            ignore_patterns: PatternSet::new(&self.ignore_patterns).unwrap_or_default(),
            record_text: self.record_text,
            record_images: self.record_images,
            record_files: self.record_files,
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

    /// The list order for these settings.
    #[must_use]
    pub fn list_order(&self) -> ListOrder {
        ListOrder {
            sort: match self.sort {
                SortSetting::LastUsed => SortOrder::LastUsed,
                SortSetting::FirstCopied => SortOrder::FirstCopied,
                SortSetting::CopyCount => SortOrder::CopyCount,
            },
            pins: match self.pin_position {
                PinPositionSetting::Top => PinPosition::Top,
                PinPositionSetting::Bottom => PinPosition::Bottom,
            },
        }
    }
}

/// Checks one ignore pattern for the settings screen; the error says what is wrong.
///
/// # Errors
///
/// The parser's message.
pub fn check_pattern(pattern: &str) -> Result<(), String> {
    validate_pattern(pattern)
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
        assert_eq!(settings.poll_interval_ms, 500);
        assert_eq!(settings.history_shortcut, "shift+cmd+v");
        assert_eq!(settings.capture_shortcut, "shift+cmd+2");
        assert!(
            settings
                .filter_policy()
                .ignored_apps
                .contains("org.keepassxc.keepassxc")
        );
        assert_eq!(settings.filter_policy().allowed_apps, None);
        assert_eq!(settings.limits().max_items, 500);
        assert_eq!(settings.list_order(), ListOrder::default());
    }

    /// A change to the defaults, and the setting validation should name.
    type Case = (fn(&mut Settings), &'static str);

    /// The field named by validation after `change`.
    fn invalid_field(change: fn(&mut Settings)) -> Result<(), &'static str> {
        let mut settings = Settings::default();
        change(&mut settings);
        settings.validate().map_err(|e| e.field)
    }

    #[test]
    fn each_out_of_range_number_is_named() {
        let cases: [Case; 9] = [
            (|s| s.max_items = 49, "max_items"),
            (|s| s.max_items = 10_001, "max_items"),
            (|s| s.max_text_bytes = 10, "max_text_bytes"),
            (|s| s.max_image_bytes = 9 * MIB, "max_image_bytes"),
            (|s| s.max_image_total_bytes = 0, "max_image_total_bytes"),
            (|s| s.sensitive_ttl_ms = 1, "sensitive_ttl_ms"),
            (|s| s.poll_interval_ms = 50, "poll_interval_ms"),
            (|s| s.visible_items = 1, "visible_items"),
            (|s| s.image_preview_height = 1_000, "image_preview_height"),
        ];

        for (change, field) in cases {
            assert_eq!(invalid_field(change), Err(field));
        }
    }

    #[test]
    fn each_invalid_text_or_list_is_named() {
        let cases: [Case; 6] = [
            (|s| s.history_shortcut = String::new(), "history_shortcut"),
            (|s| s.capture_shortcut = "cmd 2".into(), "capture_shortcut"),
            (|s| s.ignored_apps = vec![" ".into()], "ignored_apps"),
            (
                |s| s.allowed_apps = vec!["com.example\n".into()],
                "allowed_apps",
            ),
            (|s| s.ignored_types = vec!["a".into(); 257], "ignored_types"),
            (|s| s.ignore_patterns = vec!["(".into()], "ignore_patterns"),
        ];

        for (change, field) in cases {
            assert_eq!(invalid_field(change), Err(field));
        }
    }

    #[test]
    fn policy_and_order_follow_the_settings() {
        let settings = Settings {
            allow_list_mode: true,
            allowed_apps: vec![" com.example.editor ".into()],
            ignore_patterns: vec!["^otp".into()],
            record_images: false,
            sort: SortSetting::CopyCount,
            pin_position: PinPositionSetting::Bottom,
            ..Settings::default()
        };

        let policy = settings.filter_policy();

        assert_eq!(
            policy.allowed_apps,
            Some(["com.example.editor".to_owned()].into())
        );
        assert!(policy.ignore_patterns.is_match("OTP 1"));
        assert!(!policy.record_images);
        assert_eq!(
            settings.list_order(),
            ListOrder {
                sort: SortOrder::CopyCount,
                pins: PinPosition::Bottom
            }
        );
        assert!(check_pattern("(").is_err());
        assert_eq!(MatchMode::from(SearchModeSetting::Mixed), MatchMode::Mixed);
    }
}
