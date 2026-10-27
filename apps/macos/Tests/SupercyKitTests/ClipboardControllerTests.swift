import XCTest

final class ClipboardControllerTests: XCTestCase {
    func testACopyIsRecordedWhenTheTimerFires() throws {
        let rig = try Rig(self)
        rig.controller.start()

        rig.copy("Hello from Supercy")
        rig.scheduler.fireRepeating()

        XCTAssertEqual(rig.previews, ["Hello from Supercy"])
        XCTAssertEqual(rig.scheduler.activeRepeating.first?.interval, 0.5)
    }

    func testConcealedAndTransientCopiesAreNeverStored() throws {
        let rig = try Rig(self)
        rig.controller.start()

        rig.copy("demo-password-123", extraTypes: [PasteboardType.concealed])
        rig.scheduler.fireRepeating()
        rig.copy("one-time code", extraTypes: [PasteboardType.transient])
        rig.scheduler.fireRepeating()

        XCTAssertEqual(rig.previews, [])
    }

    func testPausingStopsTheTimerAndCopiesDuringThePauseAreNotRecordedAfterResuming() throws {
        let rig = try Rig(self)
        rig.controller.start()

        rig.controller.pause(minutes: nil)
        XCTAssertTrue(rig.scheduler.activeRepeating.isEmpty)
        rig.copy("copied while paused")
        rig.controller.resume()
        rig.scheduler.fireRepeating()

        XCTAssertEqual(rig.previews, [])
        XCTAssertFalse(rig.controller.isPaused)
        XCTAssertEqual(rig.scheduler.activeRepeating.count, 1)
    }

    func testATimedPauseResumesOnTheScheduledTick() throws {
        let rig = try Rig(self)
        rig.controller.start()

        rig.controller.pause(minutes: 15)
        let tick = try XCTUnwrap(rig.scheduler.pendingOnce)
        rig.clock.now += 15 * 60_000
        tick.block()

        XCTAssertEqual(tick.delay, 15 * 60, accuracy: 0.001)
        XCTAssertFalse(rig.controller.isPaused)
        XCTAssertEqual(rig.scheduler.activeRepeating.count, 1)
    }

    func testIgnoreNextCopySkipsExactlyOne() throws {
        let rig = try Rig(self)
        rig.controller.start()

        rig.controller.skipNextCopy()
        rig.copy("skipped")
        rig.scheduler.fireRepeating()
        rig.copy("kept")
        rig.scheduler.fireRepeating()

        XCTAssertEqual(rig.previews, ["kept"])
    }

    func testChoosingWritesThePasteboardWithoutRecordingItAgain() throws {
        let rig = try Rig(self)
        rig.controller.start()
        rig.copy("first")
        rig.scheduler.fireRepeating()
        rig.copy("second")
        rig.scheduler.fireRepeating()
        let first = try XCTUnwrap(rig.engine.list(kind: nil, offset: 0, limit: 10).last)

        let result = rig.controller.choose(id: first.id, plainText: false, paste: false)
        rig.scheduler.fireRepeating()

        XCTAssertEqual(result, .copied)
        XCTAssertEqual(rig.pasteboard.readItems().first?.first?.data, Data("first".utf8))
        XCTAssertEqual(rig.previews, ["first", "second"])
        XCTAssertEqual(try rig.engine.list(kind: nil, offset: 0, limit: 1).first?.copyCount, 1)
    }

    func testPastingNeedsAccessibilityAndPastesOnceAllowed() throws {
        let rig = try Rig(self)
        rig.controller.start()
        rig.copy("demo")
        rig.scheduler.fireRepeating()
        let id = try XCTUnwrap(rig.engine.list(kind: nil, offset: 0, limit: 1).first?.id)

        let refused = rig.controller.choose(id: id, plainText: false, paste: true)
        rig.paster.isTrusted = true
        let pasted = rig.controller.choose(id: id, plainText: true, paste: true)

        XCTAssertEqual(refused, .needsAccessibility)
        XCTAssertEqual(pasted, .pasted)
        XCTAssertEqual(rig.paster.pasteCount, 1)
    }

    func testSensitiveItemsExpireOnTheScheduledTick() throws {
        let rig = try Rig(self)
        rig.controller.start()

        rig.copy("-----BEGIN EC " + "PRIVATE KEY-----")
        rig.scheduler.fireRepeating()
        let tick = try XCTUnwrap(rig.scheduler.pendingOnce)
        rig.clock.now += 120_000
        tick.block()

        XCTAssertEqual(tick.delay, 120, accuracy: 0.001)
        XCTAssertEqual(rig.previews, [])
    }

    func testTheIntervalSettingChangesTheTimer() throws {
        let rig = try Rig(self)
        rig.controller.start()

        rig.controller.applyPollInterval(1_000)

        XCTAssertEqual(rig.scheduler.activeRepeating.map(\.interval), [1.0])
    }
}
