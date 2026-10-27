import Foundation
import XCTest

/// A scheduler that runs nothing until the test says so.
final class ManualScheduler: Scheduling {
    final class Token: Cancellable {
        var cancelled = false
        func cancel() { cancelled = true }
    }

    private(set) var repeating: [(interval: TimeInterval, token: Token, block: () -> Void)] = []
    private(set) var once: [(delay: TimeInterval, token: Token, block: () -> Void)] = []

    func every(_ interval: TimeInterval, _ block: @escaping () -> Void) -> Cancellable {
        let token = Token()
        repeating.append((interval, token, block))
        return token
    }

    func after(_ delay: TimeInterval, _ block: @escaping () -> Void) -> Cancellable {
        let token = Token()
        once.append((delay, token, block))
        return token
    }

    /// Repeating timers that are still running.
    var activeRepeating: [(interval: TimeInterval, token: Token, block: () -> Void)] {
        repeating.filter { !$0.token.cancelled }
    }

    /// The pending one-shot timer, if any.
    var pendingOnce: (delay: TimeInterval, token: Token, block: () -> Void)? {
        once.last { !$0.token.cancelled }
    }

    /// Fires every running repeating timer once.
    func fireRepeating() {
        activeRepeating.forEach { $0.block() }
    }
}

final class ManualClock: Clock {
    var now: UInt64
    init(_ now: UInt64) { self.now = now }
    func nowMs() -> UInt64 { now }
}

final class FakePaster: Pasting {
    var isTrusted = false
    private(set) var pasteCount = 0
    private(set) var trustRequests = 0
    func requestTrust() { trustRequests += 1 }
    func paste() { pasteCount += 1 }
}

final class FakeFrontApp: FrontAppProviding {
    var app: SourceApp? = SourceApp(bundleId: "com.example.editor", name: "Demo Editor")
    func frontApp() -> SourceApp? { app }
}

final class FakeSystem: SystemIntegration {
    var launchAtLogin = false
    var isAccessibilityTrusted = false
    private(set) var openedSettings = 0
    func openAccessibilitySettings() { openedSettings += 1 }
}

let demoNow: UInt64 = 1_700_000_000_000

/// A fresh data directory under the test run's temporary directory (`/tmp/supercy/...`),
/// removed at the end of the test.
func temporaryDirectory(_ test: XCTestCase) throws -> URL {
    let dir = FileManager.default.temporaryDirectory
        .appendingPathComponent("supercy-kit-tests", isDirectory: true)
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    test.addTeardownBlock { try? FileManager.default.removeItem(at: dir) }
    return dir
}

/// An engine on a temporary directory with an in-memory keystore.
func makeEngine(_ test: XCTestCase) throws -> SupercyEngine {
    try SupercyEngine.open(dataDir: try temporaryDirectory(test).path, keystore: MemoryKeystore(), nowMs: demoNow)
}

/// Everything a controller needs, all fake except the engine.
struct Rig {
    let engine: SupercyEngine
    let pasteboard = InMemoryPasteboard()
    let frontApp = FakeFrontApp()
    let paster = FakePaster()
    let scheduler = ManualScheduler()
    let clock = ManualClock(demoNow)
    let controller: ClipboardController

    init(_ test: XCTestCase) throws {
        engine = try makeEngine(test)
        controller = ClipboardController(engine: engine, pasteboard: pasteboard, frontApp: frontApp,
                                         paster: paster, scheduler: scheduler, clock: clock)
    }

    /// Puts plain text on the fake pasteboard, as an app copying it would.
    func copy(_ text: String, extraTypes: [String] = []) {
        let entries = [PasteboardEntry(type: PasteboardType.plainText, data: Data(text.utf8))]
            + extraTypes.map { PasteboardEntry(type: $0, data: Data()) }
        pasteboard.write(items: [entries])
    }

    var previews: [String] {
        ((try? engine.list(kind: nil, offset: 0, limit: 100)) ?? []).map(\.preview)
    }
}
