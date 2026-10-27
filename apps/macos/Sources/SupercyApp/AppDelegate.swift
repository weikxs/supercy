import AppKit
import SwiftUI

/// Wires everything together. In `--isolated` mode (see `LaunchOptions`) nothing touches the
/// user's clipboard, keychain or keyboard shortcuts.
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var controller: ClipboardController?
    private var statusItem: StatusItemController?
    private var panel: HistoryPanelController?
    private var hotKey: HotKey?
    private var settingsWindow: NSWindow?
    private let clock = SystemClock()
    private let system = MacSystemIntegration()

    func applicationDidFinishLaunching(_ notification: Notification) {
        let parsed = LaunchOptions.parse(
            arguments: CommandLine.arguments,
            environment: ProcessInfo.processInfo.environment,
            defaultDataDirectory: LaunchOptions.standardDataDirectory()
        )
        guard case let .success(options) = parsed else {
            fail("The isolated mode needs SUPERCY_DATA_DIR under /tmp.")
            return
        }
        do {
            try start(options)
        } catch {
            fail(SettingsModel.message(error))
        }
    }

    private func start(_ options: LaunchOptions) throws {
        let keystore: Keystore = options.isolated ? MemoryKeystore() : KeychainKeystore()
        let pasteboard: PasteboardReading & PasteboardWriting = options.isolated ? InMemoryPasteboard() : SystemPasteboard()
        let engine = try SupercyEngine.open(dataDir: options.dataDirectory.path, keystore: keystore, nowMs: clock.nowMs())
        let settings = try engine.settings()
        L10n.language = UILanguage.resolve(settings.language)
        let controller = ClipboardController(engine: engine, pasteboard: pasteboard, frontApp: WorkspaceFrontApp(),
                                             paster: AccessibilityPaster(), scheduler: TimerScheduler(), clock: clock)
        let panel = HistoryPanelController(controller: controller, clock: clock)
        let statusItem = StatusItemController(controller: controller, panel: panel)
        panel.onOpenSettings = { [weak self] in self?.openSettings() }
        panel.onNeedsAccessibility = { [weak self] in self?.explainAccessibility() }
        statusItem.onOpenSettings = { [weak self] in self?.openSettings() }
        controller.onIngest = { [weak statusItem] _ in statusItem?.refresh() }
        controller.onHistoryChange = { [weak statusItem] in statusItem?.refresh() }
        self.controller = controller
        self.panel = panel
        self.statusItem = statusItem
        if !options.isolated {
            let hotKey = HotKey { [weak self] in self?.panel?.toggle(near: self?.statusItem?.button) }
            if let shortcut = Shortcut(text: settings.historyShortcut) { hotKey.register(shortcut) }
            self.hotKey = hotKey
        }
        statusItem.apply(settings)
        controller.start()
    }

    func applicationWillTerminate(_ notification: Notification) {
        controller?.stop()
        if let controller, (try? controller.engine.settings().clearOnQuit) == true {
            _ = try? controller.engine.clear(keepPinned: true)
        }
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        // Opening Supercy again (Finder, Spotlight) shows the settings: the way back when the
        // menu bar icon is hidden.
        openSettings()
        return false
    }

    private func openSettings() {
        guard let controller, let model = try? SettingsModel(engine: controller.engine, system: system) else { return }
        model.onSaved = { [weak self] settings in self?.settingsChanged(settings) }
        let window = settingsWindow ?? NSWindow(contentRect: NSRect(x: 0, y: 0, width: 560, height: 520),
                                                styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = L10n.text(.settings).replacingOccurrences(of: "…", with: "")
        window.contentView = NSHostingView(rootView: SettingsView(model: model))
        window.isReleasedWhenClosed = false
        window.center()
        settingsWindow = window
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    private func settingsChanged(_ settings: Settings) {
        controller?.applyPollInterval(settings.pollIntervalMs)
        statusItem?.apply(settings)
        if let shortcut = Shortcut(text: settings.historyShortcut) { hotKey?.register(shortcut) }
    }

    private func explainAccessibility() {
        let alert = NSAlert()
        alert.messageText = L10n.text(.needsAccessibilityTitle)
        alert.informativeText = L10n.text(.accessibilityExplanation)
        alert.addButton(withTitle: L10n.text(.openSystemSettings))
        alert.addButton(withTitle: L10n.text(.copyOnly))
        NSApp.activate(ignoringOtherApps: true)
        if alert.runModal() == .alertFirstButtonReturn {
            AccessibilityPaster().requestTrust()
            system.openAccessibilitySettings()
        }
    }

    private func fail(_ message: String) {
        let alert = NSAlert()
        alert.messageText = L10n.text(.appName)
        alert.informativeText = message
        alert.runModal()
        NSApp.terminate(nil)
    }
}
