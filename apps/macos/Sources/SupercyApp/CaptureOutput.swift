import AppKit
import UniformTypeIdentifiers

/// Where a finished screenshot goes: the pasteboard, a file, a pin — and, once, the history.
final class CaptureOutput {
    private let controller: ClipboardController
    private let pins: PinsController
    /// The folder of the last save, this session only (no core setting yet).
    private var saveDirectory: URL?
    private var savePanel: NSSavePanel?
    /// Screenshots are recorded as copied from Supercy.
    private static let source = SourceApp(bundleId: Bundle.main.bundleIdentifier ?? "io.github.weikxs.Supercy",
                                          name: "Supercy")

    init(controller: ClipboardController, pins: PinsController) {
        self.controller = controller
        self.pins = pins
    }

    /// Delivers a screenshot: `frame` is where it was on screen (AppKit coordinates), `scale`
    /// its pixels per point. Every action also records it in the history once.
    func deliver(_ image: CGImage, action: CaptureAction, frame: CGRect, scale: CGFloat) {
        switch action {
        case .copy: copy(image)
        case .save: save(image, record: true)
        case .pin:
            pins.pin(image, scale: scale, frame: frame)
            encode(image) { [weak self] png in self?.record(png, copy: false) }
        }
    }

    /// Copies `image` to the pasteboard (and the history).
    func copy(_ image: CGImage) {
        encode(image) { [weak self] png in self?.record(png, copy: true) }
    }

    /// Copies a picked colour as text (and records it like any copy).
    func copy(text: String) {
        controller.record(items: [[PasteboardEntry(type: PasteboardType.plainText, data: Data(text.utf8))]],
                          source: Self.source, copy: true)
    }

    /// Asks where to save `image` as PNG, starting in the last folder or the Desktop.
    func save(_ image: CGImage, record: Bool = false) {
        savePanel?.cancel(nil)
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.png]
        panel.canCreateDirectories = true
        panel.isExtensionHidden = false
        let directory = saveDirectory ?? FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask).first
        panel.directoryURL = directory
        let name = CaptureFileName.name(for: Date())
        panel.nameFieldStringValue = directory.map { folder in
            CaptureFileName.unique(name) { FileManager.default.fileExists(atPath: folder.appendingPathComponent($0).path) }
        } ?? name
        panel.level = .modalPanel
        savePanel = panel
        NSApp.activate(ignoringOtherApps: true)
        panel.begin { [weak self] response in
            self?.savePanel = nil
            guard response == .OK, let url = panel.url else { return }
            self?.saveDirectory = url.deletingLastPathComponent()
            self?.encode(image) { png in self?.write(png, to: url, record: record) }
        }
    }

    private func write(_ png: Data?, to url: URL, record: Bool) {
        do {
            guard let png else { throw CocoaError(.fileWriteUnknown) }
            try png.write(to: url, options: .atomic)
            if record { self.record(png, copy: false) }
        } catch {
            NSLog("Supercy: saving a screenshot failed: %@", error.localizedDescription)
            let alert = NSAlert()
            alert.messageText = L10n.text(.saveFailed)
            alert.informativeText = error.localizedDescription
            alert.runModal()
        }
    }

    private func record(_ png: Data?, copy: Bool) {
        guard let png else { return }
        controller.record(items: [[PasteboardEntry(type: PasteboardType.png, data: png)]], source: Self.source, copy: copy)
    }

    /// Encodes PNG off the main thread (a 5K screenshot takes a moment), then calls back on it.
    private func encode(_ image: CGImage, completion: @escaping (Data?) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async {
            let png = CaptureImage.png(image)
            DispatchQueue.main.async { completion(png) }
        }
    }
}
