import CoreGraphics
import Foundation
import ImageIO

/// Image thumbnails for the history list, decoded at display size (never the full image) and
/// kept in a small least-recently-used cache. The cache belongs to the open panel and goes
/// away with it, so nothing stays in memory while the panel is closed.
public final class ThumbnailCache {
    public let maxEntries: Int
    public let maxBytes: Int
    private var entries: [String: CGImage] = [:]
    private var order: [String] = []
    public private(set) var bytes = 0

    public init(maxEntries: Int = 48, maxBytes: Int = 4 * 1024 * 1024) {
        self.maxEntries = maxEntries
        self.maxBytes = maxBytes
    }

    public var count: Int { entries.count }

    /// The thumbnail for `key` at most `maxPixels` high or wide; `load` supplies the encoded
    /// image only when it is not cached.
    public func thumbnail(for key: String, maxPixels: Int, load: () -> Data?) -> CGImage? {
        let cacheKey = "\(key)@\(maxPixels)"
        if let image = entries[cacheKey] {
            touch(cacheKey)
            return image
        }
        guard let data = load(), let image = Self.decode(data, maxPixels: maxPixels) else { return nil }
        insert(image, for: cacheKey)
        return image
    }

    /// Decodes `data` straight to a thumbnail no larger than `maxPixels` on its longer side.
    public static func decode(_ data: Data, maxPixels: Int) -> CGImage? {
        guard maxPixels > 0, let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixels,
        ]
        return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    }

    private static func cost(_ image: CGImage) -> Int { image.bytesPerRow * image.height }

    private func insert(_ image: CGImage, for key: String) {
        entries[key] = image
        order.append(key)
        bytes += Self.cost(image)
        while (entries.count > maxEntries || bytes > maxBytes), let oldest = order.first {
            order.removeFirst()
            if let removed = entries.removeValue(forKey: oldest) { bytes -= Self.cost(removed) }
        }
    }

    private func touch(_ key: String) {
        order.removeAll { $0 == key }
        order.append(key)
    }
}
