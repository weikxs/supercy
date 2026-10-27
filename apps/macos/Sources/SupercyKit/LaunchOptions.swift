import Foundation

/// How the app was started.
///
/// `--isolated` is for measurements and checks: an in-memory pasteboard (the user's clipboard is
/// never read), an in-memory keystore (the keychain is never used), no global shortcut, and a
/// data directory from `SUPERCY_DATA_DIR`, which must be under `/tmp`.
public struct LaunchOptions: Equatable {
    public let isolated: Bool
    public let dataDirectory: URL

    /// Data under /tmp only, for the isolated mode.
    static let allowedIsolatedPrefixes = ["/tmp/", "/private/tmp/"]

    public enum Problem: Error, Equatable {
        case isolatedNeedsTemporaryDataDirectory
    }

    public static func parse(arguments: [String], environment: [String: String], defaultDataDirectory: URL) -> Result<LaunchOptions, Problem> {
        guard arguments.contains("--isolated") else {
            return .success(LaunchOptions(isolated: false, dataDirectory: defaultDataDirectory))
        }
        guard let path = environment["SUPERCY_DATA_DIR"],
              allowedIsolatedPrefixes.contains(where: { path.hasPrefix($0) }),
              !path.contains("/../")
        else {
            return .failure(.isolatedNeedsTemporaryDataDirectory)
        }
        return .success(LaunchOptions(isolated: true, dataDirectory: URL(fileURLWithPath: path, isDirectory: true)))
    }

    /// `~/Library/Application Support/Supercy`.
    public static func standardDataDirectory() -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library/Application Support")
        return base.appendingPathComponent("Supercy", isDirectory: true)
    }
}
