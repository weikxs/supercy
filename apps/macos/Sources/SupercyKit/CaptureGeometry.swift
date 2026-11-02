import CoreGraphics
import Foundation

/// The corners and edges of a selection that can be dragged to resize it, and its inside,
/// which moves it.
public enum SelectionHandle: CaseIterable, Equatable {
    case topLeft, top, topRight, right, bottomRight, bottom, bottomLeft, left, inside
}

/// An arrow key.
public enum NudgeDirection: Equatable {
    case left, right, up, down
}

/// Rectangle arithmetic for the capture overlay. Everything here is in "display space":
/// points relative to one display, origin at its top-left corner, y growing downwards (the
/// way screenshots, window bounds and pixel rows are laid out).
public enum CaptureGeometry {
    /// A selection side smaller than this (in points) counts as a click, not a drag.
    public static let dragThreshold: CGFloat = 3

    /// The rectangle spanned by two corners, in any order.
    public static func rect(from a: CGPoint, to b: CGPoint) -> CGRect {
        CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(a.x - b.x), height: abs(a.y - b.y))
    }

    /// `rect` with its edges on whole points and cut to `bounds`; `nil` when nothing is left.
    public static func snapped(_ rect: CGRect, in bounds: CGRect) -> CGRect? {
        let minX = rect.minX.rounded(), minY = rect.minY.rounded()
        let whole = CGRect(x: minX, y: minY, width: rect.maxX.rounded() - minX, height: rect.maxY.rounded() - minY)
        let cut = whole.intersection(bounds)
        return cut.isNull || cut.width < 1 || cut.height < 1 ? nil : cut
    }

    /// `rect` moved by (`dx`, `dy`) but kept inside `bounds`, size unchanged.
    public static func moved(_ rect: CGRect, dx: CGFloat, dy: CGFloat, in bounds: CGRect) -> CGRect {
        let x = clamp(rect.minX + dx, bounds.minX, max(bounds.minX, bounds.maxX - rect.width))
        let y = clamp(rect.minY + dy, bounds.minY, max(bounds.minY, bounds.maxY - rect.height))
        return CGRect(x: x, y: y, width: rect.width, height: rect.height)
    }

    /// `rect` after dragging `handle` by (`dx`, `dy`): dragging past the opposite edge flips
    /// the rectangle, as in any drawing app. Kept inside `bounds`, at least one point wide.
    public static func resized(_ rect: CGRect, handle: SelectionHandle, dx: CGFloat, dy: CGFloat,
                               in bounds: CGRect) -> CGRect {
        if handle == .inside { return moved(rect, dx: dx, dy: dy, in: bounds) }
        var left = rect.minX, right = rect.maxX, top = rect.minY, bottom = rect.maxY
        switch handle {
        case .topLeft, .left, .bottomLeft: left += dx
        case .topRight, .right, .bottomRight: right += dx
        default: break
        }
        switch handle {
        case .topLeft, .top, .topRight: top += dy
        case .bottomLeft, .bottom, .bottomRight: bottom += dy
        default: break
        }
        let spanned = self.rect(from: CGPoint(x: left, y: top), to: CGPoint(x: right, y: bottom))
        return snapped(spanned, in: bounds) ?? atLeastOnePoint(spanned, in: bounds)
    }

    /// The handle under `point`: corners first, then edges (within `tolerance` points), then the
    /// inside; `nil` outside the selection.
    public static func handle(at point: CGPoint, of rect: CGRect, tolerance: CGFloat) -> SelectionHandle? {
        let nearLeft = abs(point.x - rect.minX) <= tolerance, nearRight = abs(point.x - rect.maxX) <= tolerance
        let nearTop = abs(point.y - rect.minY) <= tolerance, nearBottom = abs(point.y - rect.maxY) <= tolerance
        let withinX = point.x >= rect.minX - tolerance && point.x <= rect.maxX + tolerance
        let withinY = point.y >= rect.minY - tolerance && point.y <= rect.maxY + tolerance
        guard withinX, withinY else { return nil }
        switch (nearLeft, nearRight, nearTop, nearBottom) {
        case (true, _, true, _): return .topLeft
        case (_, true, true, _): return .topRight
        case (true, _, _, true): return .bottomLeft
        case (_, true, _, true): return .bottomRight
        case (true, _, _, _): return .left
        case (_, true, _, _): return .right
        case (_, _, true, _): return .top
        case (_, _, _, true): return .bottom
        default: return rect.contains(point) ? .inside : nil
        }
    }

    /// Arrow keys: move the selection by `step` points, or with `resize` grow or shrink it from
    /// its right and bottom edges. Kept inside `bounds`, at least one point wide.
    public static func nudged(_ rect: CGRect, _ direction: NudgeDirection, resize: Bool, step: CGFloat,
                              in bounds: CGRect) -> CGRect {
        let (dx, dy): (CGFloat, CGFloat)
        switch direction {
        case .left: (dx, dy) = (-step, 0)
        case .right: (dx, dy) = (step, 0)
        case .up: (dx, dy) = (0, -step)
        case .down: (dx, dy) = (0, step)
        }
        guard resize else { return moved(rect, dx: dx, dy: dy, in: bounds) }
        let width = clamp(rect.width + dx, 1, bounds.maxX - rect.minX)
        let height = clamp(rect.height + dy, 1, bounds.maxY - rect.minY)
        return CGRect(x: rect.minX, y: rect.minY, width: width, height: height)
    }

    /// The pixels of a display image (`scale` pixels per point) covered by `rect`, on whole
    /// pixels and inside the image; `nil` when nothing is left.
    public static func pixelRect(_ rect: CGRect, scale: CGFloat, imageWidth: Int, imageHeight: Int) -> CGRect? {
        let scaled = CGRect(x: rect.minX * scale, y: rect.minY * scale, width: rect.width * scale, height: rect.height * scale)
        let image = CGRect(x: 0, y: 0, width: imageWidth, height: imageHeight)
        let cut = scaled.integral.intersection(image)
        return cut.isNull || cut.width < 1 || cut.height < 1 ? nil : cut
    }

    /// A display-space rectangle on the screen whose AppKit frame is `screenFrame` (global,
    /// origin at the bottom-left of the main screen, y growing upwards), in AppKit coordinates.
    public static func appKitRect(_ rect: CGRect, screenFrame: CGRect) -> CGRect {
        CGRect(x: screenFrame.minX + rect.minX, y: screenFrame.maxY - rect.maxY, width: rect.width, height: rect.height)
    }

    /// An AppKit global point in display space of the screen whose frame is `screenFrame`.
    public static func displayPoint(_ point: CGPoint, screenFrame: CGRect) -> CGPoint {
        CGPoint(x: point.x - screenFrame.minX, y: screenFrame.maxY - point.y)
    }

    /// Converts between display space and a view's own space (origin bottom-left) of the same
    /// `height`; the conversion is its own inverse.
    public static func flipped(_ rect: CGRect, height: CGFloat) -> CGRect {
        CGRect(x: rect.minX, y: height - rect.maxY, width: rect.width, height: rect.height)
    }

    /// See `flipped(_:height:)`.
    public static func flipped(_ point: CGPoint, height: CGFloat) -> CGPoint {
        CGPoint(x: point.x, y: height - point.y)
    }

    private static func atLeastOnePoint(_ rect: CGRect, in bounds: CGRect) -> CGRect {
        let x = clamp(rect.minX.rounded(), bounds.minX, bounds.maxX - 1)
        let y = clamp(rect.minY.rounded(), bounds.minY, bounds.maxY - 1)
        let width = clamp(rect.width.rounded(), 1, bounds.maxX - x)
        let height = clamp(rect.height.rounded(), 1, bounds.maxY - y)
        return CGRect(x: x, y: y, width: width, height: height)
    }

    static func clamp(_ value: CGFloat, _ low: CGFloat, _ high: CGFloat) -> CGFloat {
        min(max(value, low), high)
    }
}

/// A window on screen that the overlay can highlight and select.
public struct CaptureWindow: Equatable {
    /// The window server's window number.
    public let id: UInt32
    /// The window's frame in display space (may extend past the display).
    public let frame: CGRect

    public init(id: UInt32, frame: CGRect) {
        self.id = id
        self.frame = frame
    }

    /// Windows smaller than this (points, either side) are not offered for selection.
    public static let minimumSide: CGFloat = 40

    /// The windows of the window server's on-screen list (`CGWindowListCopyWindowInfo`, front
    /// to back) that can be selected on a display whose global bounds (top-left origin, as in
    /// that list) are `displayBounds`: normal windows of other processes, visible, not tiny,
    /// at least partly on the display. Frames are returned in display space, front to back.
    public static func selectable(from list: [[String: Any]], ownProcess: Int32, displayBounds: CGRect) -> [CaptureWindow] {
        list.compactMap { info -> CaptureWindow? in
            guard let number = (info["kCGWindowNumber"] as? NSNumber)?.uint32Value,
                  (info["kCGWindowLayer"] as? NSNumber)?.intValue == 0,
                  (info["kCGWindowOwnerPID"] as? NSNumber)?.int32Value != ownProcess,
                  ((info["kCGWindowAlpha"] as? NSNumber)?.doubleValue ?? 1) > 0,
                  let bounds = info["kCGWindowBounds"] as? [String: Any],
                  let frame = rect(from: bounds),
                  frame.width >= minimumSide, frame.height >= minimumSide,
                  frame.intersects(displayBounds)
            else { return nil }
            return CaptureWindow(id: number, frame: frame.offsetBy(dx: -displayBounds.minX, dy: -displayBounds.minY))
        }
    }

    /// The front-most window under `point` (display space).
    public static func window(at point: CGPoint, in windows: [CaptureWindow]) -> CaptureWindow? {
        windows.first { $0.frame.contains(point) }
    }

    private static func rect(from bounds: [String: Any]) -> CGRect? {
        guard let x = (bounds["X"] as? NSNumber)?.doubleValue, let y = (bounds["Y"] as? NSNumber)?.doubleValue,
              let width = (bounds["Width"] as? NSNumber)?.doubleValue,
              let height = (bounds["Height"] as? NSNumber)?.doubleValue else { return nil }
        return CGRect(x: x, y: y, width: width, height: height)
    }
}
