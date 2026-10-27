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

/// A keystore in a file, for the isolated mode only: lets a measurement quit and relaunch with
/// the same history. The file lives in the data directory under /tmp; never used for real data.
public final class FileKeystore: Keystore, @unchecked Sendable {
    private let directory: URL
    private let lock = NSLock()

    public init(directory: URL) {
        self.directory = directory
    }

    private func file(_ account: String) -> URL {
        directory.appendingPathComponent("isolated-keystore-\(account.filter { $0.isLetter || $0.isNumber || $0 == "-" })")
    }

    public func read(account: String) throws -> Data? {
        lock.lock(); defer { lock.unlock() }
        return try? Data(contentsOf: file(account))
    }

    public func write(account: String, secret: Data) throws {
        lock.lock(); defer { lock.unlock() }
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try secret.write(to: file(account), options: [.atomic])
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file(account).path)
        } catch {
            throw KeystoreError.Unavailable(message: error.localizedDescription)
        }
    }

    public func delete(account: String) throws {
        lock.lock(); defer { lock.unlock() }
        try? FileManager.default.removeItem(at: file(account))
    }
}
