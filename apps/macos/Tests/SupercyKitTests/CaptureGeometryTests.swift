import CoreGraphics
import XCTest

final class CaptureGeometryTests: XCTestCase {
    private let display = CGRect(x: 0, y: 0, width: 1440, height: 900)

    func testARectFromTwoCornersInAnyOrder() {
        XCTAssertEqual(CaptureGeometry.rect(from: CGPoint(x: 50, y: 80), to: CGPoint(x: 10, y: 20)),
                       CGRect(x: 10, y: 20, width: 40, height: 60))
    }

    func testSnappingRoundsToWholePointsAndCutsToTheDisplay() {
        XCTAssertEqual(CaptureGeometry.snapped(CGRect(x: 10.4, y: 20.6, width: 30.2, height: 9.8), in: display),
                       CGRect(x: 10, y: 21, width: 31, height: 9))
        XCTAssertEqual(CaptureGeometry.snapped(CGRect(x: 1400, y: -20, width: 100, height: 50), in: display),
                       CGRect(x: 1400, y: 0, width: 40, height: 30))
        XCTAssertNil(CaptureGeometry.snapped(CGRect(x: 2000, y: 0, width: 10, height: 10), in: display))
    }

    func testMovingKeepsTheSizeAndStaysOnTheDisplay() {
        let rect = CGRect(x: 100, y: 100, width: 200, height: 100)

        XCTAssertEqual(CaptureGeometry.moved(rect, dx: 20, dy: -30, in: display), CGRect(x: 120, y: 70, width: 200, height: 100))
        XCTAssertEqual(CaptureGeometry.moved(rect, dx: -500, dy: 5000, in: display), CGRect(x: 0, y: 800, width: 200, height: 100))
    }

    func testResizingByEachHandleAndFlippingPastTheOppositeEdge() {
        let rect = CGRect(x: 100, y: 100, width: 200, height: 100)

        XCTAssertEqual(CaptureGeometry.resized(rect, handle: .bottomRight, dx: 50, dy: 20, in: display),
                       CGRect(x: 100, y: 100, width: 250, height: 120))
        XCTAssertEqual(CaptureGeometry.resized(rect, handle: .topLeft, dx: -10, dy: -10, in: display),
                       CGRect(x: 90, y: 90, width: 210, height: 110))
        XCTAssertEqual(CaptureGeometry.resized(rect, handle: .left, dx: 0, dy: 99, in: display), rect)
        XCTAssertEqual(CaptureGeometry.resized(rect, handle: .right, dx: -300, dy: 0, in: display),
                       CGRect(x: 0, y: 100, width: 100, height: 100))
        XCTAssertEqual(CaptureGeometry.resized(rect, handle: .bottom, dx: 0, dy: 5000, in: display),
                       CGRect(x: 100, y: 100, width: 200, height: 800))
        XCTAssertEqual(CaptureGeometry.resized(rect, handle: .inside, dx: 5, dy: 5, in: display),
                       CGRect(x: 105, y: 105, width: 200, height: 100))
        XCTAssertEqual(CaptureGeometry.resized(rect, handle: .top, dx: 0, dy: 100, in: display).height, 1,
                       "never thinner than one point")
    }

    func testHandlesAreFoundAtCornersEdgesAndInside() {
        let rect = CGRect(x: 100, y: 100, width: 200, height: 100)
        let handle = { (x: CGFloat, y: CGFloat) in CaptureGeometry.handle(at: CGPoint(x: x, y: y), of: rect, tolerance: 6) }

        XCTAssertEqual(handle(102, 98), .topLeft)
        XCTAssertEqual(handle(300, 100), .topRight)
        XCTAssertEqual(handle(100, 200), .bottomLeft)
        XCTAssertEqual(handle(305, 204), .bottomRight)
        XCTAssertEqual(handle(200, 100), .top)
        XCTAssertEqual(handle(200, 203), .bottom)
        XCTAssertEqual(handle(97, 150), .left)
        XCTAssertEqual(handle(300, 150), .right)
        XCTAssertEqual(handle(200, 150), .inside)
        XCTAssertNil(handle(50, 150))
        XCTAssertNil(handle(200, 220))
    }

    func testArrowKeysMoveOrResizeWithinTheDisplay() {
        let rect = CGRect(x: 0, y: 10, width: 20, height: 20)

        XCTAssertEqual(CaptureGeometry.nudged(rect, .right, resize: false, step: 1, in: display),
                       CGRect(x: 1, y: 10, width: 20, height: 20))
        XCTAssertEqual(CaptureGeometry.nudged(rect, .left, resize: false, step: 10, in: display), rect)
        XCTAssertEqual(CaptureGeometry.nudged(rect, .up, resize: false, step: 10, in: display),
                       CGRect(x: 0, y: 0, width: 20, height: 20))
        XCTAssertEqual(CaptureGeometry.nudged(rect, .down, resize: true, step: 10, in: display),
                       CGRect(x: 0, y: 10, width: 20, height: 30))
        XCTAssertEqual(CaptureGeometry.nudged(rect, .left, resize: true, step: 50, in: display).width, 1)
        XCTAssertEqual(CaptureGeometry.nudged(CGRect(x: 1430, y: 0, width: 10, height: 10), .right, resize: true, step: 10,
                                              in: display).width, 10)
    }

    func testPixelRectsFollowTheScaleAndStayInTheImage() {
        XCTAssertEqual(CaptureGeometry.pixelRect(CGRect(x: 10, y: 20, width: 30, height: 40), scale: 2,
                                                 imageWidth: 2880, imageHeight: 1800),
                       CGRect(x: 20, y: 40, width: 60, height: 80))
        XCTAssertEqual(CaptureGeometry.pixelRect(CGRect(x: 1430, y: 890, width: 20, height: 20), scale: 2,
                                                 imageWidth: 2880, imageHeight: 1800),
                       CGRect(x: 2860, y: 1780, width: 20, height: 20))
        XCTAssertEqual(CaptureGeometry.pixelRect(CGRect(x: 0.5, y: 0.5, width: 1, height: 1), scale: 1.5,
                                                 imageWidth: 100, imageHeight: 100),
                       CGRect(x: 0, y: 0, width: 3, height: 3))
        XCTAssertNil(CaptureGeometry.pixelRect(CGRect(x: 2000, y: 0, width: 5, height: 5), scale: 1,
                                               imageWidth: 100, imageHeight: 100))
    }

    func testDisplaySpaceAndAppKitCoordinatesConvertBothWays() {
        // A second screen to the right of a 1440 × 900 main screen, its bottom 100 points higher.
        let screen = CGRect(x: 1440, y: 100, width: 1920, height: 1080)
        let rect = CGRect(x: 10, y: 20, width: 300, height: 200)

        let appKit = CaptureGeometry.appKitRect(rect, screenFrame: screen)

        XCTAssertEqual(appKit, CGRect(x: 1450, y: 960, width: 300, height: 200))
        XCTAssertEqual(CaptureGeometry.displayPoint(CGPoint(x: appKit.minX, y: appKit.maxY), screenFrame: screen),
                       CGPoint(x: 10, y: 20))
        XCTAssertEqual(CaptureGeometry.flipped(CaptureGeometry.flipped(rect, height: 900), height: 900), rect)
        XCTAssertEqual(CaptureGeometry.flipped(rect, height: 900), CGRect(x: 10, y: 680, width: 300, height: 200))
        XCTAssertEqual(CaptureGeometry.flipped(CGPoint(x: 5, y: 0), height: 900), CGPoint(x: 5, y: 900))
    }
}

final class CaptureWindowTests: XCTestCase {
    private func info(_ number: Int, pid: Int32 = 42, layer: Int = 0, alpha: Double = 1,
                      _ x: Double, _ y: Double, _ width: Double, _ height: Double) -> [String: Any] {
        ["kCGWindowNumber": NSNumber(value: number), "kCGWindowOwnerPID": NSNumber(value: pid),
         "kCGWindowLayer": NSNumber(value: layer), "kCGWindowAlpha": NSNumber(value: alpha),
         "kCGWindowBounds": ["X": NSNumber(value: x), "Y": NSNumber(value: y),
                             "Width": NSNumber(value: width), "Height": NSNumber(value: height)]]
    }

    func testOnlyNormalVisibleWindowsOfOtherAppsOnTheDisplayInDisplaySpace() {
        let list = [
            info(1, layer: 25, 0, 0, 1440, 24), // menu bar
            info(2, 100, 100, 600, 400),
            info(3, pid: 7, 120, 120, 300, 300), // Supercy's own window
            info(4, alpha: 0, 0, 0, 500, 500),
            info(5, 10, 10, 20, 20), // too small
            info(6, 1400, 200, 800, 600), // across both displays
            info(7, 50, 50, 1000, 700),
        ]

        let main = CaptureWindow.selectable(from: list, ownProcess: 7, displayBounds: CGRect(x: 0, y: 0, width: 1440, height: 900))
        let second = CaptureWindow.selectable(from: list, ownProcess: 7,
                                              displayBounds: CGRect(x: 1440, y: 0, width: 1920, height: 1080))

        XCTAssertEqual(main.map(\.id), [2, 6, 7], "front to back, the one spanning both displays too")
        XCTAssertEqual(second.map(\.id), [6])
        XCTAssertEqual(second.first?.frame, CGRect(x: -40, y: 200, width: 800, height: 600))
    }

    func testTheFrontMostWindowUnderThePointerWins() {
        let windows = [CaptureWindow(id: 2, frame: CGRect(x: 100, y: 100, width: 100, height: 100)),
                       CaptureWindow(id: 7, frame: CGRect(x: 0, y: 0, width: 800, height: 600))]

        XCTAssertEqual(CaptureWindow.window(at: CGPoint(x: 150, y: 150), in: windows)?.id, 2)
        XCTAssertEqual(CaptureWindow.window(at: CGPoint(x: 50, y: 50), in: windows)?.id, 7)
        XCTAssertNil(CaptureWindow.window(at: CGPoint(x: 900, y: 50), in: windows))
    }
}
