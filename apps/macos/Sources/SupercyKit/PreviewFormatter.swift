import Foundation

/// Turns item previews into what the list shows.
public enum PreviewFormatter {
    /// Longest title shown in a row.
    public static let titleLength = 120

    /// The row title: the preview on one line, with special characters shown when asked.
    public static func title(_ preview: String, showSpecialChars: Bool) -> String {
        let safe = displayable(preview)
        let text = showSpecialChars ? visible(safe) : safe
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

    /// Shown in place of characters that must not be rendered.
    static let placeholder: Character = "□"

    /// The text with U+FFFC (the attachment placeholder in rich text) and control characters
    /// other than line breaks and tabs replaced by `placeholder`, one for one: several U+FFFC
    /// in a line can hang CoreText, and control characters have no glyph.
    public static func displayable(_ text: String) -> String {
        guard text.unicodeScalars.contains(where: isUnrenderable) else { return text }
        return String(text.map { char -> Character in
            char.unicodeScalars.contains(where: isUnrenderable) ? placeholder : char
        })
    }

    private static func isUnrenderable(_ scalar: Unicode.Scalar) -> Bool {
        switch scalar {
        case "\n", "\r", "\t": return false
        case "\u{FFFC}": return true
        default: return scalar.properties.generalCategory == .control
        }
    }

    /// File size for people: `512 B`, `1.2 KB`, `3.4 MB`.
    public static func size(_ bytes: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: bytes), countStyle: .file)
    }
}
