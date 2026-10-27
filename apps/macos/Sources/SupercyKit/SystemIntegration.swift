import Foundation

/// System features the settings screen shows and changes. The app implements it over
/// the system login-item service and the Accessibility API; tests use a fake.
public protocol SystemIntegration: AnyObject {
    var launchAtLogin: Bool { get set }
    var isAccessibilityTrusted: Bool { get }
    func openAccessibilitySettings()
}

/// A keystore in memory: used by tests and by the isolated mode. Never used for real data.
public final class MemoryKeystore: Keystore, @unchecked Sendable {
    private var secrets: [String: Data] = [:]
    private let lock = NSLock()

    public init() {}

    public func read(account: String) throws -> Data? {
        lock.lock(); defer { lock.unlock() }
        return secrets[account]
    }

    public func write(account: String, secret: Data) throws {
        lock.lock(); defer { lock.unlock() }
        secrets[account] = secret
    }

    public func delete(account: String) throws {
        lock.lock(); defer { lock.unlock() }
        secrets[account] = nil
    }
}
