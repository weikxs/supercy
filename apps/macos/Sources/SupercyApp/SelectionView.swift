import AppKit
import QuartzCore

/// Tells the capture session what happened on one display's overlay.
protocol SelectionViewDelegate: AnyObject {
    /// A new selection started here: the other displays drop theirs.
    func selectionViewDidStartSelecting(_ view: SelectionView)
    func selectionView(_ view: SelectionView, didChoose action: CaptureAction)
    /// The colour under the pointer, as text to copy.
    func selectionView(_ view: SelectionView, didPickColor text: String)
    func selectionViewDidCancel(_ view: SelectionView)
}

/// One display's overlay: the frozen screenshot, dimmed outside the selection, with the
/// highlight, handles, size, loupe and action bar. The selection logic is `SelectionState`;
/// this view only turns events into its calls and draws the result. Drawn with layers, so
/// moving the pointer never redraws the full-size screenshot.
final class SelectionView: NSView {
    let shot: DisplayShot
    weak var delegate: SelectionViewDelegate?
    private(set) var state: SelectionState
    private let canvas = PassThroughView()
    private let imageLayer = CALayer()
    private let dimLayer = CAShapeLayer()
    private let borderLayer = CAShapeLayer()
    private let handlesLayer = CAShapeLayer()
    private let sizeLabel = SelectionView.makeLabel()
    private let hintLabel = SelectionView.makeLabel()
    private let magnifier = MagnifierView()
    private let actionBar = CaptureActionBar()
    private var pointer: CGPoint?

    private static let handleSide: CGFloat = 7
    private static let labelGap: CGFloat = 6
    private static let dimAlpha: CGFloat = 0.45

    init(shot: DisplayShot, preset: CGRect?) {
        self.shot = shot
        state = SelectionState(bounds: shot.bounds, windows: shot.windows, selection: preset)
        super.init(frame: NSRect(origin: .zero, size: shot.screen.frame.size))
        buildLayers()
        hintLabel.stringValue = " " + L10n.text(.captureHint) + " "
        hintLabel.sizeToFit()
        for view in [sizeLabel, hintLabel, magnifier, actionBar] as [NSView] {
            view.isHidden = true
            addSubview(view)
        }
        actionBar.onAction = { [weak self] action in self?.choose(action) }
        actionBar.onCancel = { [weak self] in
            guard let self else { return }
            self.delegate?.selectionViewDidCancel(self)
        }
        render()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    private func buildLayers() {
        // A layer-hosting canvas (its layer tree is ours) under the labels and buttons.
        canvas.frame = bounds
        canvas.layer = CALayer()
        canvas.wantsLayer = true
        addSubview(canvas)
        let scale = shot.screen.backingScaleFactor
        imageLayer.contents = shot.image
        imageLayer.contentsGravity = .resize
        imageLayer.frame = bounds
        dimLayer.fillRule = .evenOdd
        dimLayer.fillColor = NSColor.black.withAlphaComponent(Self.dimAlpha).cgColor
        borderLayer.fillColor = nil
        borderLayer.strokeColor = NSColor.controlAccentColor.cgColor
        handlesLayer.fillColor = NSColor.white.cgColor
        handlesLayer.strokeColor = NSColor.controlAccentColor.cgColor
        handlesLayer.lineWidth = 1
        for layer in [imageLayer, dimLayer, borderLayer, handlesLayer] {
            layer.contentsScale = scale
            layer.frame = bounds
            canvas.layer?.addSublayer(layer)
        }
    }

    private static func makeLabel() -> NSTextField {
        let label = PassThroughLabel(labelWithString: "")
        label.font = .monospacedDigitSystemFont(ofSize: 11, weight: .medium)
        label.textColor = .white
        label.drawsBackground = true
        label.backgroundColor = NSColor.black.withAlphaComponent(0.7)
        label.wantsLayer = true
        label.layer?.cornerRadius = 4
        label.layer?.masksToBounds = true
        return label
    }

    /// Lets go of the screenshot (the window is closing).
    func releaseImage() {
        imageLayer.contents = nil
        magnifier.clear()
    }

    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        // Always active: the overlays of the other displays follow the pointer too.
        addTrackingArea(NSTrackingArea(rect: bounds, options: [.mouseMoved, .mouseEnteredAndExited, .activeAlways,
                                                                .inVisibleRect, .cursorUpdate],
                                       owner: self, userInfo: nil))
    }

    // MARK: Events

    /// The event's location in display space.
    private func location(_ event: NSEvent) -> CGPoint {
        CaptureGeometry.flipped(convert(event.locationInWindow, from: nil), height: bounds.height)
    }

    /// Drops this display's selection (another display started one).
    func reset() {
        update(state.cleared())
    }

    override func mouseEntered(with event: NSEvent) {
        window?.makeKey()
        pointer = location(event)
        update(state.hovering(at: location(event)))
    }

    override func mouseExited(with event: NSEvent) {
        pointer = nil
        render()
    }

    override func mouseMoved(with event: NSEvent) {
        pointer = location(event)
        update(state.hovering(at: location(event)))
        updateCursor()
    }

    override func mouseDown(with event: NSEvent) {
        let point = location(event)
        pointer = point
        if state.handle(at: point) == nil { delegate?.selectionViewDidStartSelecting(self) }
        // A double-click inside the selection copies it, as Return does.
        if event.clickCount == 2, state.hasSelection, state.handle(at: point) == .inside {
            choose(.copy)
            return
        }
        update(state.pressed(at: point))
    }

    override func mouseDragged(with event: NSEvent) {
        pointer = location(event)
        update(state.dragged(to: location(event)))
    }

    override func mouseUp(with event: NSEvent) {
        pointer = location(event)
        update(state.released(at: location(event)))
        updateCursor()
    }

    /// Right-click: drop the selection, or leave when there is none.
    override func rightMouseDown(with event: NSEvent) {
        if state.hasSelection {
            update(state.cleared().hovering(at: location(event)))
        } else {
            delegate?.selectionViewDidCancel(self)
        }
    }

    override func cursorUpdate(with event: NSEvent) {
        updateCursor()
    }

    private func updateCursor() {
        guard let pointer else { return }
        switch state.handle(at: pointer) {
        case .inside?: NSCursor.openHand.set()
        case .left?, .right?: NSCursor.resizeLeftRight.set()
        case .top?, .bottom?: NSCursor.resizeUpDown.set()
        default: NSCursor.crosshair.set()
        }
    }

    override func keyDown(with event: NSEvent) {
        if !handleKey(event) { super.keyDown(with: event) }
    }

    private enum Key {
        static let returnKey: UInt16 = 36, enter: UInt16 = 76, escape: UInt16 = 53, space: UInt16 = 49
        static let a: UInt16 = 0, c: UInt16 = 8, s: UInt16 = 1, t: UInt16 = 17
        static let left: UInt16 = 123, right: UInt16 = 124, down: UInt16 = 125, up: UInt16 = 126
    }

    /// The overlay's keys; returns whether the key was used.
    private func handleKey(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection([.command, .option, .shift, .control])
        switch (event.keyCode, flags) {
        case (Key.escape, _): delegate?.selectionViewDidCancel(self)
        case (Key.returnKey, []), (Key.enter, []), (Key.c, .command): if state.hasSelection { choose(.copy) }
        case (Key.s, .command): if state.hasSelection { choose(.save) }
        case (Key.t, .option): if state.hasSelection { choose(.pin) }
        case (Key.space, []), (Key.a, .command):
            delegate?.selectionViewDidStartSelecting(self)
            update(state.selectingAll())
        case (Key.c, []): pickColor(rgb: false)
        case (Key.c, .shift): pickColor(rgb: true)
        case (Key.left, _), (Key.right, _), (Key.up, _), (Key.down, _): nudge(event.keyCode, flags: flags)
        default: return false
        }
        return true
    }

    private func nudge(_ keyCode: UInt16, flags: NSEvent.ModifierFlags) {
        let direction: NudgeDirection
        switch keyCode {
        case Key.left: direction = .left
        case Key.right: direction = .right
        case Key.up: direction = .up
        default: direction = .down
        }
        update(state.nudged(direction, resize: flags.contains(.shift), large: flags.contains(.option)))
    }

    private func pickColor(rgb: Bool) {
        guard let color = magnifier.color else { return }
        delegate?.selectionView(self, didPickColor: rgb ? color.rgb : color.hex)
    }

    private func choose(_ action: CaptureAction) {
        guard state.hasSelection else { return }
        delegate?.selectionView(self, didChoose: action)
    }

    // MARK: Drawing

    private func update(_ next: SelectionState) {
        guard next != state else {
            renderLoupe()
            return
        }
        state = next
        render()
    }

    private func render() {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        let height = bounds.height
        let selection = state.selection.map { CaptureGeometry.flipped($0, height: height) }
        let highlight = state.highlight.map { CaptureGeometry.flipped($0, height: height) }
        // A hovered window shows undimmed; the whole display stays dimmed, with only a border.
        let clear = selection ?? (state.highlight == state.bounds ? nil : highlight)
        let dim = CGMutablePath()
        dim.addRect(bounds)
        if let clear { dim.addRect(clear) }
        dimLayer.path = dim
        let outline = selection ?? highlight
        borderLayer.lineWidth = selection == nil ? 3 : 1
        borderLayer.path = outline.map { CGPath(rect: $0.insetBy(dx: borderLayer.lineWidth / 2, dy: borderLayer.lineWidth / 2),
                                                transform: nil) }
        handlesLayer.path = state.hasSelection ? selection.map(handlesPath) : nil
        CATransaction.commit()
        renderSize(selection ?? highlight)
        renderActions(selection)
        hintLabel.isHidden = state.phase != .choosing || pointer == nil
        hintLabel.setFrameOrigin(NSPoint(x: (bounds.width - hintLabel.frame.width) / 2, y: bounds.height - 60))
        renderLoupe()
    }

    private func handlesPath(_ rect: CGRect) -> CGPath {
        let side = Self.handleSide
        let xs = [rect.minX, rect.midX, rect.maxX], ys = [rect.minY, rect.midY, rect.maxY]
        let path = CGMutablePath()
        for x in xs {
            for y in ys where !(x == rect.midX && y == rect.midY) {
                path.addRect(CGRect(x: x - side / 2, y: y - side / 2, width: side, height: side))
            }
        }
        return path
    }

    /// The size in pixels, above the rectangle (inside it when there is no room).
    private func renderSize(_ rect: CGRect?) {
        guard let rect, pointer != nil || state.hasSelection else {
            sizeLabel.isHidden = true
            return
        }
        let width = Int((rect.width * shot.scale).rounded()), height = Int((rect.height * shot.scale).rounded())
        sizeLabel.stringValue = " \(width) × \(height) "
        sizeLabel.sizeToFit()
        let above = rect.maxY + Self.labelGap
        let y = above + sizeLabel.frame.height <= bounds.height ? above : rect.maxY - sizeLabel.frame.height - Self.labelGap
        sizeLabel.setFrameOrigin(NSPoint(x: max(0, rect.minX), y: y))
        sizeLabel.isHidden = false
    }

    /// The action bar under the selection's right end (above, or inside, when there is no room).
    private func renderActions(_ selection: CGRect?) {
        guard state.showsActions, let selection else {
            actionBar.isHidden = true
            return
        }
        let size = actionBar.frame.size
        let x = min(max(0, selection.maxX - size.width), bounds.width - size.width)
        let below = selection.minY - Self.labelGap - size.height
        let above = selection.maxY + Self.labelGap + sizeLabel.frame.height + Self.labelGap
        let y: CGFloat
        if below >= 0 {
            y = below
        } else if above + size.height <= bounds.height {
            y = above
        } else {
            y = selection.minY + Self.labelGap
        }
        actionBar.setFrameOrigin(NSPoint(x: x, y: y))
        actionBar.isHidden = false
    }

    /// The loupe follows the pointer while choosing or dragging, not over a finished selection.
    private func renderLoupe() {
        guard let pointer, !state.hasSelection else {
            magnifier.isHidden = true
            return
        }
        let pixel = Magnifier.pixel(at: pointer, scale: shot.scale, width: shot.image.width, height: shot.image.height)
        magnifier.show(shot.image, pixel: pixel)
        let origin = Magnifier.origin(near: pointer, size: MagnifierView.size, bounds: shot.bounds)
        let frame = CGRect(origin: origin, size: MagnifierView.size)
        magnifier.frame = CaptureGeometry.flipped(frame, height: bounds.height)
        magnifier.isHidden = false
    }
}

/// A view that never takes the mouse: clicks go to the overlay underneath.
private final class PassThroughView: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

/// A label that never takes the mouse.
private final class PassThroughLabel: NSTextField {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}
