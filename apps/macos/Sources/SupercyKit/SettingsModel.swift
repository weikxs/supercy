import Combine
import Foundation

/// The settings screen's state: a working copy of the core's settings, saved on every change.
/// The core validates; a refused change is reported and the copy goes back to what was saved.
public final class SettingsModel: ObservableObject {
    @Published public var settings: Settings
    @Published public private(set) var error: String?
    @Published public var launchAtLogin: Bool {
        didSet { if launchAtLogin != system.launchAtLogin { system.launchAtLogin = launchAtLogin } }
    }

    private let store: SettingsStore
    private let system: SystemIntegration
    /// Called after settings were saved (the app re-registers shortcuts, intervals, icons).
    public var onSaved: ((Settings) -> Void)?

    public init(store: SettingsStore, system: SystemIntegration) throws {
        self.store = store
        self.system = system
        settings = try store.load()
        launchAtLogin = system.launchAtLogin
    }

    public convenience init(engine: SupercyEngine, system: SystemIntegration) throws {
        try self.init(store: EngineSettingsStore(engine: engine), system: system)
    }

    public var isAccessibilityTrusted: Bool { system.isAccessibilityTrusted }

    public func openAccessibilitySettings() { system.openAccessibilitySettings() }

    /// Saves `change` applied to the current settings.
    public func update(_ change: (inout Settings) -> Void) {
        var next = settings
        change(&next)
        save(next)
    }

    public func save(_ next: Settings) {
        do {
            settings = try store.save(next)
            error = nil
            L10n.language = UILanguage.resolve(settings.language)
            onSaved?(settings)
        } catch {
            self.error = Self.message(error)
            settings = (try? store.load()) ?? settings
        }
    }

    /// Adds `entry` to the list at `keyPath` unless empty or present.
    public func add(_ entry: String, to keyPath: WritableKeyPath<Settings, [String]>) {
        let trimmed = entry.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, !settings[keyPath: keyPath].contains(trimmed) else { return }
        update { $0[keyPath: keyPath].append(trimmed) }
    }

    public func remove(_ entry: String, from keyPath: WritableKeyPath<Settings, [String]>) {
        update { $0[keyPath: keyPath].removeAll { $0 == entry } }
    }

    /// Why `pattern` is not a valid ignore pattern, if it is not.
    public func patternProblem(_ pattern: String) -> String? {
        checkIgnorePattern(pattern: pattern)
    }

    static func message(_ error: Error) -> String {
        switch error as? SupercyError {
        case let .InvalidSetting(message)?, let .Internal(message)?, let .NotFound(message)?,
             let .KeyMissing(message)?, let .Locked(message)?, let .TooNew(message)?,
             let .InvalidPinKey(message)?, let .InvalidId(message)?, let .Keystore(message)?:
            return message
        case nil:
            return (error as? SettingsChannelError)?.message ?? error.localizedDescription
        }
    }
}
