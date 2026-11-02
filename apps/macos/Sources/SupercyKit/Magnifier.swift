import CoreGraphics

/// The loupe shown while selecting: a few pixels around the pointer, enlarged, with a grid,
/// the pointer's pixel coordinates and its colour. Pure arithmetic; the app draws it.
public enum Magnifier {
    /// Pixels shown on each side of the centre pixel (so 15 × 15 pixels in all).
    public static let radius = 7
    /// Points per shown pixel.
    public static let zoom: CGFloat = 10
    /// Distance between the pointer and the loupe, in points.
    public static let gap: CGFloat = 20

    /// The loupe's image area, in points.
    public static var lensSide: CGFloat { CGFloat(2 * radius + 1) * zoom }

    /// The pixel under `point` (display space) in a display image of `width` × `height` pixels
    /// at `scale` pixels per point, kept inside the image.
    public static func pixel(at point: CGPoint, scale: CGFloat, width: Int, height: Int) -> (x: Int, y: Int) {
        let x = Int((point.x * scale).rounded(.down))
        let y = Int((point.y * scale).rounded(.down))
        return (min(max(x, 0), max(width - 1, 0)), min(max(y, 0), max(height - 1, 0)))
    }

    /// The square of image pixels the loupe shows around `pixel` (may reach past the image's
    /// edges; the part outside is drawn empty).
    public static func sampleRect(around pixel: (x: Int, y: Int)) -> CGRect {
        CGRect(x: pixel.x - radius, y: pixel.y - radius, width: 2 * radius + 1, height: 2 * radius + 1)
    }

    /// Where a loupe of `size` goes (its top-left corner, display space): below and to the right
    /// of `cursor`, moved to the other side when it would leave `bounds`.
    public static func origin(near cursor: CGPoint, size: CGSize, bounds: CGRect) -> CGPoint {
        var x = cursor.x + gap
        var y = cursor.y + gap
        if x + size.width > bounds.maxX { x = cursor.x - gap - size.width }
        if y + size.height > bounds.maxY { y = cursor.y - gap - size.height }
        return CGPoint(x: max(bounds.minX, x), y: max(bounds.minY, y))
    }
}

/// An sRGB colour with 8-bit channels, as the colour picker copies it.
public struct PixelColor: Equatable {
    public let red: UInt8
    public let green: UInt8
    public let blue: UInt8

    public init(red: UInt8, green: UInt8, blue: UInt8) {
        self.red = red
        self.green = green
        self.blue = blue
    }

    /// `#1A2B3C`.
    public var hex: String {
        "#" + [red, green, blue].map { byte in
            let digits = String(byte, radix: 16, uppercase: true)
            return digits.count == 1 ? "0" + digits : digits
        }.joined()
    }

    /// `rgb(26, 43, 60)`.
    public var rgb: String { "rgb(\(red), \(green), \(blue))" }
}
