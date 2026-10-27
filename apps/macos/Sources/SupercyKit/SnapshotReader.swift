import AppKit
import Foundation

/// The application in front, for "copied from" and the ignore lists.
public protocol FrontAppProviding {
    func frontApp() -> SourceApp?
}

/// Turns the pasteboard into a `ClipboardSnapshot` for the core. All decisions (markers,
/// ignore lists, limits) are the core's; the reader only reads, caps sizes and converts TIFF to
/// PNG (macOS images are often uncompressed TIFF, many times larger).
public struct SnapshotReader {
    /// Data larger than this is not read into the snapshot; only its size is passed on.
    public static let maxEntryBytes = 16 * 1024 * 1024

    private let pasteboard: PasteboardReading
    private let frontApp: FrontAppProviding

    public init(pasteboard: PasteboardReading, frontApp: FrontAppProviding) {
        self.pasteboard = pasteboard
        self.frontApp = frontApp
    }

    public func snapshot(at nowMs: UInt64) -> ClipboardSnapshot {
        let items = pasteboard.readItems()
        let all = items.flatMap { item in clipEntries(of: item) }
        return ClipboardSnapshot(entries: all, frontApp: frontApp.frontApp(), capturedAtMs: nowMs)
    }

    private func clipEntries(of item: [PasteboardEntry]) -> [ClipEntry] {
        var entries = item.map(clipEntry)
        let types = Set(item.map(\.type))
        if types.contains(PasteboardType.tiff), !types.contains(PasteboardType.png),
           let tiff = item.first(where: { $0.type == PasteboardType.tiff })?.data,
           let png = Self.pngData(fromTIFF: tiff) {
            entries.removeAll { $0.typeId == PasteboardType.tiff }
            entries.append(ClipEntry(typeId: PasteboardType.png, data: png, size: UInt64(png.count)))
        }
        return entries
    }

    private func clipEntry(_ entry: PasteboardEntry) -> ClipEntry {
        guard let data = entry.data else {
            return ClipEntry(typeId: entry.type, data: nil, size: 0)
        }
        if data.count > Self.maxEntryBytes {
            return ClipEntry(typeId: entry.type, data: nil, size: UInt64(data.count))
        }
        return ClipEntry(typeId: entry.type, data: data, size: UInt64(data.count))
    }

    static func pngData(fromTIFF tiff: Data) -> Data? {
        guard let image = NSBitmapImageRep(data: tiff) else { return nil }
        return image.representation(using: .png, properties: [:])
    }
}
