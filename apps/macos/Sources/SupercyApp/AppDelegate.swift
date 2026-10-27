import AppKit
import SwiftUI

/// Wires everything together according to the launch mode (see `LaunchMode`): the isolated mode
/// never touches the user's clipboard, keychain or shortcuts; the trial reads the real clipboard
/// but keeps everything under /tmp and never uses the keychain or login items.
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    private var controller: ClipboardController?
    private var statusItem: StatusItemController?
    private var panel: HistoryPanelController?
    private var hotKey: HotKey?
    private var settingsWindow: NSWindow?
    private var system: SystemIntegration = MacSystemIntegration()
    private let clock = SystemClock()
    private var measurement: MeasurementScript?
    /// Measurement baseline: keep the settings window and do not hand memory back (isolated
    /// measurement only, to compare against).
    private var baseline = false
    /// One-shot timer that hands freed memory back after a burst of copies.
    private var reliefTimer: Timer?
    private static let reliefDelay: TimeInterval = 1

    func applicationDidFinishLaunching(_ notification: Notification) {
        let parsed = LaunchOptions.parse(
            arguments: CommandLine.arguments,
            environment: ProcessInfo.processInfo.environment,
            defaultDataDirectory: LaunchOptions.standardDataDirectory()
        )
        switch parsed {
        case let .success(options):
            do {
                try start(options)
            } catch {
                fail(SettingsModel.message(error))
            }
        case .failure(.conflictingModes):
            fail("--isolated and --trial cannot be used together.")
        case .failure(.needsTemporaryDataDirectory):
            fail("This mode needs SUPERCY_DATA_DIR under /tmp.")
        case .failure(.trialNeedsTemporaryHome):
            fail("The trial needs HOME (and CFFIXED_USER_HOME) under /tmp; start it with apps/macos/scripts/trial.sh.")
        }
    }

    private func start(_ options: LaunchOptions) throws {
        let mode = options.mode
        let keystore: Keystore = mode.usesLoginKeychain ? KeychainKeystore() : MemoryKeystore()
        let pasteboard: PasteboardReading & PasteboardWriting =
            mode.readsSystemPasteboard ? SystemPasteboard() : InMemoryPasteboard()
        if !mode.allowsLoginItem { system = TrialSystemIntegration(base: system) }
        let engine = try SupercyEngine.open(dataDir: options.dataDirectory.path, keystore: keystore, nowMs: clock.nowMs())
        let settings = try engine.settings()
        L10n.language = UILanguage.resolve(settings.language)
        let controller = ClipboardController(engine: engine, pasteboard: pasteboard, frontApp: WorkspaceFrontApp(),
                                             paster: AccessibilityPaster(), scheduler: TimerScheduler(), clock: clock)
        let panel = HistoryPanelController(controller: controller, clock: clock)
        let statusItem = StatusItemController(controller: controller, panel: panel)
        if mode == .trial { statusItem.trialNotice = L10n.text(.trialMode) }
        panel.onOpenSettings = { [weak self] in self?.openSettings() }
        panel.onNeedsAccessibility = { [weak self] in self?.explainAccessibility() }
        baseline = mode == .isolated && ProcessInfo.processInfo.environment["SUPERCY_MEASURE_BASELINE"] == "1"
        if !baseline { panel.onClosed = { Self.releaseFreedMemory() } }
        statusItem.onOpenSettings = { [weak self] in self?.openSettings() }
        controller.onIngest = { [weak self, weak statusItem] _ in
            statusItem?.refresh()
            self?.scheduleRelief()
        }
        controller.onHistoryChange = { [weak statusItem] in statusItem?.refresh() }
        self.controller = controller
        self.panel = panel
        self.statusItem = statusItem
        if mode.registersGlobalShortcut {
            let hotKey = HotKey { [weak self] in self?.panel?.toggle(near: self?.statusItem?.button) }
            if let shortcut = Shortcut(text: settings.historyShortcut) { hotKey.register(shortcut) }
            self.hotKey = hotKey
        }
        statusItem.apply(settings)
        controller.start()
        if mode == .isolated, ProcessInfo.processInfo.environment["SUPERCY_MEASURE"] == "1",
           let memory = pasteboard as? InMemoryPasteboard {
            panel.offscreen = true
            let script = MeasurementScript(controller: controller, pasteboard: memory, panel: panel,
                                           openSettings: { [weak self] in self?.openSettings(offscreen: true) },
                                           closeSettings: { [weak self] in self?.settingsWindow?.close() })
            measurement = script
            script.run()
        }
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

    /// Creates the settings window on demand; it is released again when closed.
    private func openSettings(offscreen: Bool = false) {
        guard let controller, let model = try? SettingsModel(engine: controller.engine, system: system) else { return }
        model.onSaved = { [weak self] settings in self?.settingsChanged(settings) }
        let window = settingsWindow ?? NSWindow(contentRect: NSRect(x: 0, y: 0, width: 560, height: 520),
                                                styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = L10n.text(.settings).replacingOccurrences(of: "…", with: "")
        window.contentView = NSHostingView(rootView: SettingsView(model: model))
        window.isReleasedWhenClosed = false
        window.delegate = self
        settingsWindow = window
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
        guard !baseline, let window = notification.object as? NSWindow, window === settingsWindow else { return }
        window.contentView = nil
        window.delegate = nil
        settingsWindow = nil
        DispatchQueue.main.async { Self.releaseFreedMemory() }
    }

    /// After copies stop for a moment, hands freed memory back (no timer runs otherwise).
    private func scheduleRelief() {
        guard !baseline else { return }
        reliefTimer?.invalidate()
        reliefTimer = Timer.scheduledTimer(withTimeInterval: Self.reliefDelay, repeats: false) { [weak self] _ in
            self?.reliefTimer = nil
            Self.releaseFreedMemory()
        }
        reliefTimer?.tolerance = 0.5
    }

    /// Returns freed heap pages to the system after a window closed, so the idle footprint
    /// goes back down instead of keeping the peak.
    static func releaseFreedMemory() {
        malloc_zone_pressure_relief(nil, 0)
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
