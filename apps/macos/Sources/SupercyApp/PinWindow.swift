import AppKit
import QuartzCore

/// A panel that takes the keyboard without activating Supercy.
final class PinPanel: NSPanel {
    override var canBecomeKey: Bool { true }
}

/// What a pin asks of its owner.
protocol PinWindowDelegate: AnyObject {
    func pinWantsCopy(_ pin: PinWindow)
    func pinWantsSave(_ pin: PinWindow)
    func pinDidClose(_ pin: PinWindow)
    func pinWantsCloseAll(_ pin: PinWindow)
}

/// One image pinned to the screen: floating, borderless, dragged by its body. Scroll or pinch
/// to zoom, ⌥-scroll for opacity, double-click or Esc to close, right-click for the menu. It
/// keeps only its bitmap at display size (NFR-14), released when it closes.
final class PinWindow: NSObject {
    /// The pinned bitmap; `nil` once closed.
    private(set) var image: CGImage?
    /// Image pixels per point at 100 %.
    let scale: CGFloat
    private(set) var zoom: CGFloat = 1
    private(set) var opacity: CGFloat = 1
    weak var delegate: PinWindowDelegate?
    private var panel: PinPanel?
    private static let opacities: [CGFloat] = [1, 0.8, 0.6, 0.4, 0.2]

    /// Shows `image` (taken at `scale` pixels per point) with its top-left corner where
    /// `frame` (AppKit coordinates) has it.
    init(image: CGImage, scale: CGFloat, frame: CGRect) {
        self.image = image
        self.scale = scale
        super.init()
        let size = PinMetrics.size(pixels: CGSize(width: image.width, height: image.height), scale: scale, zoom: 1)
        let rect = CGRect(x: frame.minX, y: frame.maxY - size.height, width: size.width, height: size.height)
        let panel = PinPanel(contentRect: rect, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered,
                             defer: false)
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.hasShadow = true
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.animationBehavior = .none
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        let view = PinView(frame: NSRect(origin: .zero, size: rect.size))
        view.pin = self
        view.needsDisplay = true
        panel.contentView = view
        self.panel = panel
        panel.orderFrontRegardless()
    }

    var isClickThrough: Bool { panel?.ignoresMouseEvents ?? false }

    func setClickThrough(_ on: Bool) {
        panel?.ignoresMouseEvents = on
    }

    func close() {
        guard let panel else { return }
        image = nil
        panel.contentView?.layer?.contents = nil
        panel.orderOut(nil)
        panel.contentView = nil
        panel.close()
        self.panel = nil
        delegate?.pinDidClose(self)
    }

    // MARK: Zoom and opacity

    func setZoom(_ newZoom: CGFloat, anchor: CGPoint? = nil) {
        guard let panel, let image else { return }
        let clamped = PinMetrics.clampedZoom(newZoom)
        guard clamped != zoom else { return }
        let frame = panel.frame
        let size = PinMetrics.size(pixels: CGSize(width: image.width, height: image.height), scale: scale, zoom: clamped)
        let point = anchor ?? CGPoint(x: frame.minX, y: frame.maxY)
        var next = PinMetrics.zoomedFrame(frame, anchor: point, from: zoom, to: clamped)
        next.size = size
        zoom = clamped
        panel.setFrame(next, display: true)
    }

    func setOpacity(_ value: CGFloat) {
        opacity = PinMetrics.clampedOpacity(value)
        panel?.alphaValue = opacity
    }

    fileprivate func scrolled(_ event: NSEvent) {
        let delta = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY : event.scrollingDeltaY * 10
        if event.modifierFlags.contains(.option) {
            setOpacity(PinMetrics.opacity(opacity, scrolledBy: delta))
        } else {
            setZoom(PinMetrics.zoom(zoom, scrolledBy: delta), anchor: NSEvent.mouseLocation)
        }
    }

    fileprivate func pinched(_ event: NSEvent) {
        setZoom(PinMetrics.zoom(zoom, pinchedBy: event.magnification), anchor: NSEvent.mouseLocation)
    }

    // MARK: Menu

    fileprivate func menu() -> NSMenu {
        let menu = NSMenu()
        menu.addItem(item(.copy, #selector(copyImage)))
        menu.addItem(item(.save, #selector(saveImage)))
        menu.addItem(.separator())
        menu.addItem(item(.actualSize, #selector(actualSize)))
        let opacityItem = NSMenuItem(title: L10n.text(.opacity), action: nil, keyEquivalent: "")
        let opacityMenu = NSMenu()
        for value in Self.opacities {
            let entry = NSMenuItem(title: PinMetrics.percent(value), action: #selector(chooseOpacity(_:)), keyEquivalent: "")
            entry.target = self
            entry.representedObject = NSNumber(value: Double(value))
            entry.state = abs(opacity - value) < 0.01 ? .on : .off
            opacityMenu.addItem(entry)
        }
        opacityItem.submenu = opacityMenu
        menu.addItem(opacityItem)
        let through = item(.pinsClickThrough, #selector(clickThrough))
        menu.addItem(through)
        menu.addItem(.separator())
        menu.addItem(item(.closePin, #selector(closeClicked)))
        menu.addItem(item(.closeAllPins, #selector(closeAll)))
        return menu
    }

    private func item(_ key: L10nKey, _ action: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: L10n.text(key), action: action, keyEquivalent: "")
        item.target = self
        return item
    }

    @objc fileprivate func copyImage() { delegate?.pinWantsCopy(self) }
    @objc fileprivate func saveImage() { delegate?.pinWantsSave(self) }
    @objc fileprivate func actualSize() { setZoom(1) }
    @objc private func chooseOpacity(_ sender: NSMenuItem) {
        setOpacity(CGFloat((sender.representedObject as? NSNumber)?.doubleValue ?? 1))
    }
    @objc private func clickThrough() { setClickThrough(true) }
    @objc fileprivate func closeClicked() { close() }
    @objc private func closeAll() { delegate?.pinWantsCloseAll(self) }
}

/// The pin's content: the bitmap as a layer, scaled by the window size.
private final class PinView: NSView {
    weak var pin: PinWindow?

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.contentsGravity = .resize
        layer?.magnificationFilter = .nearest
        layer?.borderWidth = 1
        layer?.borderColor = NSColor.black.withAlphaComponent(0.25).cgColor
        autoresizingMask = [.width, .height]
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override var wantsUpdateLayer: Bool { true }

    override func updateLayer() {
        layer?.contents = pin?.image
    }
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func mouseDown(with event: NSEvent) {
        if event.clickCount == 2 {
            pin?.closeClicked()
            return
        }
        window?.makeKey()
        window?.makeFirstResponder(self)
        window?.performDrag(with: event)
    }

    override func scrollWheel(with event: NSEvent) { pin?.scrolled(event) }
    override func magnify(with event: NSEvent) { pin?.pinched(event) }
    override func menu(for event: NSEvent) -> NSMenu? { pin?.menu() }

    override func keyDown(with event: NSEvent) {
        let flags = event.modifierFlags.intersection([.command, .option, .shift, .control])
        switch (event.keyCode, flags) {
        case (53, _), (13, .command): pin?.closeClicked() // Esc, ⌘W
        case (8, .command): pin?.copyImage() // ⌘C
        case (1, .command): pin?.saveImage() // ⌘S
        case (29, .command): pin?.actualSize() // ⌘0
        default: super.keyDown(with: event)
        }
    }
}
