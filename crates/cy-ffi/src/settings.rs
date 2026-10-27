//! Settings as Swift sees them, and their conversions.

use cy_engine as engine;

/// How unpinned items are sorted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SortOrder {
    /// Most recently used first.
    LastUsed,
    /// Most recently first copied first.
    FirstCopied,
    /// Most often copied first.
    CopyCount,
}

/// Where pinned items go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum PinPosition {
    /// Above the other items.
    Top,
    /// Below the other items.
    Bottom,
}

/// Where the history opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum PopupPosition {
    /// At the mouse pointer.
    Cursor,
    /// Below the menu bar icon.
    MenuIcon,
    /// In the middle of the screen.
    ScreenCenter,
    /// Where it was last time.
    LastPosition,
}

/// The menu bar icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MenuIcon {
    /// A clipboard.
    Clipboard,
    /// A paper clip.
    Paperclip,
    /// Scissors.
    Scissors,
    /// A stack of pages.
    Stack,
}

/// The interface language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Language {
    /// Follow the system (Chinese or English, else English).
    System,
    /// Simplified Chinese.
    SimplifiedChinese,
    /// English.
    English,
}

/// Maps between the FFI enum and the engine's stored enum, both ways.
macro_rules! both_ways {
    ($ffi:ident, $engine:ty, [$($variant:ident),+]) => {
        impl From<$engine> for $ffi {
            fn from(value: $engine) -> Self {
                match value { $(<$engine>::$variant => Self::$variant),+ }
            }
        }
        impl From<$ffi> for $engine {
            fn from(value: $ffi) -> Self {
                match value { $($ffi::$variant => Self::$variant),+ }
            }
        }
    };
}

both_ways!(
    SortOrder,
    engine::SortSetting,
    [LastUsed, FirstCopied, CopyCount]
);
both_ways!(PinPosition, engine::PinPositionSetting, [Top, Bottom]);
both_ways!(
    MenuIcon,
    engine::MenuIcon,
    [Clipboard, Paperclip, Scissors, Stack]
);
both_ways!(
    PopupPosition,
    engine::PopupPosition,
    [Cursor, MenuIcon, ScreenCenter, LastPosition]
);
both_ways!(
    Language,
    engine::Language,
    [System, SimplifiedChinese, English]
);
both_ways!(
    SearchMode,
    engine::SearchModeSetting,
    [Fuzzy, Exact, Regex, Mixed]
);

use crate::SearchMode;

/// Everything the user chooses. Ranges are checked by `update_settings`.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "user switches are naturally on/off values"
)]
pub struct Settings {
    /// Unpinned items kept (50–10 000).
    pub max_items: u32,
    /// Longest text kept, in bytes (1 KiB–2 MiB).
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
    /// The allow-list, by bundle identifier.
    pub allowed_apps: Vec<String>,
    /// Clipboard types that mean "do not keep".
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
    pub search_mode: SearchMode,
    /// Sort order.
    pub sort: SortOrder,
    /// Where pinned items go.
    pub pin_position: PinPosition,
    /// Paste into the front app after choosing (needs Accessibility); else only copy.
    pub paste_automatically: bool,
    /// Paste without formatting by default.
    pub paste_plain_by_default: bool,
    /// Where the history opens.
    pub popup_position: PopupPosition,
    /// Shortcut for the history, such as `shift+cmd+v`.
    pub history_shortcut: String,
    /// Shortcut for a screenshot (from version 0.2).
    pub capture_shortcut: String,
    /// Clipboard check interval in milliseconds (100–2000).
    pub poll_interval_ms: u32,
    /// Show spaces, tabs and line breaks in previews.
    pub show_special_chars: bool,
    /// Show the menu bar icon.
    pub show_menu_icon: bool,
    /// Which menu bar icon.
    pub menu_icon: MenuIcon,
    /// Show the latest copy next to the menu bar icon.
    pub show_recent_in_menu_bar: bool,
    /// Clear the history (except pins) on quit.
    pub clear_on_quit: bool,
    /// Rows visible before scrolling (5–100).
    pub visible_items: u32,
    /// Image preview height in the list, in points (16–400).
    pub image_preview_height: u32,
    /// Interface language.
    pub language: Language,
}

/// Copies every field between two structs with the same field names, converting each.
macro_rules! convert_settings {
    ($from:expr, $to:path, [$($field:ident),+]) => {{
        let from = $from;
        $to { $($field: from.$field.into()),+ }
    }};
}

impl From<engine::Settings> for Settings {
    fn from(s: engine::Settings) -> Self {
        convert_settings!(
            s,
            Self,
            [
                max_items,
                max_text_bytes,
                max_image_bytes,
                max_image_total_bytes,
                detect_sensitive,
                sensitive_ttl_ms,
                keep_auto_generated,
                ignored_apps,
                allow_list_mode,
                allowed_apps,
                ignored_types,
                ignore_patterns,
                record_text,
                record_images,
                record_files,
                search_mode,
                sort,
                pin_position,
                paste_automatically,
                paste_plain_by_default,
                popup_position,
                history_shortcut,
                capture_shortcut,
                poll_interval_ms,
                show_special_chars,
                show_menu_icon,
                menu_icon,
                show_recent_in_menu_bar,
                clear_on_quit,
                visible_items,
                image_preview_height,
                language
            ]
        )
    }
}

impl From<Settings> for engine::Settings {
    fn from(s: Settings) -> Self {
        convert_settings!(
            s,
            Self,
            [
                max_items,
                max_text_bytes,
                max_image_bytes,
                max_image_total_bytes,
                detect_sensitive,
                sensitive_ttl_ms,
                keep_auto_generated,
                ignored_apps,
                allow_list_mode,
                allowed_apps,
                ignored_types,
                ignore_patterns,
                record_text,
                record_images,
                record_files,
                search_mode,
                sort,
                pin_position,
                paste_automatically,
                paste_plain_by_default,
                popup_position,
                history_shortcut,
                capture_shortcut,
                poll_interval_ms,
                show_special_chars,
                show_menu_icon,
                menu_icon,
                show_recent_in_menu_bar,
                clear_on_quit,
                visible_items,
                image_preview_height,
                language
            ]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_the_ffi_form() {
        let original = engine::Settings {
            sort: engine::SortSetting::CopyCount,
            pin_position: engine::PinPositionSetting::Bottom,
            popup_position: engine::PopupPosition::ScreenCenter,
            language: engine::Language::SimplifiedChinese,
            menu_icon: engine::MenuIcon::Scissors,
            search_mode: engine::SearchModeSetting::Mixed,
            ..engine::Settings::default()
        };

        let back: engine::Settings = Settings::from(original.clone()).into();

        assert_eq!(back, original);
    }
}
