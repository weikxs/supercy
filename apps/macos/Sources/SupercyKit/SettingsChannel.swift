import Foundation

/// Where the settings screen loads and saves settings: the engine in the main process, or the
/// main process over a pipe when the screen runs in its own short-lived process.
public protocol SettingsStore: AnyObject {
    func load() throws -> Settings
    /// Validates and saves; returns what was saved.
    func save(_ settings: Settings) throws -> Settings
    /// Writes the pinned items to the file at `path`; returns how many.
    func exportPins(to path: String) throws -> UInt32
    /// Pins the items of the pins file at `path`.
    func importPins(from path: String) throws -> PinsImport
}

/// Settings straight from the engine.
public final class EngineSettingsStore: SettingsStore {
    private let engine: SupercyEngine
    private let clock: Clock

    public init(engine: SupercyEngine, clock: Clock = SystemClock()) {
        self.engine = engine
        self.clock = clock
    }

    public func load() throws -> Settings { try engine.settings() }

    public func save(_ settings: Settings) throws -> Settings { try engine.updateSettings(settings: settings) }

    public func exportPins(to path: String) throws -> UInt32 { try engine.exportPins(path: path) }

    public func importPins(from path: String) throws -> PinsImport {
        try engine.importPins(path: path, nowMs: clock.nowMs())
    }
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
///     settings → main   {"type":"exportPins","path":"…"}
///     main → settings   {"type":"exported","count":3}      or   an error
///     settings → main   {"type":"importPins","path":"…"}
///     main → settings   {"type":"imported","added":2,"pinnedExisting":0,"alreadyPinned":1,"skipped":0}
///
/// The settings process shows the file dialogs; only the chosen path crosses the pipe, and the
/// main process reads or writes the file.
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

    public static func exportPinsMessage(path: String) -> String { line(["type": "exportPins", "path": path]) }

    public static func importPinsMessage(path: String) -> String { line(["type": "importPins", "path": path]) }

    /// The main process's answer to a line from the settings process: saves settings or moves
    /// pinned items through `store` and replies (an `error` reply for anything unreadable).
    /// `saved` receives saved settings.
    public static func handle(_ text: String, store: SettingsStore, saved: (Settings) -> Void) -> String {
        let unreadable = line(["type": "error", "message": "unreadable settings message"])
        guard let object = object(text) else { return unreadable }
        do {
            switch (object["type"] as? String, object["path"] as? String) {
            case ("save", _):
                guard let fields = object["settings"] as? [String: Any],
                      let requested = SettingsCoding.decode(fields) else { return unreadable }
                let result = try store.save(requested)
                saved(result)
                return line(["type": "saved", "settings": SettingsCoding.encode(result)])
            case let ("exportPins", path?):
                return line(["type": "exported", "count": try store.exportPins(to: path)])
            case let ("importPins", path?):
                let report = try store.importPins(from: path)
                return line(["type": "imported", "added": report.added, "pinnedExisting": report.pinnedExisting,
                             "alreadyPinned": report.alreadyPinned, "skipped": report.skipped])
            default:
                return unreadable
            }
        } catch {
            return line(["type": "error", "message": SettingsModel.message(error)])
        }
    }

    /// The fields of a reply of `type`, or the error it reports.
    static func reply(_ text: String?, type: String) throws -> [String: Any] {
        guard let text, let object = object(text) else { throw SettingsChannelError(message: "no answer from Supercy") }
        guard object["type"] as? String == type else {
            throw SettingsChannelError(message: object["message"] as? String ?? "unreadable answer")
        }
        return object
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

    public func exportPins(to path: String) throws -> UInt32 {
        send(SettingsChannel.exportPinsMessage(path: path))
        let fields = try SettingsChannel.reply(receive(), type: "exported")
        return (fields["count"] as? NSNumber)?.uint32Value ?? 0
    }

    public func importPins(from path: String) throws -> PinsImport {
        send(SettingsChannel.importPinsMessage(path: path))
        let fields = try SettingsChannel.reply(receive(), type: "imported")
        let count = { (key: String) in (fields[key] as? NSNumber)?.uint32Value ?? 0 }
        return PinsImport(added: count("added"), pinnedExisting: count("pinnedExisting"),
                          alreadyPinned: count("alreadyPinned"), skipped: count("skipped"))
    }
}
