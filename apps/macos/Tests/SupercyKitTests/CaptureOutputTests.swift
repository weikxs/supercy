import CoreGraphics
import ImageIO
import XCTest

/// A synthetic sRGB image whose pixel (x, y), counted from the top-left, has the colour
/// `color(x, y)`. Never a real screenshot.
func makeTestImage(width: Int, height: Int, color: (Int, Int) -> (UInt8, UInt8, UInt8)) -> CGImage? {
    var bytes: [UInt8] = []
    for y in 0..<height {
        for x in 0..<width {
            let (red, green, blue) = color(x, y)
            bytes += [red, green, blue, 255]
        }
    }
    guard let provider = CGDataProvider(data: Data(bytes) as CFData),
          let space = CGColorSpace(name: CGColorSpace.sRGB) else { return nil }
    return CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4,
                   space: space, bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
                   provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
}

final class MagnifierTests: XCTestCase {
    func testThePixelUnderThePointerFollowsTheScaleAndStaysInTheImage() {
        XCTAssertTrue(Magnifier.pixel(at: CGPoint(x: 10.6, y: 20.2), scale: 2, width: 2880, height: 1800) == (21, 40))
        XCTAssertTrue(Magnifier.pixel(at: CGPoint(x: -3, y: 5000), scale: 2, width: 2880, height: 1800) == (0, 1799))
        XCTAssertEqual(Magnifier.sampleRect(around: (0, 0)), CGRect(x: -7, y: -7, width: 15, height: 15))
    }

    func testTheLoupeMovesToTheOtherSideNearTheEdges() {
        let bounds = CGRect(x: 0, y: 0, width: 1440, height: 900)
        let size = CGSize(width: 150, height: 200)

        XCTAssertEqual(Magnifier.origin(near: CGPoint(x: 100, y: 100), size: size, bounds: bounds), CGPoint(x: 120, y: 120))
        XCTAssertEqual(Magnifier.origin(near: CGPoint(x: 1400, y: 850), size: size, bounds: bounds),
                       CGPoint(x: 1230, y: 630))
    }

    func testColoursAreWrittenAsHexAndRGB() {
        let color = PixelColor(red: 26, green: 43, blue: 255)

        XCTAssertEqual(color.hex, "#1A2BFF")
        XCTAssertEqual(PixelColor(red: 0, green: 5, blue: 10).hex, "#00050A")
        XCTAssertEqual(color.rgb, "rgb(26, 43, 255)")
    }
}

final class CaptureImageTests: XCTestCase {
    private func gradient() throws -> CGImage {
        try XCTUnwrap(makeTestImage(width: 8, height: 6) { x, y in (UInt8(x * 30), UInt8(y * 40), 200) })
    }

    func testColourPickingReadsThePixelFromTheTopLeft() throws {
        let image = try gradient()

        XCTAssertEqual(CaptureImage.color(of: image, x: 0, y: 0), PixelColor(red: 0, green: 0, blue: 200))
        XCTAssertEqual(CaptureImage.color(of: image, x: 7, y: 5), PixelColor(red: 210, green: 200, blue: 200))
        XCTAssertNil(CaptureImage.color(of: image, x: 8, y: 0))
    }

    func testACropIsItsOwnSmallBitmap() throws {
        let image = try gradient()

        let crop = try XCTUnwrap(CaptureImage.crop(image, to: CGRect(x: 2, y: 1, width: 3, height: 2)))

        XCTAssertEqual(crop.width, 3)
        XCTAssertEqual(crop.height, 2)
        XCTAssertLessThanOrEqual(crop.bytesPerRow * crop.height, 3 * 4 * 2 + 64 * 2, "no more memory than its own pixels")
        XCTAssertEqual(CaptureImage.color(of: crop, x: 0, y: 0), PixelColor(red: 60, green: 40, blue: 200))
        XCTAssertEqual(CaptureImage.color(of: crop, x: 2, y: 1), PixelColor(red: 120, green: 80, blue: 200))
    }

    func testFittingOnlyShrinksAndKeepsTheAspect() throws {
        let image = try XCTUnwrap(makeTestImage(width: 400, height: 200) { _, _ in (1, 2, 3) })

        XCTAssertTrue(CaptureImage.fitting(image, maxWidth: 500, maxHeight: 500) === image)
        let small = try XCTUnwrap(CaptureImage.fitting(image, maxWidth: 100, maxHeight: 100))
        XCTAssertEqual(small.width, 100)
        XCTAssertEqual(small.height, 50)
    }

    func testPNGEncodingRoundTrips() throws {
        let png = try XCTUnwrap(CaptureImage.png(try gradient()))
        let source = try XCTUnwrap(CGImageSourceCreateWithData(png as CFData, nil))
        let decoded = try XCTUnwrap(CGImageSourceCreateImageAtIndex(source, 0, nil))

        XCTAssertEqual(decoded.width, 8)
        XCTAssertEqual(CaptureImage.color(of: decoded, x: 7, y: 5), PixelColor(red: 210, green: 200, blue: 200))
    }
}

final class PinMetricsTests: XCTestCase {
    func testZoomAndOpacityStayInRange() {
        XCTAssertEqual(PinMetrics.clampedZoom(100), 8)
        XCTAssertEqual(PinMetrics.clampedZoom(0), 0.1)
        XCTAssertEqual(PinMetrics.clampedOpacity(2), 1)
        XCTAssertEqual(PinMetrics.clampedOpacity(0), 0.1)
        XCTAssertEqual(PinMetrics.zoom(7.9, scrolledBy: 1000), 8)
        XCTAssertEqual(PinMetrics.opacity(0.15, scrolledBy: -1000), 0.1)
    }

    func testScrollingAndPinchingChangeTheZoomEvenly() {
        let bigger = PinMetrics.zoom(1, scrolledBy: 10)
        let back = PinMetrics.zoom(bigger, scrolledBy: -10)

        XCTAssertGreaterThan(bigger, 1)
        XCTAssertEqual(back, 1, accuracy: 0.0001)
        XCTAssertEqual(PinMetrics.zoom(1, scrolledBy: 10_000), PinMetrics.zoom(1, scrolledBy: 25), "one flick is capped")
        XCTAssertEqual(PinMetrics.zoom(2, pinchedBy: 0.5), 3)
        XCTAssertEqual(PinMetrics.opacity(0.5, scrolledBy: 10), 0.6, accuracy: 0.0001)
    }

    func testSizesFollowPixelsScaleAndZoom() {
        XCTAssertEqual(PinMetrics.size(pixels: CGSize(width: 800, height: 600), scale: 2, zoom: 1),
                       CGSize(width: 400, height: 300))
        XCTAssertEqual(PinMetrics.size(pixels: CGSize(width: 800, height: 600), scale: 2, zoom: 0.5),
                       CGSize(width: 200, height: 150))
        XCTAssertEqual(PinMetrics.percent(0.8), "80 %")
    }

    func testZoomingKeepsTheAnchorOverTheSameSpot() {
        let frame = CGRect(x: 100, y: 100, width: 400, height: 300)
        let anchor = CGPoint(x: 200, y: 400)

        let zoomed = PinMetrics.zoomedFrame(frame, anchor: anchor, from: 1, to: 2)

        XCTAssertEqual(zoomed, CGRect(x: 0, y: -200, width: 800, height: 600))
        XCTAssertEqual((anchor.x - zoomed.minX) / zoomed.width, (anchor.x - frame.minX) / frame.width)
    }

    func testAPinKeepsNoMorePixelsThanItsScreenShows() {
        let pixels = CGSize(width: 2000, height: 1000)

        XCTAssertEqual(PinMetrics.keptPixels(pixels: pixels, imageScale: 2, screenScale: 2), pixels)
        XCTAssertEqual(PinMetrics.keptPixels(pixels: pixels, imageScale: 2, screenScale: 1), CGSize(width: 1000, height: 500))
        XCTAssertEqual(PinMetrics.keptPixels(pixels: pixels, imageScale: 1, screenScale: 2), pixels)
    }
}

final class CaptureFileNameTests: XCTestCase {
    func testNamesFollowTheTemplateWithoutColons() throws {
        let utc = try XCTUnwrap(TimeZone(identifier: "UTC"))
        let date = Date(timeIntervalSince1970: 1_769_868_309) // 2026-01-31 14:05:09 UTC

        XCTAssertEqual(CaptureFileName.name(for: date, timeZone: utc), "Supercy 2026-01-31 14.05.09.png")
    }

    func testTakenNamesGetANumber() {
        let taken: Set<String> = ["Shot.png", "Shot (2).png"]

        XCTAssertEqual(CaptureFileName.unique("Free.png") { taken.contains($0) }, "Free.png")
        XCTAssertEqual(CaptureFileName.unique("Shot.png") { taken.contains($0) }, "Shot (3).png")
        XCTAssertEqual(CaptureDelay.seconds, [3, 5, 10])
    }
}

final class CaptureRecordingTests: XCTestCase {
    private let source = SourceApp(bundleId: "com.example.capture", name: "Demo Capture")

    private func demoPNG() throws -> Data {
        let image = try XCTUnwrap(makeTestImage(width: 32, height: 16) { x, _ in (UInt8(x * 8), 90, 160) })
        return try XCTUnwrap(CaptureImage.png(image))
    }

    func testACopiedScreenshotIsRecordedOnceAndPutOnThePasteboard() throws {
        let rig = try Rig(self)
        rig.controller.start()
        let png = try demoPNG()

        rig.controller.record(items: [[PasteboardEntry(type: PasteboardType.png, data: png)]], source: source, copy: true)
        rig.scheduler.fireRepeating()

        let items = try rig.engine.list(kind: nil, offset: 0, limit: 10)
        XCTAssertEqual(items.count, 1, "the watcher does not record it a second time")
        XCTAssertEqual(items.first?.kind, .image)
        XCTAssertEqual(rig.pasteboard.readItems().first?.first?.data, png)
    }

    func testASavedOrPinnedScreenshotIsRecordedWithoutTouchingThePasteboard() throws {
        let rig = try Rig(self)
        rig.controller.start()
        rig.copy("already there")
        rig.scheduler.fireRepeating()
        let before = rig.pasteboard.changeCount

        rig.controller.record(items: [[PasteboardEntry(type: PasteboardType.png, data: try demoPNG())]], source: source,
                              copy: false)

        XCTAssertEqual(rig.pasteboard.changeCount, before)
        XCTAssertEqual(try rig.engine.list(kind: nil, offset: 0, limit: 10).count, 2)
    }

    func testPinningFromTheHistoryOnlyWorksForImages() throws {
        let rig = try Rig(self)
        rig.controller.start()
        let png = try demoPNG()
        rig.copy("some text")
        rig.scheduler.fireRepeating()
        rig.clock.now += 1
        rig.controller.record(items: [[PasteboardEntry(type: PasteboardType.png, data: png)]], source: source, copy: false)
        let model = HistoryViewModel(controller: rig.controller)
        var pinned: [Data] = []
        var closed = 0
        model.onClose = { closed += 1 }

        XCTAssertFalse(model.perform(.pinToScreen, nowMs: demoNow), "not available without a pin handler")
        model.onPinImage = { pinned.append($0) }
        XCTAssertEqual(model.items.first?.kind, .image)
        XCTAssertTrue(model.perform(.pinToScreen, nowMs: demoNow))
        model.moveSelection(by: 1)
        XCTAssertFalse(model.perform(.pinToScreen, nowMs: demoNow))

        XCTAssertEqual(pinned.count, 1)
        XCTAssertEqual(pinned.first.flatMap { ThumbnailCache.decode($0, maxPixels: 64)?.width }, 32)
        XCTAssertEqual(closed, 1)
    }
}
