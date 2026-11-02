import CoreGraphics

/// What the capture overlay of one display shows and how the mouse and keys change it. A
/// value: every event returns a new state, so the whole interaction is tested without windows.
/// Coordinates are in display space (see `CaptureGeometry`).
public struct SelectionState: Equatable {
    public enum Phase: Equatable {
        /// No selection yet: the pointer highlights the window under it, or the whole display.
        case choosing
        /// The mouse is down and drawing a new region from `anchor`.
        case drawing(anchor: CGPoint)
        /// A selection is made; the action bar is shown.
        case selected
        /// A handle of the selection is being dragged, from `start`, with the selection `original`.
        case adjusting(handle: SelectionHandle, start: CGPoint, original: CGRect)
    }

    /// The display's own rectangle (origin zero, size in points).
    public let bounds: CGRect
    /// Selectable windows, front to back.
    public let windows: [CaptureWindow]
    public let phase: Phase
    /// The selected (or being drawn) region.
    public let selection: CGRect?
    /// The window whose whole frame was chosen with a click, until the selection is changed.
    public let selectedWindow: CaptureWindow?
    /// What a click would select now: the window under the pointer or the whole display.
    public let highlight: CGRect?
    private let hovered: CaptureWindow?

    /// Points the arrow keys move a selection; ⌥ moves by `largeStep`.
    public static let step: CGFloat = 1
    public static let largeStep: CGFloat = 10
    /// How close to an edge (points) the pointer grabs it.
    public static let handleTolerance: CGFloat = 6

    /// A fresh overlay; with `selection` (repeating the last region) it starts selected.
    public init(bounds: CGRect, windows: [CaptureWindow], selection: CGRect? = nil) {
        let preset = selection.flatMap { CaptureGeometry.snapped($0, in: bounds) }
        self.init(bounds: bounds, windows: windows, phase: preset == nil ? .choosing : .selected,
                  selection: preset, selectedWindow: nil, highlight: preset == nil ? bounds : nil, hovered: nil)
    }

    private init(bounds: CGRect, windows: [CaptureWindow], phase: Phase, selection: CGRect?,
                 selectedWindow: CaptureWindow?, highlight: CGRect?, hovered: CaptureWindow?) {
        self.bounds = bounds
        self.windows = windows
        self.phase = phase
        self.selection = selection
        self.selectedWindow = selectedWindow
        self.highlight = highlight
        self.hovered = hovered
    }

    private func with(phase: Phase? = nil, selection: CGRect?? = nil, selectedWindow: CaptureWindow?? = nil,
                      highlight: CGRect?? = nil, hovered: CaptureWindow?? = nil) -> SelectionState {
        SelectionState(bounds: bounds, windows: windows, phase: phase ?? self.phase,
                       selection: selection ?? self.selection, selectedWindow: selectedWindow ?? self.selectedWindow,
                       highlight: highlight ?? self.highlight, hovered: hovered ?? self.hovered)
    }

    public var hasSelection: Bool { phase == .selected && selection != nil }

    /// Whether the action bar should show: a finished selection, not while dragging.
    public var showsActions: Bool { hasSelection }

    /// The pointer moved with no button down.
    public func hovering(at point: CGPoint) -> SelectionState {
        guard phase == .choosing else { return self }
        let window = CaptureWindow.window(at: point, in: windows)
        return with(highlight: .some(area(of: window)), hovered: .some(window))
    }

    /// The part of `window` on this display, or the whole display without a window.
    private func area(of window: CaptureWindow?) -> CGRect {
        window.flatMap { CaptureGeometry.snapped($0.frame, in: bounds) } ?? bounds
    }

    /// The handle under `point`, when a selection is shown.
    public func handle(at point: CGPoint) -> SelectionHandle? {
        guard hasSelection, let selection else { return nil }
        return CaptureGeometry.handle(at: point, of: selection, tolerance: Self.handleTolerance)
    }

    /// The mouse went down: grab a handle of the selection, or start a new region.
    public func pressed(at point: CGPoint) -> SelectionState {
        if let selection, let handle = handle(at: point) {
            return with(phase: .adjusting(handle: handle, start: point, original: selection))
        }
        let base = (phase == .choosing ? self : cleared()).hovering(at: point)
        return base.with(phase: .drawing(anchor: point), selection: .some(nil))
    }

    /// The mouse moved with the button down.
    public func dragged(to point: CGPoint) -> SelectionState {
        switch phase {
        case let .drawing(anchor):
            let spanned = CaptureGeometry.rect(from: anchor, to: point)
            guard max(spanned.width, spanned.height) >= CaptureGeometry.dragThreshold else {
                return with(selection: .some(nil), highlight: .some(area(of: hovered)))
            }
            return with(selection: .some(CaptureGeometry.snapped(spanned, in: bounds)), highlight: .some(nil))
        case let .adjusting(handle, start, original):
            let rect = CaptureGeometry.resized(original, handle: handle, dx: point.x - start.x, dy: point.y - start.y,
                                               in: bounds)
            return with(selection: .some(rect), selectedWindow: .some(nil))
        case .choosing, .selected:
            return self
        }
    }

    /// The mouse went up: a drag selects its region; a click selects the highlighted window or
    /// the whole display.
    public func released(at point: CGPoint) -> SelectionState {
        switch phase {
        case .drawing:
            let dragged = self.dragged(to: point)
            if let region = dragged.selection {
                return dragged.with(phase: .selected, selection: .some(region), selectedWindow: .some(nil))
            }
            return with(phase: .selected, selection: .some(area(of: hovered)), selectedWindow: .some(hovered),
                        highlight: .some(nil))
        case .adjusting:
            return dragged(to: point).with(phase: .selected)
        case .choosing, .selected:
            return self
        }
    }

    /// Arrow keys (with `resize` for ⇧, `large` for ⌥).
    public func nudged(_ direction: NudgeDirection, resize: Bool, large: Bool) -> SelectionState {
        guard hasSelection, let selection else { return self }
        let rect = CaptureGeometry.nudged(selection, direction, resize: resize, step: large ? Self.largeStep : Self.step,
                                          in: bounds)
        return with(selection: .some(rect), selectedWindow: .some(nil))
    }

    /// Selects the whole display.
    public func selectingAll() -> SelectionState {
        with(phase: .selected, selection: .some(bounds), selectedWindow: .some(nil), highlight: .some(nil))
    }

    /// Back to choosing, nothing selected.
    public func cleared() -> SelectionState {
        SelectionState(bounds: bounds, windows: windows)
    }
}
