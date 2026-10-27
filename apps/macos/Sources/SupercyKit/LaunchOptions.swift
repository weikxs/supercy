import Foundation

/// How the app was started.
public enum LaunchMode: Equatable {
    /// The real app: system pasteboard, login keychain, global shortcut, login item.
    case normal
    /// For measurements: an in-memory pasteboard (the user's clipboard is never read), an
    /// in-memory keystore, no global shortcut, data under `/tmp`.
    case isolated
    /// For a manual check with real copies: the system pasteboard, the global shortcut and
    /// pasting as in the real app, but data only under `/tmp`, an in-memory keystore (never the
    /// login keychain) and no login item.
    case trial

    public var readsSystemPasteboard: Bool { self != .isolated }
    public var usesLoginKeychain: Bool { self == .normal }
    public var registersGlobalShortcut: Bool { self != .isolated }
    public var allowsLoginItem: Bool { self == .normal }
}

/// The parsed launch arguments and environment.
public struct LaunchOptions: Equatable {
    public let mode: LaunchMode
    public let dataDirectory: URL

    public init(mode: LaunchMode, dataDirectory: URL) {
        self.mode = mode
        self.dataDirectory = dataDirectory
    }

    /// Only these directories may hold data outside the normal mode.
    static let temporaryPrefixes = ["/tmp/", "/private/tmp/"]

    public enum Problem: Error, Equatable {
        /// `--isolated` and `--trial` together.
        case conflictingModes
        /// `SUPERCY_DATA_DIR` is missing or not under `/tmp`.
        case needsTemporaryDataDirectory
        /// The trial needs `HOME` (and `CFFIXED_USER_HOME`, if set) under `/tmp`, so the
        /// system's caches, preferences and saved state for the app land there too.
        case trialNeedsTemporaryHome
    }

    public static func parse(arguments: [String], environment: [String: String], defaultDataDirectory: URL) -> Result<LaunchOptions, Problem> {
        let isolated = arguments.contains("--isolated")
        let trial = arguments.contains("--trial")
        if isolated && trial { return .failure(.conflictingModes) }
        guard isolated || trial else {
            return .success(LaunchOptions(mode: .normal, dataDirectory: defaultDataDirectory))
        }
        guard let path = environment["SUPERCY_DATA_DIR"], isTemporary(path) else {
            return .failure(.needsTemporaryDataDirectory)
        }
        if trial {
            let homes = [environment["HOME"], environment["CFFIXED_USER_HOME"]]
            guard let home = homes[0], isTemporary(home), homes.compactMap({ $0 }).allSatisfy(isTemporary) else {
                return .failure(.trialNeedsTemporaryHome)
            }
        }
        let url = URL(fileURLWithPath: path, isDirectory: true)
        return .success(LaunchOptions(mode: trial ? .trial : .isolated, dataDirectory: url))
    }

    static func isTemporary(_ path: String) -> Bool {
        temporaryPrefixes.contains { path.hasPrefix($0) } && !path.split(separator: "/").contains("..")
    }

    /// `~/Library/Application Support/Supercy`.
    public static func standardDataDirectory() -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library/Application Support")
        return base.appendingPathComponent("Supercy", isDirectory: true)
    }
}

/// Wraps the system integration for the trial: the login item is never registered.
public final class TrialSystemIntegration: SystemIntegration {
    private let base: SystemIntegration

    public init(base: SystemIntegration) {
        self.base = base
    }

    public var launchAtLogin: Bool {
        get { false }
        set { _ = newValue }
    }

    public var isAccessibilityTrusted: Bool { base.isAccessibilityTrusted }

    public func openAccessibilitySettings() { base.openAccessibilitySettings() }
}
