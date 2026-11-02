import AppKit

/// A borderless window over a whole display, above everything else, for the overlay.
final class OverlayWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}

/// A finished selection: which display, which region (display space), and the window when a
/// click chose a whole window.
struct CaptureSelection {
    let shot: DisplayShot
    let rect: CGRect
    let window: CaptureWindow?
    let action: CaptureAction
}

/// The overlays of one screenshot, one per display. Exists only while selecting; closing it
/// releases the windows and the frozen screenshots.
final class CaptureSession: SelectionViewDelegate {
    private var windows: [OverlayWindow] = []
    private var views: [SelectionView] = []
    /// Called once: with the selection, or with a picked colour, or with nothing (cancelled).
    var onFinish: ((CaptureSessionResult) -> Void)?

    enum CaptureSessionResult {
        case selected(CaptureSelection)
        case color(String)
        case cancelled
    }

    /// Opens the overlays; `preset` (display and region) selects the last region again.
    init(shots: [DisplayShot], preset: (displayID: CGDirectDisplayID, rect: CGRect)?) {
        for shot in shots {
            let rect = preset.flatMap { $0.displayID == shot.displayID ? $0.rect : nil }
            let view = SelectionView(shot: shot, preset: rect)
            view.delegate = self
            let window = OverlayWindow(contentRect: shot.screen.frame, styleMask: .borderless, backing: .buffered,
                                       defer: false)
            window.setFrame(shot.screen.frame, display: false)
            window.level = .screenSaver
            window.isOpaque = true
            window.backgroundColor = .black
            window.hasShadow = false
            window.isReleasedWhenClosed = false
            window.hidesOnDeactivate = false
            window.animationBehavior = .none
            window.acceptsMouseMovedEvents = true
            window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
            window.contentView = view
            windows.append(window)
            views.append(view)
        }
    }

    /// Shows the overlays and gives the keyboard to the one under the pointer.
    func show() {
        NSApp.activate(ignoringOtherApps: true)
        let mouse = NSEvent.mouseLocation
        for window in windows { window.orderFrontRegardless() }
        let key = windows.first { NSMouseInRect(mouse, $0.frame, false) } ?? windows.first
        key?.makeKeyAndOrderFront(nil)
        if let key, let view = key.contentView { key.makeFirstResponder(view) }
    }

    /// Takes the overlays off the screen at once.
    func hide() {
        windows.forEach { $0.orderOut(nil) }
    }

    /// Closes every overlay and lets go of the screenshots.
    func close() {
        for view in views {
            view.delegate = nil
            view.releaseImage()
        }
        for window in windows {
            window.orderOut(nil)
            window.contentView = nil
            window.close()
        }
        views = []
        windows = []
    }

    private func finish(_ result: CaptureSessionResult) {
        let callback = onFinish
        onFinish = nil
        callback?(result)
    }

    // MARK: SelectionViewDelegate

    func selectionViewDidStartSelecting(_ view: SelectionView) {
        for other in views where other !== view && other.state.selection != nil { other.reset() }
    }

    func selectionView(_ view: SelectionView, didChoose action: CaptureAction) {
        guard let rect = view.state.selection else { return }
        finish(.selected(CaptureSelection(shot: view.shot, rect: rect, window: view.state.selectedWindow, action: action)))
    }

    func selectionView(_ view: SelectionView, didPickColor text: String) {
        finish(.color(text))
    }

    func selectionViewDidCancel(_ view: SelectionView) {
        finish(.cancelled)
    }
}
