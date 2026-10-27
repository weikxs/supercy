import AppKit
import XCTest

final class ThumbnailCacheTests: XCTestCase {
    private func png(width: Int, height: Int) throws -> Data {
        let image = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height,
                                                   bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                                   colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        return try XCTUnwrap(image.representation(using: .png, properties: [:]))
    }

    func testDecodesAtDisplaySizeNotFullSize() throws {
        let data = try png(width: 2_000, height: 1_000)

        let thumbnail = try XCTUnwrap(ThumbnailCache.decode(data, maxPixels: 80))

        XCTAssertEqual(max(thumbnail.width, thumbnail.height), 80)
        XCTAssertNil(ThumbnailCache.decode(Data("not an image".utf8), maxPixels: 80))
    }

    func testLoadsOnceAndEvictsTheLeastRecentlyUsed() throws {
        let cache = ThumbnailCache(maxEntries: 2, maxBytes: 1 << 20)
        let data = try png(width: 64, height: 64)
        var loads = 0
        let load = { () -> Data? in loads += 1; return data }

        _ = cache.thumbnail(for: "a", maxPixels: 32, load: load)
        _ = cache.thumbnail(for: "b", maxPixels: 32, load: load)
        _ = cache.thumbnail(for: "a", maxPixels: 32, load: load)
        _ = cache.thumbnail(for: "c", maxPixels: 32, load: load)
        _ = cache.thumbnail(for: "a", maxPixels: 32, load: load)
        _ = cache.thumbnail(for: "b", maxPixels: 32, load: load)

        XCTAssertEqual(loads, 4, "a stays cached; b was evicted by c and loaded again")
        XCTAssertEqual(cache.count, 2)
    }

    func testStaysWithinTheByteBudget() throws {
        let cache = ThumbnailCache(maxEntries: 100, maxBytes: 64 * 64 * 4 * 3)
        let data = try png(width: 64, height: 64)

        for n in 0..<10 { _ = cache.thumbnail(for: "\(n)", maxPixels: 64) { data } }

        XCTAssertLessThanOrEqual(cache.bytes, cache.maxBytes)
        XCTAssertLessThanOrEqual(cache.count, 3)
    }

    func testImageItemsGetThumbnailsThroughTheViewModel() throws {
        let rig = try Rig(self)
        rig.controller.start()
        rig.pasteboard.write(items: [[PasteboardEntry(type: PasteboardType.png, data: try png(width: 300, height: 200))]])
        rig.scheduler.fireRepeating()
        let model = HistoryViewModel(controller: rig.controller)

        let thumbnail = try XCTUnwrap(model.thumbnail(of: try XCTUnwrap(model.items.first), maxPixels: 60))

        XCTAssertEqual(thumbnail.width, 60)
        XCTAssertEqual(model.thumbnails.count, 1)
    }
}
