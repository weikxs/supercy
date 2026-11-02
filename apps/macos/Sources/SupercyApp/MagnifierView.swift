import AppKit

/// The loupe next to the pointer: the pixels around it enlarged with a grid, the pixel's
/// coordinates and colour, and the colour keys. It reads the frozen screenshot, never the
/// screen; it never takes the mouse.
final class MagnifierView: NSView {
    private static let infoHeight: CGFloat = 50
    private static let padding: CGFloat = 6
    static var size: NSSize { NSSize(width: Magnifier.lensSide, height: Magnifier.lensSide + infoHeight) }

    private var image: CGImage?
    private var pixel: (x: Int, y: Int) = (0, 0)
    private(set) var color: PixelColor?

    init() {
        super.init(frame: NSRect(origin: .zero, size: Self.size))
        wantsLayer = true
        layer?.cornerRadius = 6
        layer?.masksToBounds = true
        layer?.borderWidth = 1
        layer?.borderColor = NSColor.white.withAlphaComponent(0.8).cgColor
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    /// Shows pixel `pixel` of `image`.
    func show(_ image: CGImage, pixel: (x: Int, y: Int)) {
        self.image = image
        self.pixel = pixel
        color = CaptureImage.color(of: image, x: pixel.x, y: pixel.y)
        needsDisplay = true
    }

    /// Lets go of the screenshot.
    func clear() {
        image = nil
        color = nil
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        NSColor.black.withAlphaComponent(0.85).setFill()
        bounds.fill()
        let lens = NSRect(x: 0, y: Self.infoHeight, width: Magnifier.lensSide, height: Magnifier.lensSide)
        drawPixels(in: lens, context: context)
        drawGrid(in: lens)
        drawInfo()
    }

    private func drawPixels(in lens: NSRect, context: CGContext) {
        guard let image else { return }
        let wanted = Magnifier.sampleRect(around: pixel)
        let available = wanted.intersection(CGRect(x: 0, y: 0, width: image.width, height: image.height))
        guard !available.isNull, let part = image.cropping(to: available) else { return }
        // Image rows go down, the view's y goes up: place the part by its offset from the top.
        let x = lens.minX + (available.minX - wanted.minX) * Magnifier.zoom
        let top = lens.maxY - (available.minY - wanted.minY) * Magnifier.zoom
        let target = CGRect(x: x, y: top - available.height * Magnifier.zoom,
                            width: available.width * Magnifier.zoom, height: available.height * Magnifier.zoom)
        context.saveGState()
        context.interpolationQuality = .none
        context.draw(part, in: target)
        context.restoreGState()
    }

    private func drawGrid(in lens: NSRect) {
        let grid = NSBezierPath()
        let cells = 2 * Magnifier.radius + 1
        for index in 1..<cells {
            let offset = CGFloat(index) * Magnifier.zoom
            grid.move(to: NSPoint(x: lens.minX + offset, y: lens.minY))
            grid.line(to: NSPoint(x: lens.minX + offset, y: lens.maxY))
            grid.move(to: NSPoint(x: lens.minX, y: lens.minY + offset))
            grid.line(to: NSPoint(x: lens.maxX, y: lens.minY + offset))
        }
        NSColor.white.withAlphaComponent(0.12).setStroke()
        grid.lineWidth = 0.5
        grid.stroke()
        let centre = NSRect(x: lens.minX + CGFloat(Magnifier.radius) * Magnifier.zoom,
                            y: lens.minY + CGFloat(Magnifier.radius) * Magnifier.zoom,
                            width: Magnifier.zoom, height: Magnifier.zoom)
        NSColor.white.setStroke()
        let mark = NSBezierPath(rect: centre.insetBy(dx: 0.5, dy: 0.5))
        mark.lineWidth = 1
        mark.stroke()
    }

    private func drawInfo() {
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 10, weight: .regular),
            .foregroundColor: NSColor.white,
        ]
        let padding = Self.padding
        let swatch = NSRect(x: padding, y: Self.infoHeight - 30, width: 10, height: 10)
        ("\(pixel.x), \(pixel.y)" as NSString).draw(at: NSPoint(x: padding, y: Self.infoHeight - 17), withAttributes: attributes)
        if let color {
            NSColor(srgbRed: CGFloat(color.red) / 255, green: CGFloat(color.green) / 255,
                    blue: CGFloat(color.blue) / 255, alpha: 1).setFill()
            swatch.fill()
            (color.hex as NSString).draw(at: NSPoint(x: swatch.maxX + 4, y: swatch.minY - 2), withAttributes: attributes)
        }
        var hint = attributes
        hint[.foregroundColor] = NSColor.white.withAlphaComponent(0.6)
        hint[.font] = NSFont.systemFont(ofSize: 9)
        (L10n.text(.colorHint) as NSString).draw(at: NSPoint(x: padding, y: 4), withAttributes: hint)
    }
}
