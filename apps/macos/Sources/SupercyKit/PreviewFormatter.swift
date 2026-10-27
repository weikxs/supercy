import Foundation

/// Turns item previews into what the list shows.
public enum PreviewFormatter {
    /// Longest title shown in a row.
    public static let titleLength = 120

    /// The row title: the preview on one line, with special characters shown when asked.
    public static func title(_ preview: String, showSpecialChars: Bool) -> String {
        let text = showSpecialChars ? visible(preview) : preview
            .replacingOccurrences(of: "\r\n", with: " ")
            .replacingOccurrences(of: "\n", with: " ")
            .replacingOccurrences(of: "\t", with: " ")
        // Only trailing space is trimmed: trimming the start would shift the highlight ranges.
        let trimmed = showSpecialChars ? text : String(text.reversed().drop(while: \.isWhitespace).reversed())
        return trimmed.count > titleLength ? String(trimmed.prefix(titleLength)) + "…" : trimmed
    }

    /// Spaces, tabs and line breaks as visible symbols (one character each, so highlight ranges
    /// stay valid).
    public static func visible(_ text: String) -> String {
        String(text.map { char -> Character in
            switch char {
            case " ": return "·"
            case "\t": return "⇥"
            case "\n", "\r\n": return "↩"
            default: return char
            }
        })
    }

    /// The preview with matched ranges marked, for the row title.
    public static func highlighted(_ title: String, matches: [TextRange]) -> AttributedString {
        var attributed = AttributedString(title)
        let nsTitle = title as NSString
        for range in matches {
            let nsRange = NSRange(location: Int(range.location), length: Int(range.length))
            guard NSMaxRange(nsRange) <= nsTitle.length,
                  let swiftRange = Range(nsRange, in: title),
                  let lower = AttributedString.Index(swiftRange.lowerBound, within: attributed),
                  let upper = AttributedString.Index(swiftRange.upperBound, within: attributed)
            else { continue }
            attributed[lower..<upper].inlinePresentationIntent = .stronglyEmphasized
        }
        return attributed
    }

    /// File size for people: `512 B`, `1.2 KB`, `3.4 MB`.
    public static func size(_ bytes: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: bytes), countStyle: .file)
    }
}
