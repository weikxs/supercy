import Foundation

/// Checks the pasteboard's change count at a fixed interval — the only polling Supercy does —
/// and only while started. Stopped while recording is paused.
public final class PasteboardWatcher {
    private let pasteboard: PasteboardReading
    private let scheduler: Scheduling
    private let onChange: () -> Void
    private var interval: TimeInterval
    private var timer: Cancellable?
    private var lastChangeCount: Int

    public init(pasteboard: PasteboardReading, scheduler: Scheduling, interval: TimeInterval, onChange: @escaping () -> Void) {
        self.pasteboard = pasteboard
        self.scheduler = scheduler
        self.interval = interval
        self.onChange = onChange
        lastChangeCount = pasteboard.changeCount
    }

    public var isRunning: Bool { timer != nil }

    /// Starts checking. Changes made while stopped are not reported.
    public func start() {
        guard timer == nil else { return }
        resync()
        timer = scheduler.every(interval) { [weak self] in self?.check() }
    }

    public func stop() {
        timer?.cancel()
        timer = nil
    }

    /// Changes the interval (takes effect at once when running).
    public func setInterval(_ newInterval: TimeInterval) {
        guard newInterval != interval else { return }
        interval = newInterval
        if isRunning {
            stop()
            start()
        }
    }

    /// Accepts the current content without reporting it (after Supercy wrote it itself).
    public func resync() {
        lastChangeCount = pasteboard.changeCount
    }

    /// Reports a change, if any. Called by the timer.
    public func check() {
        let current = pasteboard.changeCount
        guard current != lastChangeCount else { return }
        lastChangeCount = current
        onChange()
    }
}
