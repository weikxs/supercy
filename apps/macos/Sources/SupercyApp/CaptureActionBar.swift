import AppKit

/// What to do with a finished selection.
enum CaptureAction {
    case copy, save, pin
}

/// The small bar under a selection: Copy, Save, Pin to Screen, Cancel.
final class CaptureActionBar: NSVisualEffectView {
    private static let buttonSide: CGFloat = 30
    private static let spacing: CGFloat = 2
    private static let inset: CGFloat = 4

    var onAction: ((CaptureAction) -> Void)?
    var onCancel: (() -> Void)?

    init() {
        super.init(frame: .zero)
        material = .hudWindow
        blendingMode = .withinWindow
        state = .active
        wantsLayer = true
        layer?.cornerRadius = 8
        let buttons = [
            button("doc.on.doc", .copy, shortcut: "⏎ ⌘C", action: #selector(copyClicked)),
            button("square.and.arrow.down", .save, shortcut: "⌘S", action: #selector(saveClicked)),
            button("pin", .pinToScreen, shortcut: "⌥T", action: #selector(pinClicked)),
            button("xmark", .cancel, shortcut: "Esc", action: #selector(cancelClicked)),
        ]
        var x = Self.inset
        for button in buttons {
            button.frame = NSRect(x: x, y: Self.inset, width: Self.buttonSide, height: Self.buttonSide)
            addSubview(button)
            x += Self.buttonSide + Self.spacing
        }
        setFrameSize(NSSize(width: x - Self.spacing + Self.inset, height: Self.buttonSide + 2 * Self.inset))
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    private func button(_ symbol: String, _ title: L10nKey, shortcut: String, action: Selector) -> NSButton {
        let label = L10n.text(title).replacingOccurrences(of: "…", with: "")
        let image = NSImage(systemSymbolName: symbol, accessibilityDescription: label) ?? NSImage()
        let button = NSButton(image: image, target: self, action: action)
        button.isBordered = false
        button.contentTintColor = .white
        button.toolTip = "\(label) (\(shortcut))"
        button.setAccessibilityLabel(label)
        return button
    }

    @objc private func copyClicked() { onAction?(.copy) }
    @objc private func saveClicked() { onAction?(.save) }
    @objc private func pinClicked() { onAction?(.pin) }
    @objc private func cancelClicked() { onCancel?() }
}
