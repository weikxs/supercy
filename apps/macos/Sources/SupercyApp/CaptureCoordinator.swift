import AppKit

/// Runs a screenshot from the shortcut or the menu: permission check, optional countdown,
/// frozen displays, overlays, then the output. Between screenshots it holds nothing but the
/// last region (display and rectangle, in memory only): no window, image or timer (NFR-12).
final class CaptureCoordinator {
    private let output: CaptureOutput
    let pins: PinsController
    private var session: CaptureSession?
    private var countdown: CountdownPanel?
    private var isCapturing = false
    private var lastRegion: (displayID: CGDirectDisplayID, rect: CGRect)?
    /// The app in front before the overlays took the keyboard; it gets it back afterwards.
    private var previousApp: NSRunningApplication?
    /// Called when a screenshot ended and its memory can go back (also when a pin closes).
    var onFinished: (() -> Void)?
    /// Lets the menu close before the screen is frozen (seconds).
    private static let menuCloseDelay: TimeInterval = 0.2

    init(output: CaptureOutput, pins: PinsController) {
        self.output = output
        self.pins = pins
    }

    var canRepeatLastRegion: Bool { lastRegion != nil }
    private var isBusy: Bool { session != nil || countdown != nil || isCapturing }

    /// Starts a screenshot, after `delay` seconds when given; with `repeatLast` the last region
    /// is selected at once. `fromMenu` waits for the menu to close first.
    func start(delay: Int = 0, repeatLast: Bool = false, fromMenu: Bool = false) {
        guard !isBusy else { return }
        guard ScreenCapturePermission.isAllowed else {
            explainPermission()
            return
        }
        previousApp = NSWorkspace.shared.frontmostApplication
        if delay > 0 {
            let panel = CountdownPanel(seconds: delay) { [weak self] in
                self?.countdown = nil
                self?.freeze(repeatLast: repeatLast)
            }
            countdown = panel
            panel.start()
        } else if fromMenu {
            isCapturing = true
            DispatchQueue.main.asyncAfter(deadline: .now() + Self.menuCloseDelay) { [weak self] in
                self?.isCapturing = false
                self?.freeze(repeatLast: repeatLast)
            }
        } else {
            freeze(repeatLast: repeatLast)
        }
    }

    /// Takes the frozen screenshots and opens the overlays.
    private func freeze(repeatLast: Bool) {
        guard !isBusy else { return }
        isCapturing = true
        ScreenCapturer.captureDisplays { [weak self] result in
            guard let self else { return }
            self.isCapturing = false
            switch result {
            case let .success(shots):
                let session = CaptureSession(shots: shots, preset: repeatLast ? self.lastRegion : nil)
                session.onFinish = { [weak self] result in self?.finish(result) }
                self.session = session
                session.show()
            case let .failure(failure):
                NSLog("Supercy: screenshot failed: %@", String(describing: failure))
                if ScreenCapturePermission.isAllowed { self.showFailure() } else { self.explainPermission() }
                self.ended()
            }
        }
    }

    private func finish(_ result: CaptureSession.CaptureSessionResult) {
        // Hide the overlays now; close them (and let the frozen screenshots go) once the event
        // that ended the session is done with its view.
        let closing = session
        session = nil
        closing?.hide()
        DispatchQueue.main.async { closing?.close() }
        switch result {
        case .cancelled:
            restoreFocus()
        case let .color(text):
            output.copy(text: text)
            restoreFocus()
        case let .selected(selection):
            lastRegion = (selection.shot.displayID, selection.rect)
            deliver(selection)
        }
        ended()
    }

    private func deliver(_ selection: CaptureSelection) {
        let shot = selection.shot
        let screenFrame = shot.screen.frame
        let frame = CaptureGeometry.appKitRect(selection.rect, screenFrame: screenFrame)
        let pixels = CaptureGeometry.pixelRect(selection.rect, scale: shot.scale, imageWidth: shot.image.width,
                                               imageHeight: shot.image.height)
        guard let crop = pixels.flatMap({ CaptureImage.crop(shot.image, to: $0) }) else {
            showFailure()
            return
        }
        let action = selection.action
        let scale = shot.scale
        if action != .save { restoreFocus() }
        guard let window = selection.window else {
            output.deliver(crop, action: action, frame: frame, scale: scale)
            return
        }
        // A whole window: take it alone (nothing in front of it, no shadow); the crop when it is gone.
        ScreenCapturer.captureWindow(id: window.id) { [weak self] image in
            guard let image else {
                self?.output.deliver(crop, action: action, frame: frame, scale: scale)
                return
            }
            let place = CaptureGeometry.appKitRect(window.frame, screenFrame: screenFrame)
            self?.output.deliver(image, action: action, frame: place, scale: CGFloat(image.width) / max(window.frame.width, 1))
        }
    }

    private func ended() {
        DispatchQueue.main.async { [weak self] in self?.onFinished?() }
    }

    private func restoreFocus() {
        previousApp?.activate()
        previousApp = nil
    }

    private func showFailure() {
        let alert = NSAlert()
        alert.messageText = L10n.text(.captureFailed)
        NSApp.activate(ignoringOtherApps: true)
        alert.runModal()
    }

    /// FR-15.1: why the permission is needed, then System Settings if the user agrees.
    private func explainPermission() {
        let alert = NSAlert()
        alert.messageText = L10n.text(.needsScreenRecordingTitle)
        alert.informativeText = L10n.text(.screenRecordingExplanation)
        alert.addButton(withTitle: L10n.text(.openSystemSettings))
        alert.addButton(withTitle: L10n.text(.cancel))
        NSApp.activate(ignoringOtherApps: true)
        if alert.runModal() == .alertFirstButtonReturn {
            ScreenCapturePermission.openSettings()
        }
    }
}
