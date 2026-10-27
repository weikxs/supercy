import Foundation

/// Connects the pasteboard, the core engine and the paste simulation. All clipboard decisions
/// are the core's; this class only moves data and keeps the single tick timer.
public final class ClipboardController {
    public let engine: SupercyEngine
    private let pasteboard: PasteboardReading & PasteboardWriting
    private let reader: SnapshotReader
    private let paster: Pasting
    private let scheduler: Scheduling
    private let clock: Clock
    private var watcher: PasteboardWatcher!
    private var tickTimer: Cancellable?
    /// Between `start` and `stop`.
    private var started = false

    /// Called after the history changed (new copy, expiry, ...), on the main thread.
    public var onHistoryChange: (() -> Void)?
    /// Called after an ingest, with the outcome (for the menu bar's latest copy).
    public var onIngest: ((IngestOutcome) -> Void)?

    public init(
        engine: SupercyEngine,
        pasteboard: PasteboardReading & PasteboardWriting,
        frontApp: FrontAppProviding,
        paster: Pasting,
        scheduler: Scheduling,
        clock: Clock
    ) {
        self.engine = engine
        self.pasteboard = pasteboard
        self.reader = SnapshotReader(pasteboard: pasteboard, frontApp: frontApp)
        self.paster = paster
        self.scheduler = scheduler
        self.clock = clock
        let interval = Double((try? engine.settings().pollIntervalMs) ?? 500) / 1000
        watcher = PasteboardWatcher(pasteboard: pasteboard, scheduler: scheduler, interval: interval) { [weak self] in
            self?.pasteboardChanged()
        }
    }

    public var isWatching: Bool { watcher.isRunning }

    /// Starts watching unless recording is paused; runs the first tick.
    public func start() {
        started = true
        if !isPaused { watcher.start() }
        tick()
    }

    public func stop() {
        started = false
        watcher.stop()
        tickTimer?.cancel()
        tickTimer = nil
    }

    // MARK: Recording

    func pasteboardChanged() {
        let snapshot = reader.snapshot(at: clock.nowMs())
        guard let result = try? engine.ingest(snapshot: snapshot) else { return }
        schedule(result.nextTickMs)
        onIngest?(result.outcome)
        switch result.outcome {
        case .stored, .updated: onHistoryChange?()
        default: break
        }
    }

    public var isPaused: Bool {
        (try? engine.pauseStatus(nowMs: clock.nowMs()).paused) ?? false
    }

    /// Pauses recording for `minutes`, or until resumed when `nil`. The pasteboard is not even
    /// checked while paused.
    public func pause(minutes: Int?) {
        let until = minutes.map { clock.nowMs() + UInt64($0) * 60_000 }
        try? engine.pause(untilMs: until)
        watcher.stop()
        schedule(until)
    }

    /// Resumes recording. What was copied during the pause is not recorded.
    public func resume() {
        try? engine.resume()
        if started { watcher.start() }
    }

    public func skipNextCopy() {
        try? engine.skipNext()
    }

    /// Runs time-based work in the core (expiry, end of a timed pause) and schedules the next.
    public func tick() {
        let before = (try? engine.itemCount()) ?? 0
        let next = try? engine.tick(nowMs: clock.nowMs())
        // A timed pause that ended: recording is on again, but the watcher is still stopped.
        let resumed = started && !isPaused && !watcher.isRunning
        if resumed { watcher.start() }
        schedule(next ?? nil)
        if resumed || ((try? engine.itemCount()) ?? 0) != before { onHistoryChange?() }
    }

    private func schedule(_ atMs: UInt64?) {
        tickTimer?.cancel()
        tickTimer = nil
        guard let atMs else { return }
        let delay = Double(atMs.saturatingSubtract(clock.nowMs())) / 1000
        tickTimer = scheduler.after(delay) { [weak self] in self?.tick() }
    }

    public func applyPollInterval(_ ms: UInt32) {
        watcher.setInterval(Double(ms) / 1000)
    }

    // MARK: Choosing an item

    /// Puts item `id` on the pasteboard and, when `paste` is set and allowed, pastes it.
    public func choose(id: String, plainText: Bool, paste: Bool) -> ChooseResult {
        let representations: [PasteRepresentation]
        do {
            representations = try engine.payload(id: id)
        } catch {
            return .failed(String(describing: error))
        }
        pasteboard.write(items: PasteboardWriter.items(for: representations, plainText: plainText))
        watcher.resync()
        try? engine.markUsed(id: id, nowMs: clock.nowMs())
        onHistoryChange?()
        guard paste else { return .copied }
        guard paster.isTrusted else { return .needsAccessibility }
        paster.paste()
        return .pasted
    }
}

extension UInt64 {
    func saturatingSubtract(_ other: UInt64) -> UInt64 { self > other ? self - other : 0 }
}
