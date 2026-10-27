import AppKit
import SwiftUI

/// A panel that takes the keyboard without activating Supercy, so the app you were using
/// stays in front and receives the paste.
final class KeyPanel: NSPanel {
    override var canBecomeKey: Bool { true }
}

/// Shows the history panel and routes its keys to the view model.
final class HistoryPanelController: NSObject, NSWindowDelegate {
    private let controller: ClipboardController
    private let clock: Clock
    private var panel: KeyPanel?
    private var model: HistoryViewModel?
    private var keyMonitor: Any?
    private var lastOrigin: NSPoint?
    var onOpenSettings: (() -> Void)?
    var onNeedsAccessibility: (() -> Void)?
    /// Called after the panel closed and released its views (to hand memory back).
    var onClosed: (() -> Void)?
    /// Places the panel off screen (scripted measurements only).
    var offscreen = false

    init(controller: ClipboardController, clock: Clock) {
        self.controller = controller
        self.clock = clock
    }

    var isShown: Bool { panel?.isVisible ?? false }

    func toggle(near statusButton: NSStatusBarButton?) {
        isShown ? close() : show(near: statusButton)
    }

    func show(near statusButton: NSStatusBarButton?) {
        controller.tick()
        let model = HistoryViewModel(controller: controller)
        model.onClose = { [weak self] in self?.close() }
        model.onOpenSettings = { [weak self] in
            self?.close()
            self?.onOpenSettings?()
        }
        model.onNeedsAccessibility = { [weak self] in self?.onNeedsAccessibility?() }
        let view = HistoryView(model: model, isPaused: controller.isPaused)
        let hosting = NSHostingView(rootView: view)
        let panel = KeyPanel(contentRect: NSRect(origin: .zero, size: hosting.fittingSize),
                             styleMask: [.nonactivatingPanel, .titled, .fullSizeContentView],
                             backing: .buffered, defer: false)
        panel.titleVisibility = .hidden
        panel.titlebarAppearsTransparent = true
        panel.isFloatingPanel = true
        panel.level = .popUpMenu
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.contentView = hosting
        panel.delegate = self
        let origin = offscreen ? NSPoint(x: -20_000, y: -20_000)
            : origin(for: panel.frame.size, statusButton: statusButton, settings: model.settings)
        panel.setFrameOrigin(origin)
        self.panel = panel
        self.model = model
        panel.makeKeyAndOrderFront(nil)
        installKeyMonitor()
    }

    func close() {
        if let panel { lastOrigin = panel.frame.origin }
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        keyMonitor = nil
        panel?.orderOut(nil)
        panel?.contentView = nil
        panel = nil
        // Drop the view model with its previews, thumbnails and cached payloads.
        model = nil
        onClosed?()
    }

    func windowDidResignKey(_ notification: Notification) {
        close()
    }

    private func installKeyMonitor() {
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, let model = self.model, event.window === self.panel else { return event }
            guard let command = KeyCommand.from(keyCode: event.keyCode, modifiers: event.modifierFlags) else { return event }
            return model.perform(command, nowMs: self.clock.nowMs()) ? nil : event
        }
    }

    private func origin(for size: NSSize, statusButton: NSStatusBarButton?, settings: Settings?) -> NSPoint {
        let screen = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let mouse = NSEvent.mouseLocation
        let raw: NSPoint
        switch settings?.popupPosition ?? .cursor {
        case .cursor:
            raw = NSPoint(x: mouse.x, y: mouse.y - size.height)
        case .menuIcon:
            if let frame = statusButton?.window?.frame {
                raw = NSPoint(x: frame.minX, y: frame.minY - size.height)
            } else {
                raw = NSPoint(x: mouse.x, y: mouse.y - size.height)
            }
        case .screenCenter:
            raw = NSPoint(x: screen.midX - size.width / 2, y: screen.midY - size.height / 2)
        case .lastPosition:
            raw = lastOrigin ?? NSPoint(x: screen.midX - size.width / 2, y: screen.midY - size.height / 2)
        }
        let x = min(max(raw.x, screen.minX), screen.maxX - size.width)
        let y = min(max(raw.y, screen.minY), screen.maxY - size.height)
        return NSPoint(x: x, y: y)
    }
}
