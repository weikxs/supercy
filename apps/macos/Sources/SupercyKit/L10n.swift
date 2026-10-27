import Foundation

/// Every interface text in English and Simplified Chinese. A test checks both are complete.
public enum L10nKey: String, CaseIterable {
    case appName, searchPlaceholder, emptyHistory, noMatches, openHistory, pauseRecording
    case pauseFifteenMinutes, pauseOneHour, pauseUntilResumed, resumeRecording, ignoreNextCopy
    case settings, quit, clearHistory, clearAll, pin, unpin, delete, paste, pastePlain, copyOnly
    case copiedFrom, copies, firstCopied, lastUsed, size, sensitiveUntil, truncated, pinKey
    case tabGeneral, tabStorage, tabIgnore, tabAppearance, tabPermissions, tabAbout
    case launchAtLogin, historyShortcut, recordShortcut, pressShortcut, pasteAutomatically
    case pastePlainByDefault, popupPosition, popupCursor, popupMenuIcon, popupCenter, popupLast
    case language, languageSystem, languageChinese, languageEnglish, searchMode, searchFuzzy
    case searchExact, searchRegex, searchMixed, sortOrder, sortLastUsed, sortFirstCopied
    case sortCopyCount, pinPosition, pinTop, pinBottom, maxItems, recordText, recordImages
    case recordFiles, detectSensitive, sensitiveMinutes, clearOnQuit, ignoredApps, allowListMode
    case allowedApps, ignoredTypes, ignorePatterns, add, remove, invalidPattern, showMenuIcon
    case showRecentInMenuBar, showSpecialChars, visibleItems, imagePreviewHeight, pollInterval
    case accessibility, accessibilityGranted, accessibilityMissing, openSystemSettings
    case accessibilityExplanation, needsAccessibilityTitle, version, coreVersion, privacyNote
    case keyHints, paused, kindText, kindRichText, kindLink, kindImage, kindFiles, invalidRegex
    case menuIcon, iconSupercy, iconPaperclip, iconScissors, iconStack, trialMode
    case pinnedItems, exportPins, importPins, exportPinsNote, pinsExported, pinsAdded, pinsPinnedExisting
    case pinsAlreadyPinned, pinsSkipped
}

/// The interface language actually used.
public enum UILanguage: Equatable {
    case english, chinese

    /// The app's localizations, in the order the bundle declares them.
    static let localizations = ["en", "zh-Hans"]

    /// The bundle localization for this language.
    public var localization: String { self == .chinese ? "zh-Hans" : "en" }

    /// Resolves the setting against the system's preferred languages the way macOS picks a
    /// bundle localization: the first preferred language Supercy has, English when none.
    /// Traditional Chinese is not Simplified Chinese, so it falls back to English.
    public static func resolve(_ setting: Language, preferred: [String] = Locale.preferredLanguages) -> UILanguage {
        switch setting {
        case .english: return .english
        case .simplifiedChinese: return .chinese
        case .system:
            let match = Bundle.preferredLocalizations(from: localizations, forPreferences: preferred).first
            return match == UILanguage.chinese.localization ? .chinese : .english
        }
    }
}

/// Looks up interface texts.
public enum L10n {
    public static var language: UILanguage = .english

    public static func text(_ key: L10nKey, _ language: UILanguage = L10n.language) -> String {
        let table = language == .chinese ? chinese : english
        return table[key] ?? english[key] ?? key.rawValue
    }

    static let english: [L10nKey: String] = [
        .appName: "Supercy", .searchPlaceholder: "Type to search", .emptyHistory: "Nothing copied yet",
        .noMatches: "No matches", .openHistory: "Open History", .pauseRecording: "Pause Recording",
        .pauseFifteenMinutes: "For 15 Minutes", .pauseOneHour: "For 1 Hour",
        .pauseUntilResumed: "Until Resumed", .resumeRecording: "Resume Recording",
        .ignoreNextCopy: "Ignore Next Copy", .settings: "Settings…", .quit: "Quit Supercy",
        .clearHistory: "Clear History (Keep Pins)", .clearAll: "Clear Everything", .pin: "Pin",
        .unpin: "Unpin", .delete: "Delete", .paste: "Paste", .pastePlain: "Paste Without Formatting",
        .copyOnly: "Copy Only", .copiedFrom: "Copied from", .copies: "Copies",
        .firstCopied: "First copied", .lastUsed: "Last used", .size: "Size",
        .sensitiveUntil: "Sensitive, deleted at", .truncated: "Cut to the size limit",
        .pinKey: "Pin key", .tabGeneral: "General", .tabStorage: "Storage", .tabIgnore: "Ignore",
        .tabAppearance: "Appearance", .tabPermissions: "Permissions", .tabAbout: "About",
        .launchAtLogin: "Open at login", .historyShortcut: "Open history",
        .recordShortcut: "Change", .pressShortcut: "Press the new shortcut…",
        .pasteAutomatically: "Paste into the front app after choosing",
        .pastePlainByDefault: "Paste without formatting by default", .popupPosition: "Open at",
        .popupCursor: "Mouse pointer", .popupMenuIcon: "Menu bar icon", .popupCenter: "Screen center",
        .popupLast: "Last position", .language: "Language", .languageSystem: "System",
        .languageChinese: "简体中文", .languageEnglish: "English", .searchMode: "Search",
        .searchFuzzy: "Fuzzy (with pinyin)", .searchExact: "Exact", .searchRegex: "Regular expression",
        .searchMixed: "Mixed", .sortOrder: "Sort by", .sortLastUsed: "Last used",
        .sortFirstCopied: "First copied", .sortCopyCount: "Number of copies", .pinPosition: "Pins",
        .pinTop: "On top", .pinBottom: "At the bottom", .maxItems: "Items kept (pins not counted)",
        .recordText: "Text", .recordImages: "Images", .recordFiles: "Files",
        .detectSensitive: "Detect sensitive content and delete it after", .sensitiveMinutes: "minutes",
        .clearOnQuit: "Clear history when quitting (keep pins)", .ignoredApps: "Ignored apps",
        .allowListMode: "Record only the apps below", .allowedApps: "Recorded apps",
        .ignoredTypes: "Ignored pasteboard types", .ignorePatterns: "Ignored text (regular expressions)",
        .add: "Add", .remove: "Remove", .invalidPattern: "Not a valid regular expression",
        .showMenuIcon: "Show menu bar icon", .showRecentInMenuBar: "Show the latest copy in the menu bar",
        .showSpecialChars: "Show spaces, tabs and line breaks", .visibleItems: "Visible rows",
        .imagePreviewHeight: "Image preview height", .pollInterval: "Check the clipboard every (ms)",
        .accessibility: "Accessibility", .accessibilityGranted: "Allowed",
        .accessibilityMissing: "Not allowed", .openSystemSettings: "Open System Settings",
        .accessibilityExplanation: "To paste into the app you were using, Supercy sends ⌘V for you. macOS asks you to allow this under Privacy & Security → Accessibility. Without it, Supercy copies the item and you paste it yourself.",
        .needsAccessibilityTitle: "Allow Supercy to paste for you?", .version: "Version",
        .coreVersion: "Core", .privacyNote: "Your history stays on this Mac, encrypted. No account, no cloud, no telemetry.",
        .keyHints: "⏎ paste · ⌥⏎ copy · ⌥⇧⏎ plain text · ⌥P pin · ⌥⌘↑↓ move pin · ⌥⌫ delete · ⌘1–9 choose · ⌃letter pinned",
        .paused: "Recording paused", .kindText: "Text", .kindRichText: "Rich text", .kindLink: "Link",
        .kindImage: "Image", .kindFiles: "Files", .invalidRegex: "Invalid regular expression",
        .menuIcon: "Menu bar icon", .iconSupercy: "Supercy", .iconPaperclip: "Paper clip",
        .iconScissors: "Scissors", .iconStack: "Stack",
        .trialMode: "Trial: history kept in /tmp/supercy only",
        .pinnedItems: "Pinned items", .exportPins: "Export…", .importPins: "Import…",
        .exportPinsNote: "Saves the pinned items, in order and with their letters, to a file you can import on another Mac. The file is not encrypted: keep it somewhere safe.",
        .pinsExported: "Exported", .pinsAdded: "Added", .pinsPinnedExisting: "Pinned from history",
        .pinsAlreadyPinned: "Already pinned", .pinsSkipped: "Empty, skipped",
    ]

    static let chinese: [L10nKey: String] = [
        .appName: "Supercy", .searchPlaceholder: "输入以搜索", .emptyHistory: "还没有复制过内容",
        .noMatches: "没有匹配项", .openHistory: "打开历史", .pauseRecording: "暂停记录",
        .pauseFifteenMinutes: "15 分钟", .pauseOneHour: "1 小时", .pauseUntilResumed: "直到恢复",
        .resumeRecording: "恢复记录", .ignoreNextCopy: "忽略下一次复制", .settings: "设置…",
        .quit: "退出 Supercy", .clearHistory: "清空历史（保留置顶）", .clearAll: "全部清空",
        .pin: "置顶", .unpin: "取消置顶", .delete: "删除", .paste: "粘贴",
        .pastePlain: "粘贴为纯文本", .copyOnly: "仅复制", .copiedFrom: "来源",
        .copies: "复制次数", .firstCopied: "首次复制", .lastUsed: "最近使用", .size: "大小",
        .sensitiveUntil: "敏感内容，删除于", .truncated: "已按上限截断", .pinKey: "置顶按键",
        .tabGeneral: "通用", .tabStorage: "存储", .tabIgnore: "忽略", .tabAppearance: "外观",
        .tabPermissions: "权限", .tabAbout: "关于", .launchAtLogin: "登录时打开",
        .historyShortcut: "打开历史", .recordShortcut: "修改", .pressShortcut: "请按下新的快捷键…",
        .pasteAutomatically: "选中后直接粘贴到前台应用", .pastePlainByDefault: "默认粘贴为纯文本",
        .popupPosition: "打开位置", .popupCursor: "鼠标处", .popupMenuIcon: "菜单栏图标下",
        .popupCenter: "屏幕中央", .popupLast: "上次位置", .language: "语言",
        .languageSystem: "跟随系统", .languageChinese: "简体中文", .languageEnglish: "English",
        .searchMode: "搜索", .searchFuzzy: "模糊（含拼音）", .searchExact: "精确",
        .searchRegex: "正则表达式", .searchMixed: "混合", .sortOrder: "排序",
        .sortLastUsed: "最近使用", .sortFirstCopied: "首次复制", .sortCopyCount: "复制次数",
        .pinPosition: "置顶条目", .pinTop: "放在顶部", .pinBottom: "放在底部",
        .maxItems: "保留条数（不含置顶）", .recordText: "文本", .recordImages: "图片",
        .recordFiles: "文件", .detectSensitive: "识别敏感内容，并在以下时间后删除",
        .sensitiveMinutes: "分钟", .clearOnQuit: "退出时清空历史（保留置顶）",
        .ignoredApps: "忽略的应用", .allowListMode: "只记录下列应用", .allowedApps: "记录的应用",
        .ignoredTypes: "忽略的剪贴板类型", .ignorePatterns: "忽略的文本（正则表达式）", .add: "添加",
        .remove: "移除", .invalidPattern: "不是有效的正则表达式", .showMenuIcon: "显示菜单栏图标",
        .showRecentInMenuBar: "在菜单栏显示最近一次复制", .showSpecialChars: "显示空格、制表符和换行",
        .visibleItems: "可见行数", .imagePreviewHeight: "图片预览高度",
        .pollInterval: "检查剪贴板的间隔（毫秒）", .accessibility: "辅助功能",
        .accessibilityGranted: "已允许", .accessibilityMissing: "未允许",
        .openSystemSettings: "打开系统设置",
        .accessibilityExplanation: "为了粘贴到你刚才使用的应用，Supercy 会替你按下 ⌘V。macOS 需要你在“隐私与安全性 → 辅助功能”中允许。不允许时，Supercy 只复制条目，由你自己粘贴。",
        .needsAccessibilityTitle: "允许 Supercy 替你粘贴吗？", .version: "版本", .coreVersion: "核心",
        .privacyNote: "历史只保存在这台 Mac 上，并且加密。不用注册，不上云，没有遥测。",
        .keyHints: "⏎ 粘贴 · ⌥⏎ 复制 · ⌥⇧⏎ 纯文本 · ⌥P 置顶 · ⌥⌘↑↓ 移动置顶 · ⌥⌫ 删除 · ⌘1–9 选择 · ⌃字母 置顶项",
        .paused: "已暂停记录", .kindText: "文本", .kindRichText: "富文本", .kindLink: "链接",
        .kindImage: "图片", .kindFiles: "文件", .invalidRegex: "正则表达式无效",
        .menuIcon: "菜单栏图标", .iconSupercy: "Supercy", .iconPaperclip: "回形针",
        .iconScissors: "剪刀", .iconStack: "叠层",
        .trialMode: "试用模式：历史只保存在 /tmp/supercy",
        .pinnedItems: "置顶条目", .exportPins: "导出…", .importPins: "导入…",
        .exportPinsNote: "把置顶条目按顺序连同字母键保存成一个文件，可以在另一台 Mac 上导入。这个文件没有加密，请妥善保管。",
        .pinsExported: "已导出", .pinsAdded: "新增", .pinsPinnedExisting: "从历史中置顶",
        .pinsAlreadyPinned: "原本已置顶", .pinsSkipped: "内容为空，已跳过",
    ]
}
