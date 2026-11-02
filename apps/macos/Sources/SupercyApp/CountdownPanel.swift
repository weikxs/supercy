import AppKit

/// The few seconds before a timed screenshot: a small number in the middle of the screen.
final class CountdownPanel {
    private var panel: NSPanel?
    private var timer: Timer?
    private var remaining: Int
    private let label = NSTextField(labelWithString: "")
    private let completion: () -> Void
    private static let side: CGFloat = 96
    /// Time for the panel to leave the screen before it is frozen.
    private static let hideDelay: TimeInterval = 0.15

    init(seconds: Int, completion: @escaping () -> Void) {
        remaining = seconds
        self.completion = completion
    }

    func start() {
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main
        let area = screen?.frame ?? NSRect(x: 0, y: 0, width: Self.side, height: Self.side)
        let frame = NSRect(x: area.midX - Self.side / 2, y: area.midY - Self.side / 2, width: Self.side, height: Self.side)
        let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.level = .screenSaver
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.ignoresMouseEvents = true
        panel.isReleasedWhenClosed = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        let background = NSVisualEffectView(frame: NSRect(origin: .zero, size: frame.size))
        background.material = .hudWindow
        background.state = .active
        background.wantsLayer = true
        background.layer?.cornerRadius = 16
        label.font = .monospacedDigitSystemFont(ofSize: 48, weight: .semibold)
        label.textColor = .white
        label.alignment = .center
        label.frame = NSRect(x: 0, y: (Self.side - 60) / 2, width: Self.side, height: 60)
        background.addSubview(label)
        panel.contentView = background
        self.panel = panel
        show()
        panel.orderFrontRegardless()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.tick() }
    }

    private func show() {
        label.stringValue = "\(remaining)"
    }

    private func tick() {
        remaining -= 1
        guard remaining <= 0 else {
            show()
            return
        }
        timer?.invalidate()
        timer = nil
        panel?.orderOut(nil)
        panel?.contentView = nil
        panel?.close()
        panel = nil
        let completion = self.completion
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.hideDelay) { completion() }
    }
}
