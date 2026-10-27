import AppKit
import SwiftUI

/// Runs the settings screen in its own short-lived process (this executable with
/// `--settings-ui`), so the menu bar app never loads the settings' SwiftUI views and keeps its
/// memory low. The main process stays the only owner of the history: it validates and saves
/// every change sent over the pipe (see `SettingsChannel`).
final class SettingsProcess {
    private let store: SettingsStore
    private let onSaved: (Settings) -> Void
    private let onExit: () -> Void
    private var process: Process?
    private var toChild: FileHandle?

    init(store: SettingsStore, onSaved: @escaping (Settings) -> Void, onExit: @escaping () -> Void) {
        self.store = store
        self.onSaved = onSaved
        self.onExit = onExit
    }

    var isRunning: Bool { process?.isRunning ?? false }

    /// Opens the settings, or brings the open window forward.
    func open(extraArguments: [String]) {
        if let process, process.isRunning {
            NSRunningApplication(processIdentifier: process.processIdentifier)?.activate()
            return
        }
        guard let executable = Bundle.main.executableURL, let settings = try? store.load() else { return }
        let process = Process()
        let input = Pipe()
        let output = Pipe()
        process.executableURL = executable
        // AppKit's own texts in the window (text field menus, buttons) use the same language.
        process.arguments = ["--settings-ui"] + extraArguments + ["-AppleLanguages", "(\(L10n.language.localization))"]
        process.standardInput = input
        process.standardOutput = output
        process.terminationHandler = { [weak self] _ in DispatchQueue.main.async { self?.finished() } }
        do {
            try process.run()
        } catch {
            NSLog("Supercy: settings could not start: %@", error.localizedDescription)
            return
        }
        self.process = process
        toChild = input.fileHandleForWriting
        send(SettingsChannel.openMessage(settings, language: L10n.language))
        readLines(from: output.fileHandleForReading)
    }

    /// Closes the settings (closing the pipe asks the process to quit).
    func close() {
        try? toChild?.close()
        toChild = nil
    }

    private func readLines(from handle: FileHandle) {
        let reader = LineReader(handle: handle)
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            while let line = reader.next() {
                DispatchQueue.main.async { self?.handle(line) }
            }
        }
    }

    private func handle(_ line: String) {
        send(SettingsChannel.handle(line, store: store, saved: onSaved))
    }

    private func send(_ line: String) {
        try? toChild?.write(contentsOf: Data((line + "\n").utf8))
    }

    private func finished() {
        process = nil
        toChild = nil
        onExit()
    }
}

/// Reads newline-separated lines from a pipe; `next` blocks and returns `nil` at the end.
final class LineReader {
    private let handle: FileHandle
    private var buffer = Data()

    init(handle: FileHandle) {
        self.handle = handle
    }

    func next() -> String? {
        while true {
            if let newline = buffer.firstIndex(of: 0x0A) {
                let line = buffer[buffer.startIndex..<newline]
                buffer.removeSubrange(buffer.startIndex...newline)
                return String(decoding: line, as: UTF8.self)
            }
            let chunk = handle.availableData
            if chunk.isEmpty { return nil }
            buffer.append(chunk)
        }
    }
}

/// The settings process: shows the settings window, sends changes to the main process, and
/// quits when the window closes or the main process closes the pipe.
final class SettingsUIApp: NSObject, NSWindowDelegate {
    private var window: NSWindow?
    private let replies = DispatchSemaphore(value: 0)
    private var reply: String?
    private let lock = NSLock()

    func run(arguments: [String]) {
        let reader = LineReader(handle: FileHandle.standardInput)
        guard let first = reader.next(), let open = SettingsChannel.parseOpen(first) else { exit(1) }
        L10n.language = open.language
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            while let line = reader.next() { self?.deliver(line) }
            exit(0) // the main process closed the pipe or quit
        }
        let store = RemoteSettingsStore(initial: open.settings, send: { line in
            FileHandle.standardOutput.write(Data((line + "\n").utf8))
        }, receive: { [weak self] in self?.awaitReply() })
        let base: SystemIntegration = MacSystemIntegration()
        let system = arguments.contains("--trial") ? TrialSystemIntegration(base: base) : base
        guard let model = try? SettingsModel(store: store, system: system) else { exit(1) }
        show(model, offscreen: arguments.contains("--offscreen"))
    }

    private func deliver(_ line: String) {
        lock.lock()
        reply = line
        lock.unlock()
        replies.signal()
    }

    /// Waits for the main process's answer to a save (it answers at once; 5 s is generous).
    private func awaitReply() -> String? {
        guard replies.wait(timeout: .now() + 5) == .success else { return nil }
        lock.lock(); defer { lock.unlock() }
        return reply
    }

    private func show(_ model: SettingsModel, offscreen: Bool) {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 560, height: 520),
                              styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = L10n.text(.settings).replacingOccurrences(of: "…", with: "")
        window.contentView = NSHostingView(rootView: SettingsView(model: model))
        window.delegate = self
        self.window = window
        if offscreen {
            window.setFrameOrigin(NSPoint(x: -20_000, y: -20_000))
            window.orderFront(nil)
        } else {
            window.center()
            NSApp.activate(ignoringOtherApps: true)
            window.makeKeyAndOrderFront(nil)
        }
    }

    func windowWillClose(_ notification: Notification) {
        exit(0)
    }
}
