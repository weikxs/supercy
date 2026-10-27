import AppKit
import XCTest

final class HistoryViewModelTests: XCTestCase {
    private func filled(_ texts: [String]) throws -> (Rig, HistoryViewModel) {
        let rig = try Rig(self)
        rig.controller.start()
        for (offset, text) in texts.enumerated() {
            rig.clock.now = demoNow + UInt64(offset)
            rig.copy(text)
            rig.scheduler.fireRepeating()
        }
        return (rig, HistoryViewModel(controller: rig.controller))
    }

    func testListsNewestFirstAndSearchesWithPinyin() throws {
        let (_, model) = try filled(["会议纪要", "lorem ipsum", "Hello from Supercy"])

        XCTAssertEqual(model.items.map(\.preview), ["Hello from Supercy", "lorem ipsum", "会议纪要"])
        model.query = "hyjy"
        XCTAssertEqual(model.items.map(\.preview), ["会议纪要"])
        model.query = "zzz"
        XCTAssertTrue(model.items.isEmpty)
    }

    func testSelectionMovesWithinTheListAndNumbersChoose() throws {
        let (rig, model) = try filled(["a", "b", "c"])
        var closed = 0
        model.onClose = { closed += 1 }

        model.perform(.moveDown, nowMs: demoNow)
        model.perform(.moveDown, nowMs: demoNow)
        model.perform(.moveDown, nowMs: demoNow)
        XCTAssertEqual(model.selectedIndex, 2)
        model.perform(.moveUp, nowMs: demoNow)
        XCTAssertEqual(model.selected?.preview, "b")

        XCTAssertTrue(model.perform(.selectNumber(3), nowMs: demoNow))
        XCTAssertFalse(model.perform(.selectNumber(9), nowMs: demoNow))
        XCTAssertEqual(closed, 1)
        XCTAssertEqual(rig.pasteboard.readItems().first?.first?.data, Data("a".utf8))
    }

    func testPinningGivesALetterThatChoosesTheItem() throws {
        let (rig, model) = try filled(["pin me", "other"])
        model.selectedIndex = 1

        model.perform(.togglePin, nowMs: demoNow + 10)
        let pinned = try XCTUnwrap(model.items.first)

        XCTAssertTrue(pinned.pinned)
        XCTAssertEqual(pinned.pinKey, "a")
        XCTAssertTrue(model.perform(.pinKey("a"), nowMs: demoNow))
        XCTAssertFalse(model.perform(.pinKey("b"), nowMs: demoNow))
        XCTAssertEqual(rig.pasteboard.readItems().first?.first?.data, Data("pin me".utf8))
    }

    func testDeleteAndClearKeepPins() throws {
        let (_, model) = try filled(["a", "b", "c"])
        model.perform(.togglePin, nowMs: demoNow + 10)

        model.selectedIndex = 1
        model.perform(.delete, nowMs: demoNow)
        XCTAssertEqual(model.items.count, 2)
        model.perform(.clearHistory, nowMs: demoNow)
        XCTAssertEqual(model.items.map(\.preview), ["c"])
        model.perform(.clearAll, nowMs: demoNow)
        XCTAssertTrue(model.items.isEmpty)
    }

    func testInvalidRegexIsReportedInRegexMode() throws {
        let (rig, model) = try filled(["order 123456"])
        var settings = try rig.engine.settings()
        settings.searchMode = .regex
        _ = try rig.engine.updateSettings(settings: settings)

        model.query = "("
        XCTAssertNotNil(model.queryProblem)
        model.query = "\\d{6}"
        XCTAssertNil(model.queryProblem)
        XCTAssertEqual(model.items.first?.matches, [TextRange(location: 6, length: 6)])
    }

    func testCopyOnlyAndPlainTextChoices() throws {
        let (rig, model) = try filled(["demo"])
        rig.paster.isTrusted = true

        model.perform(.choose(plainText: nil, copyOnly: true), nowMs: demoNow)
        XCTAssertEqual(rig.paster.pasteCount, 0)
        model.perform(.choose(plainText: true, copyOnly: false), nowMs: demoNow)
        XCTAssertEqual(rig.paster.pasteCount, 1)
    }

    func testPinnedItemsMoveByKeyAndByDropButNotInSearchResults() throws {
        let (_, model) = try filled(["plain", "c", "b", "a"])
        for (offset, text) in ["a", "b", "c"].enumerated() {
            model.selectedIndex = try XCTUnwrap(model.items.firstIndex { $0.preview == text })
            model.perform(.togglePin, nowMs: demoNow + 10 + UInt64(offset))
        }
        XCTAssertEqual(model.items.map(\.preview), ["a", "b", "c", "plain"])
        XCTAssertEqual(model.pinnedRows, 0...2)

        model.selectedIndex = 2
        model.perform(.movePinUp, nowMs: demoNow + 20)
        XCTAssertEqual(model.items.map(\.preview), ["a", "c", "b", "plain"])
        XCTAssertEqual(model.selected?.preview, "c")
        model.perform(.movePinUp, nowMs: demoNow + 21)
        model.perform(.movePinUp, nowMs: demoNow + 22) // already first
        XCTAssertEqual(model.items.map(\.preview), ["c", "a", "b", "plain"])
        model.perform(.movePinDown, nowMs: demoNow + 23)
        XCTAssertEqual(model.items.map(\.preview), ["a", "c", "b", "plain"])

        XCTAssertTrue(model.movePin(fromRow: 0, toRow: 3, nowMs: demoNow + 24)) // dropped after the last pin
        XCTAssertEqual(model.items.map(\.preview), ["c", "b", "a", "plain"])
        XCTAssertFalse(model.movePin(fromRow: 3, toRow: 0, nowMs: demoNow + 25)) // not pinned
        XCTAssertFalse(model.movePin(fromRow: 0, toRow: 4, nowMs: demoNow + 25)) // among the unpinned

        model.query = "a"
        XCTAssertNil(model.pinnedRows)
        XCTAssertEqual(KeyCommand.from(keyCode: 126, modifiers: [.option, .command]), .movePinUp)
        XCTAssertEqual(KeyCommand.from(keyCode: 125, modifiers: [.option, .command]), .movePinDown)
    }
}

final class KeyCommandTests: XCTestCase {
    func testMapsTheDocumentedKeys() {
        let cases: [(UInt16, NSEvent.ModifierFlags, KeyCommand?)] = [
            (126, [], .moveUp), (125, [], .moveDown), (53, [], .close),
            (36, [], .choose(plainText: nil, copyOnly: false)),
            (36, .option, .choose(plainText: nil, copyOnly: true)),
            (36, [.option, .shift], .choose(plainText: true, copyOnly: false)),
            (35, .option, .togglePin), (51, .option, .delete), (51, [.option, .command], .clearHistory),
            (51, [.option, .command, .shift], .clearAll), (43, .command, .openSettings),
            (18, .command, .selectNumber(1)), (25, .command, .selectNumber(9)),
            (0, .control, .pinKey("a")), (40, .control, .pinKey("k")),
            (0, [], nil), (51, [], nil), (18, [], nil), (36, .command, nil),
        ]
        for (code, flags, expected) in cases {
            XCTAssertEqual(KeyCommand.from(keyCode: code, modifiers: flags), expected, "key \(code) \(flags)")
        }
    }
}

final class HistoryPagingTests: XCTestCase {
    func testTheListLoadsInPagesAndSearchesShowTheBestPage() throws {
        let rig = try Rig(self)
        rig.controller.start()
        for n in 0..<250 {
            rig.clock.now = demoNow + UInt64(n)
            rig.copy("demo item \(n)")
            rig.scheduler.fireRepeating()
        }
        let model = HistoryViewModel(controller: rig.controller)

        XCTAssertEqual(model.items.count, 200)
        XCTAssertTrue(model.hasMore)
        model.loadMore()
        XCTAssertEqual(model.items.count, 250)
        XCTAssertFalse(model.hasMore)
        model.query = "demo"
        XCTAssertEqual(model.items.count, 200)
        model.query = ""
        model.selectedIndex = 198
        model.moveSelection(by: 1)
        XCTAssertEqual(model.items.count, 250, "moving to the last row fetches the next page")
    }
}
