import AppKit

/// The menu bar icon: click opens the history; right-click or ⌥-click opens the menu with
/// pause, ignore-next, settings and quit.
final class StatusItemController: NSObject {
    private let controller: ClipboardController
    private let panel: HistoryPanelController
    private var item: NSStatusItem?
    private var showRecent = false
    var onOpenSettings: (() -> Void)?

    init(controller: ClipboardController, panel: HistoryPanelController) {
        self.controller = controller
        self.panel = panel
    }

    var button: NSStatusBarButton? { item?.button }

    func apply(_ settings: Settings) {
        showRecent = settings.showRecentInMenuBar
        if settings.showMenuIcon, item == nil {
            let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
            item.button?.target = self
            item.button?.action = #selector(clicked(_:))
            item.button?.sendAction(on: [.leftMouseUp, .rightMouseUp])
            self.item = item
        } else if !settings.showMenuIcon, let item {
            NSStatusBar.system.removeStatusItem(item)
            self.item = nil
        }
        refresh()
    }

    /// Updates the icon (paused or not) and the latest-copy title.
    func refresh() {
        guard let button = item?.button else { return }
        let paused = controller.isPaused
        let symbol = paused ? "pause.circle" : "doc.on.clipboard"
        button.image = NSImage(systemSymbolName: symbol, accessibilityDescription: L10n.text(paused ? .paused : .appName))
        button.imagePosition = .imageLeading
        if showRecent, let latest = try? controller.engine.list(kind: nil, offset: 0, limit: 1).first {
            button.title = " " + String(PreviewFormatter.title(latest.preview, showSpecialChars: false).prefix(24))
        } else {
            button.title = ""
        }
    }

    @objc private func clicked(_ sender: NSStatusBarButton) {
        let event = NSApp.currentEvent
        if event?.type == .rightMouseUp || event?.modifierFlags.contains(.option) == true {
            showMenu()
        } else {
            panel.toggle(near: sender)
        }
    }

    private func showMenu() {
        let menu = NSMenu()
        menu.addItem(menuItem(.openHistory, #selector(openHistory)))
        menu.addItem(.separator())
        if controller.isPaused {
            menu.addItem(menuItem(.resumeRecording, #selector(resume)))
        } else {
            let pause = NSMenuItem(title: L10n.text(.pauseRecording), action: nil, keyEquivalent: "")
            let submenu = NSMenu()
            submenu.addItem(menuItem(.pauseFifteenMinutes, #selector(pause15)))
            submenu.addItem(menuItem(.pauseOneHour, #selector(pause60)))
            submenu.addItem(menuItem(.pauseUntilResumed, #selector(pauseOpen)))
            pause.submenu = submenu
            menu.addItem(pause)
        }
        menu.addItem(menuItem(.ignoreNextCopy, #selector(skipNext)))
        menu.addItem(.separator())
        menu.addItem(menuItem(.clearHistory, #selector(clearHistory)))
        menu.addItem(menuItem(.settings, #selector(openSettings)))
        menu.addItem(.separator())
        menu.addItem(menuItem(.quit, #selector(quit)))
        item?.menu = menu
        item?.button?.performClick(nil)
        item?.menu = nil
    }

    private func menuItem(_ key: L10nKey, _ action: Selector) -> NSMenuItem {
        let menuItem = NSMenuItem(title: L10n.text(key), action: action, keyEquivalent: "")
        menuItem.target = self
        return menuItem
    }

    @objc private func openHistory() { panel.show(near: button) }
    @objc private func resume() { controller.resume(); refresh() }
    @objc private func pause15() { controller.pause(minutes: 15); refresh() }
    @objc private func pause60() { controller.pause(minutes: 60); refresh() }
    @objc private func pauseOpen() { controller.pause(minutes: nil); refresh() }
    @objc private func skipNext() { controller.skipNextCopy() }
    @objc private func clearHistory() { _ = try? controller.engine.clear(keepPinned: true); refresh() }
    @objc private func openSettings() { onOpenSettings?() }
    @objc private func quit() { NSApp.terminate(nil) }
}
