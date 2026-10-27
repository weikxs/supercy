import Foundation

/// Where the settings screen loads and saves settings: the engine in the main process, or the
/// main process over a pipe when the screen runs in its own short-lived process.
public protocol SettingsStore: AnyObject {
    func load() throws -> Settings
    /// Validates and saves; returns what was saved.
    func save(_ settings: Settings) throws -> Settings
}

/// Settings straight from the engine.
public final class EngineSettingsStore: SettingsStore {
    private let engine: SupercyEngine

    public init(engine: SupercyEngine) {
        self.engine = engine
    }

    public func load() throws -> Settings { try engine.settings() }

    public func save(_ settings: Settings) throws -> Settings { try engine.updateSettings(settings: settings) }
}

/// Why a settings message failed.
public struct SettingsChannelError: Error, Equatable {
    public let message: String
}

/// The JSON-lines protocol between the main process (which owns the history) and the settings
/// process (which only shows the screen):
///
///     main → settings   {"type":"open","language":"zh","settings":{…}}
///     settings → main   {"type":"save","settings":{…}}
///     main → settings   {"type":"saved","settings":{…}}   or   {"type":"error","message":"…"}
///
/// Closing the pipe to the settings process asks it to quit.
public enum SettingsChannel {
    public static func openMessage(_ settings: Settings, language: UILanguage) -> String {
        line(["type": "open", "language": language == .chinese ? "zh" : "en", "settings": SettingsCoding.encode(settings)])
    }

    public static func saveMessage(_ settings: Settings) -> String {
        line(["type": "save", "settings": SettingsCoding.encode(settings)])
    }

    /// Parses an `open` message.
    public static func parseOpen(_ text: String) -> (settings: Settings, language: UILanguage)? {
        guard let object = object(text), object["type"] as? String == "open",
              let fields = object["settings"] as? [String: Any], let settings = SettingsCoding.decode(fields)
        else { return nil }
        return (settings, object["language"] as? String == "zh" ? .chinese : .english)
    }

    /// The main process's answer to a `save` line: saves through `store` and replies; `nil` for
    /// lines that are not a valid `save`. `saved` receives what was saved.
    public static func handle(_ text: String, store: SettingsStore, saved: (Settings) -> Void) -> String {
        guard let object = object(text), object["type"] as? String == "save",
              let fields = object["settings"] as? [String: Any], let requested = SettingsCoding.decode(fields)
        else { return line(["type": "error", "message": "unreadable settings message"]) }
        do {
            let result = try store.save(requested)
            saved(result)
            return line(["type": "saved", "settings": SettingsCoding.encode(result)])
        } catch {
            return line(["type": "error", "message": SettingsModel.message(error)])
        }
    }

    /// The settings process's reading of a reply.
    public static func parseReply(_ text: String) -> Result<Settings, SettingsChannelError> {
        guard let object = object(text) else { return .failure(SettingsChannelError(message: "no answer from Supercy")) }
        if object["type"] as? String == "saved", let fields = object["settings"] as? [String: Any],
           let settings = SettingsCoding.decode(fields) {
            return .success(settings)
        }
        return .failure(SettingsChannelError(message: object["message"] as? String ?? "unreadable answer"))
    }

    static func line(_ object: [String: Any]) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]),
              let text = String(data: data, encoding: .utf8) else { return "{}" }
        return text
    }

    static func object(_ text: String) -> [String: Any]? {
        guard let data = text.data(using: .utf8) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }
}

/// The settings process's store: sends each save to the main process and waits for the answer.
public final class RemoteSettingsStore: SettingsStore {
    private var current: Settings
    private let send: (String) -> Void
    private let receive: () -> String?

    /// `receive` blocks until the main process answers (or returns `nil` when it is gone).
    public init(initial: Settings, send: @escaping (String) -> Void, receive: @escaping () -> String?) {
        current = initial
        self.send = send
        self.receive = receive
    }

    public func load() throws -> Settings { current }

    public func save(_ settings: Settings) throws -> Settings {
        send(SettingsChannel.saveMessage(settings))
        guard let reply = receive() else { throw SettingsChannelError(message: "Supercy is not running") }
        current = try SettingsChannel.parseReply(reply).get()
        return current
    }
}
