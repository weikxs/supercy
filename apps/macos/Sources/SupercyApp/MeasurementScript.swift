import AppKit

/// A fixed sequence for `make macos-measure`, run only in the isolated mode with
/// SUPERCY_MEASURE=1: fill the in-memory history with 500 items, open and close the history,
/// open and close the settings (both off screen), then stay idle for 60 seconds. Each phase is
/// printed so the script outside can sample memory and CPU time at that moment.
final class MeasurementScript {
    private let controller: ClipboardController
    private let pasteboard: InMemoryPasteboard
    private let panel: HistoryPanelController
    private let openSettings: () -> Void
    private let closeSettings: () -> Void
    /// Seconds the outside script gets to sample each phase.
    private static let settle: TimeInterval = 3
    private static let idleSeconds: TimeInterval = 60
    private static let textItems = 480
    private static let imageItems = 20

    init(controller: ClipboardController, pasteboard: InMemoryPasteboard, panel: HistoryPanelController,
         openSettings: @escaping () -> Void, closeSettings: @escaping () -> Void) {
        self.controller = controller
        self.pasteboard = pasteboard
        self.panel = panel
        self.openSettings = openSettings
        self.closeSettings = closeSettings
    }

    func run() {
        let steps: [(String, () -> Void)] = [
            ("idle", {}),
            ("seeded-500", { self.seed() }),
            ("popup-open", { self.panel.show(near: nil) }),
            ("popup-closed", { self.panel.close() }),
            ("settings-open", { self.openSettings() }),
            ("settings-closed", { self.closeSettings() }),
            ("idle-60s-start", {}),
        ]
        var delay = Self.settle
        for (name, action) in steps {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
                action()
                DispatchQueue.main.asyncAfter(deadline: .now() + Self.settle - 0.5) { Self.phase(name) }
            }
            delay += Self.settle + 2
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + delay + 1 + Self.idleSeconds) {
            Self.phase("idle-60s-end")
            DispatchQueue.main.asyncAfter(deadline: .now() + Self.settle) { NSApp.terminate(nil) }
        }
    }

    private func seed() {
        for n in 0..<Self.textItems {
            write(PasteboardType.plainText, Data("Demo item \(n): lorem ipsum dolor sit amet, 示例文本 \(n * 7919)".utf8))
        }
        for n in 0..<Self.imageItems {
            if let png = Self.demoPNG(seed: n) { write(PasteboardType.png, png) }
        }
    }

    private func write(_ type: String, _ data: Data) {
        pasteboard.write(items: [[PasteboardEntry(type: type, data: data)]])
        controller.pasteboardChanged()
    }

    /// A 640 × 400 image with a solid colour per seed.
    private static func demoPNG(seed: Int) -> Data? {
        guard let image = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 640, pixelsHigh: 400, bitsPerSample: 8,
                                           samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                           colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)
        else { return nil }
        let colour = NSColor(hue: CGFloat(seed % 20) / 20, saturation: 0.6, brightness: 0.9, alpha: 1)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: image)
        colour.setFill()
        NSRect(x: 0, y: 0, width: 640, height: 400).fill()
        NSGraphicsContext.restoreGraphicsState()
        return image.representation(using: .png, properties: [:])
    }

    private static func phase(_ name: String) {
        print("phase \(name)")
        fflush(stdout)
    }
}
