import AppKit
import XCTest

final class SnapshotReaderTests: XCTestCase {
    private let frontApp = FakeFrontApp()

    func testEveryTypeOfEveryItemBecomesAnEntryWithTheFrontApp() {
        let pasteboard = InMemoryPasteboard()
        pasteboard.write(items: [
            [PasteboardEntry(type: PasteboardType.fileURL, data: Data("file:///tmp/supercy-test/a.txt".utf8))],
            [PasteboardEntry(type: PasteboardType.fileURL, data: Data("file:///tmp/supercy-test/b.txt".utf8)),
             PasteboardEntry(type: PasteboardType.concealed, data: nil)],
        ])

        let snapshot = SnapshotReader(pasteboard: pasteboard, frontApp: frontApp).snapshot(at: demoNow)

        XCTAssertEqual(snapshot.entries.map(\.typeId), [PasteboardType.fileURL, PasteboardType.fileURL, PasteboardType.concealed])
        XCTAssertEqual(snapshot.frontApp?.bundleId, "com.example.editor")
        XCTAssertEqual(snapshot.capturedAtMs, demoNow)
        XCTAssertNil(snapshot.entries[2].data)
    }

    func testOversizedDataIsPassedOnAsASizeOnly() {
        let pasteboard = InMemoryPasteboard()
        let big = Data(count: SnapshotReader.maxEntryBytes + 1)
        pasteboard.write(items: [[PasteboardEntry(type: PasteboardType.png, data: big)]])

        let entry = SnapshotReader(pasteboard: pasteboard, frontApp: frontApp).snapshot(at: demoNow).entries[0]

        XCTAssertNil(entry.data)
        XCTAssertEqual(entry.size, UInt64(big.count))
    }

    func testTIFFImagesAreConvertedToPNG() throws {
        let image = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 4, pixelsHigh: 3, bitsPerSample: 8,
                                                   samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                                   colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        let tiff = try XCTUnwrap(image.tiffRepresentation)
        let pasteboard = InMemoryPasteboard()
        pasteboard.write(items: [[PasteboardEntry(type: PasteboardType.tiff, data: tiff)]])

        let entries = SnapshotReader(pasteboard: pasteboard, frontApp: frontApp).snapshot(at: demoNow).entries

        XCTAssertEqual(entries.map(\.typeId), [PasteboardType.png, thumbnailType()])
        XCTAssertEqual(entries[0].data?.prefix(4), Data([0x89, 0x50, 0x4E, 0x47]))
    }
}

final class PasteboardWatcherTests: XCTestCase {
    func testReportsOnlyChangesAndOnlyWhileRunning() {
        let pasteboard = InMemoryPasteboard()
        let scheduler = ManualScheduler()
        var changes = 0
        let watcher = PasteboardWatcher(pasteboard: pasteboard, scheduler: scheduler, interval: 0.5) { changes += 1 }

        pasteboard.write(items: [])
        watcher.start()
        scheduler.fireRepeating()
        pasteboard.write(items: [])
        scheduler.fireRepeating()
        scheduler.fireRepeating()
        watcher.stop()
        pasteboard.write(items: [])
        scheduler.fireRepeating()

        XCTAssertEqual(changes, 1, "a change before start and one while stopped are not reported")
        XCTAssertFalse(watcher.isRunning)
        XCTAssertTrue(scheduler.activeRepeating.isEmpty)
    }

    func testResyncSwallowsOwnWrites() {
        let pasteboard = InMemoryPasteboard()
        let scheduler = ManualScheduler()
        var changes = 0
        let watcher = PasteboardWatcher(pasteboard: pasteboard, scheduler: scheduler, interval: 0.5) { changes += 1 }
        watcher.start()

        pasteboard.write(items: [])
        watcher.resync()
        watcher.check()

        XCTAssertEqual(changes, 0)
    }
}

final class PasteboardWriterTests: XCTestCase {
    func testRichTextKeepsEveryFormAndPlainModeKeepsText() {
        let reps: [PasteRepresentation] = [.richText(rtf: Data("{\\rtf1 Demo}".utf8), html: "<b>Demo</b>", plain: "Demo")]

        let rich = PasteboardWriter.items(for: reps, plainText: false)
        let plain = PasteboardWriter.items(for: reps, plainText: true)

        XCTAssertEqual(rich.first?.map(\.type), [PasteboardType.rtf, PasteboardType.html, PasteboardType.plainText])
        XCTAssertEqual(plain, [[PasteboardEntry(type: PasteboardType.plainText, data: Data("Demo".utf8))]])
    }

    func testLinksImagesAndFiles() {
        let link = PasteboardWriter.items(for: [.url(url: "https://example.com", title: nil)], plainText: false)
        let image = PasteboardWriter.items(for: [.image(format: .png, width: 1, height: 1, bytes: Data([1]))], plainText: false)
        let files = PasteboardWriter.items(for: [.fileRef(names: ["file:///tmp/a", "file:///tmp/b"])], plainText: false)

        XCTAssertEqual(link.first?.map(\.type), [PasteboardType.url, PasteboardType.plainText])
        XCTAssertEqual(image.first?.map(\.type), [PasteboardType.png])
        XCTAssertEqual(files.map { $0.map(\.type) }, [[PasteboardType.fileURL], [PasteboardType.fileURL]])
        XCTAssertNil(PasteboardWriter.plainTextOf([.image(format: .png, width: 1, height: 1, bytes: Data())]))
    }
}
