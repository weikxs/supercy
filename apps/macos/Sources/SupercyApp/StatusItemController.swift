import AppKit

/// The menu bar icon: click opens the history; right-click or ⌥-click opens the menu with
/// pause, ignore-next, settings and quit.
final class StatusItemController: NSObject {
    private let controller: ClipboardController
    private let panel: HistoryPanelController
    private var item: NSStatusItem?
    private var showRecent = false
    private var style: MenuIcon = .clipboard
    /// Shows the trial notice at the top of the menu.
    var isTrial = false
    var onOpenSettings: (() -> Void)?
    /// Screenshots and pins; `nil` where they are not available (isolated mode).
    var capture: CaptureCoordinator?
    /// The screenshot shortcut, shown next to its menu item.
    var captureShortcut: Shortcut?

    init(controller: ClipboardController, panel: HistoryPanelController) {
        self.controller = controller
        self.panel = panel
    }

    var button: NSStatusBarButton? { item?.button }

    func apply(_ settings: Settings) {
        showRecent = settings.showRecentInMenuBar
        style = settings.menuIcon
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
        let description = L10n.text(paused ? .paused : .appName)
        switch MenuBarIconStyle.image(for: style, paused: paused) {
        case let .asset(name):
            let image = NSImage(named: name)
            image?.isTemplate = true
            image?.accessibilityDescription = description
            button.image = image
        case let .symbol(name):
            button.image = NSImage(systemSymbolName: name, accessibilityDescription: description)
        }
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
        if isTrial {
            let notice = NSMenuItem(title: L10n.text(.trialMode), action: nil, keyEquivalent: "")
            notice.isEnabled = false
            menu.addItem(notice)
            menu.addItem(.separator())
        }
        menu.addItem(menuItem(.openHistory, #selector(openHistory)))
        if let capture { addCaptureItems(to: menu, capture: capture) }
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

    private func addCaptureItems(to menu: NSMenu, capture: CaptureCoordinator) {
        menu.addItem(.separator())
        let take = menuItem(.captureScreen, #selector(captureScreen))
        if let shortcut = captureShortcut {
            take.keyEquivalent = shortcut.key.count == 1 ? shortcut.key : ""
            take.keyEquivalentModifierMask = NSEvent.ModifierFlags(shortcutModifiers: shortcut.modifiers)
        }
        menu.addItem(take)
        let delayed = NSMenuItem(title: L10n.text(.captureDelayed), action: nil, keyEquivalent: "")
        let delays = NSMenu()
        for seconds in CaptureDelay.seconds {
            let item = NSMenuItem(title: String(format: L10n.text(.captureInSeconds), seconds),
                                  action: #selector(captureDelayed(_:)), keyEquivalent: "")
            item.target = self
            item.tag = seconds
            delays.addItem(item)
        }
        delayed.submenu = delays
        menu.addItem(delayed)
        let repeatItem = menuItem(.repeatLastRegion, #selector(repeatLastRegion))
        // Without an action the (auto-enabling) menu shows it disabled.
        if !capture.canRepeatLastRegion { repeatItem.action = nil }
        menu.addItem(repeatItem)
        guard capture.pins.count > 0 else { return }
        let through = menuItem(.pinsClickThrough, #selector(togglePinsClickThrough))
        through.state = capture.pins.isAnyClickThrough ? .on : .off
        menu.addItem(through)
        menu.addItem(menuItem(.closeAllPins, #selector(closeAllPins)))
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
    @objc private func captureScreen() { capture?.start(fromMenu: true) }
    @objc private func captureDelayed(_ sender: NSMenuItem) { capture?.start(delay: sender.tag, fromMenu: true) }
    @objc private func repeatLastRegion() { capture?.start(repeatLast: true, fromMenu: true) }
    @objc private func closeAllPins() { capture?.pins.closeAll() }
    @objc private func togglePinsClickThrough() {
        guard let pins = capture?.pins else { return }
        pins.setClickThrough(!pins.isAnyClickThrough)
    }
    @objc private func quit() { NSApp.terminate(nil) }
}

extension NSEvent.ModifierFlags {
    /// The flags of a shortcut's modifiers.
    init(shortcutModifiers modifiers: Set<Shortcut.Modifier>) {
        self = []
        if modifiers.contains(.ctrl) { insert(.control) }
        if modifiers.contains(.opt) { insert(.option) }
        if modifiers.contains(.shift) { insert(.shift) }
        if modifiers.contains(.cmd) { insert(.command) }
    }
}
