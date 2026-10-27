import Foundation

/// Something that can be cancelled, such as a scheduled timer.
public protocol Cancellable: AnyObject {
    func cancel()
}

/// Runs code later or repeatedly. The app uses `Timer`; tests use `ManualScheduler`.
public protocol Scheduling: AnyObject {
    /// Calls `block` every `interval` seconds until cancelled.
    func every(_ interval: TimeInterval, _ block: @escaping () -> Void) -> Cancellable
    /// Calls `block` once after `delay` seconds unless cancelled.
    func after(_ delay: TimeInterval, _ block: @escaping () -> Void) -> Cancellable
}

/// A clock in milliseconds since 1970.
public protocol Clock: AnyObject {
    func nowMs() -> UInt64
}

/// The system clock.
public final class SystemClock: Clock {
    public init() {}

    public func nowMs() -> UInt64 {
        UInt64(max(0, Date().timeIntervalSince1970 * 1000))
    }
}

/// `Timer`-based scheduling on the main run loop, with a tolerance so the system can batch
/// wake-ups (energy).
public final class TimerScheduler: Scheduling {
    private final class TimerToken: Cancellable {
        let timer: Timer
        init(_ timer: Timer) { self.timer = timer }
        func cancel() { timer.invalidate() }
    }

    public init() {}

    public func every(_ interval: TimeInterval, _ block: @escaping () -> Void) -> Cancellable {
        let timer = Timer(timeInterval: interval, repeats: true) { _ in block() }
        timer.tolerance = interval * 0.2
        RunLoop.main.add(timer, forMode: .common)
        return TimerToken(timer)
    }

    public func after(_ delay: TimeInterval, _ block: @escaping () -> Void) -> Cancellable {
        let timer = Timer(timeInterval: max(0, delay), repeats: false) { _ in block() }
        timer.tolerance = 0.5
        RunLoop.main.add(timer, forMode: .common)
        return TimerToken(timer)
    }
}
