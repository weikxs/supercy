import Foundation

/// File names for saved screenshots: `Supercy 2026-01-31 14.05.09.png` (the PRD's
/// `Supercy {date} {time}` template; dots in the time because Finder shows colons as slashes).
public enum CaptureFileName {
    public static let prefix = "Supercy"
    public static let pathExtension = "png"
    /// Tries " (2)", " (3)", ... up to this before giving up on finding a free name.
    static let maxSuffix = 999

    /// The name for a screenshot taken at `date`, in `timeZone`.
    public static func name(for date: Date, timeZone: TimeZone = .current) -> String {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.timeZone = timeZone
        formatter.dateFormat = "yyyy-MM-dd HH.mm.ss"
        return "\(prefix) \(formatter.string(from: date)).\(pathExtension)"
    }

    /// `name`, or `name (2)`, `name (3)`, ... — the first one for which `isTaken` is false.
    public static func unique(_ name: String, isTaken: (String) -> Bool) -> String {
        guard isTaken(name) else { return name }
        let url = URL(fileURLWithPath: name)
        let base = url.deletingPathExtension().lastPathComponent
        let ext = url.pathExtension
        for number in 2...maxSuffix {
            let candidate = ext.isEmpty ? "\(base) (\(number))" : "\(base) (\(number)).\(ext)"
            if !isTaken(candidate) { return candidate }
        }
        return name
    }
}

/// Delays offered for a timed screenshot (FR-10.7).
public enum CaptureDelay {
    public static let seconds = [3, 5, 10]
}
