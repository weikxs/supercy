import CoreGraphics

/// Sizes, zoom and opacity of an image pinned to the screen.
public enum PinMetrics {
    public static let zoomRange: ClosedRange<CGFloat> = 0.1...8
    public static let opacityRange: ClosedRange<CGFloat> = 0.1...1
    /// Zoom change per point of scrolling (exponential, so steps feel even at any size).
    static let zoomPerPoint: CGFloat = 0.01
    /// Opacity change per point of scrolling with ⌥.
    static let opacityPerPoint: CGFloat = 0.01
    /// One scroll event changes at most this many points' worth (a fast flick stays usable).
    static let maxScrollPoints: CGFloat = 25

    public static func clampedZoom(_ zoom: CGFloat) -> CGFloat {
        min(max(zoom, zoomRange.lowerBound), zoomRange.upperBound)
    }

    public static func clampedOpacity(_ opacity: CGFloat) -> CGFloat {
        min(max(opacity, opacityRange.lowerBound), opacityRange.upperBound)
    }

    /// The zoom after scrolling `delta` points (up, positive, enlarges).
    public static func zoom(_ zoom: CGFloat, scrolledBy delta: CGFloat) -> CGFloat {
        let points = min(max(delta, -maxScrollPoints), maxScrollPoints)
        return clampedZoom(zoom * CGFloat(exp(Double(points * zoomPerPoint))))
    }

    /// The zoom after a pinch of `magnification` (as `NSEvent.magnification` reports it).
    public static func zoom(_ zoom: CGFloat, pinchedBy magnification: CGFloat) -> CGFloat {
        clampedZoom(zoom * (1 + magnification))
    }

    /// The opacity after scrolling `delta` points with ⌥ (up, positive, makes it more opaque).
    public static func opacity(_ opacity: CGFloat, scrolledBy delta: CGFloat) -> CGFloat {
        let points = min(max(delta, -maxScrollPoints), maxScrollPoints)
        return clampedOpacity(opacity + points * opacityPerPoint)
    }

    /// The window size, in points, of an image `pixels` large taken at `scale` pixels per point,
    /// shown at `zoom`.
    public static func size(pixels: CGSize, scale: CGFloat, zoom: CGFloat) -> CGSize {
        let factor = zoom / max(scale, 1)
        return CGSize(width: max(1, (pixels.width * factor).rounded()), height: max(1, (pixels.height * factor).rounded()))
    }

    /// `frame` resized from `oldZoom` to `newZoom` so that `anchor` (a point in the same
    /// coordinates, usually the pointer) stays over the same spot of the image.
    public static func zoomedFrame(_ frame: CGRect, anchor: CGPoint, from oldZoom: CGFloat, to newZoom: CGFloat) -> CGRect {
        guard oldZoom > 0 else { return frame }
        let factor = newZoom / oldZoom
        let width = (frame.width * factor).rounded(), height = (frame.height * factor).rounded()
        let x = anchor.x - (anchor.x - frame.minX) * factor
        let y = anchor.y - (anchor.y - frame.minY) * factor
        return CGRect(x: x.rounded(), y: y.rounded(), width: max(1, width), height: max(1, height))
    }

    /// The most pixels worth keeping for a pin of an image `pixels` large taken at `imageScale`,
    /// shown on a screen with `screenScale`: its size at 100 % on that screen, never more than
    /// the image has.
    public static func keptPixels(pixels: CGSize, imageScale: CGFloat, screenScale: CGFloat) -> CGSize {
        let factor = min(1, screenScale / max(imageScale, 1))
        return CGSize(width: max(1, (pixels.width * factor).rounded()), height: max(1, (pixels.height * factor).rounded()))
    }

    /// `80 %`.
    public static func percent(_ value: CGFloat) -> String {
        "\(Int((value * 100).rounded())) %"
    }
}
