import AppKit

/// Fixed sequences for `make macos-measure`, run only in the isolated mode (in-memory
/// pasteboard, data and key under /tmp):
///
/// * `seed`: copies 500 demo items (480 texts, 20 images) a few at a time, then quits.
/// * `run`: after a relaunch with those items — idle 60 s; open the history, search, paste the
///   first result (into the in-memory pasteboard), close; open and close the settings (off
///   screen); idle 10 min. Each phase is printed so the script outside samples memory and CPU.
final class MeasurementScript {
    enum Scenario: String {
        case seed, run
    }

    private let controller: ClipboardController
    private let pasteboard: InMemoryPasteboard
    private let panel: HistoryPanelController
    private let openSettings: () -> Void
    private let closeSettings: () -> Void
    private static let step: TimeInterval = 3
    private static let textItems = 480
    private static let imageItems = 20
    private static let seedBatch = 20
    /// The long idle at the end: 10 minutes, shorter with SUPERCY_MEASURE_IDLE for diagnosis.
    private static let longIdle: TimeInterval =
        ProcessInfo.processInfo.environment["SUPERCY_MEASURE_IDLE"].flatMap(TimeInterval.init) ?? 600

    init(controller: ClipboardController, pasteboard: InMemoryPasteboard, panel: HistoryPanelController,
         openSettings: @escaping () -> Void, closeSettings: @escaping () -> Void) {
        self.controller = controller
        self.pasteboard = pasteboard
        self.panel = panel
        self.openSettings = openSettings
        self.closeSettings = closeSettings
    }

    func run(_ scenario: Scenario) {
        switch scenario {
        case .seed: seed(from: 0)
        case .run: runSteps()
        }
    }

    /// Seconds the outside script gets to take its sample after each phase is printed (vmmap is
    /// slow); nothing happens in the app meanwhile.
    private static let sampleGap: TimeInterval = 5

    private lazy var steps: [(phase: String, wait: TimeInterval, action: () -> Void)] = [
        ("relaunched", Self.step, {}),
        ("idle-60s-start", 0, {}),
        ("idle-60s-end", 60, {}),
        ("history-open", Self.step, { self.panel.show(near: nil) }),
        ("searched", Self.step, { self.panel.scriptSearch("demo 42") }),
        ("pasted", Self.step, { self.panel.scriptChooseFirst() }),
        ("history-closed", Self.step, { self.panel.close() }),
        ("settings-open", Self.step, { self.openSettings() }),
        ("settings-closed", Self.step, { self.closeSettings() }),
        ("idle-10min-start", Self.step, {}),
        ("idle-10min-end", Self.longIdle, {}),
    ]

    /// Runs step `index`: its action, its wait, its phase line, then a quiet gap for sampling.
    private func runSteps(from index: Int = 0) {
        guard index < steps.count else {
            NSApp.terminate(nil)
            return
        }
        let step = steps[index]
        step.action()
        DispatchQueue.main.asyncAfter(deadline: .now() + step.wait) {
            Self.phase(step.phase)
            DispatchQueue.main.asyncAfter(deadline: .now() + Self.sampleGap) { self.runSteps(from: index + 1) }
        }
    }

    /// Copies a batch, lets the run loop breathe, and continues, like copies spread over time.
    private func seed(from start: Int) {
        let total = Self.textItems + Self.imageItems
        guard start < total else {
            Self.phase("seeded")
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { NSApp.terminate(nil) }
            return
        }
        for n in start..<min(start + Self.seedBatch, total) {
            if n < Self.textItems {
                write(PasteboardType.plainText, Data("Demo item \(n): lorem ipsum dolor sit amet, 示例文本 \(n * 7919)".utf8))
            } else if let png = Self.demoPNG(seed: n) {
                write(PasteboardType.png, png)
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { self.seed(from: start + Self.seedBatch) }
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
