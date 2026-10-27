import AppKit

/// What a history row and the preview pane show, as plain values (no views), so the AppKit
/// panel stays thin and the formatting is tested.
public enum RowPresentation {
    /// Row height in points for rows without a thumbnail.
    public static let rowHeight: CGFloat = 26

    /// SF Symbol for an item kind.
    public static func symbol(_ kind: ItemKind) -> String {
        switch kind {
        case .text: return "text.alignleft"
        case .richText: return "textformat"
        case .url: return "link"
        case .image: return "photo"
        case .fileRef: return "doc"
        }
    }

    /// The shortcut hint: ⌃ + letter for pinned items, ⌘1–⌘9 for the first nine rows.
    public static func hint(for item: ItemSummary, index: Int) -> String {
        if let key = item.pinKey { return "⌃" + key.uppercased() }
        return index < 9 ? "⌘\(index + 1)" : ""
    }

    /// The row title with matches in bold.
    public static func title(for item: ItemSummary, showSpecialChars: Bool, fontSize: CGFloat) -> NSAttributedString {
        if item.kind == .image {
            let size = item.imageWidth.map { "\($0)×\(item.imageHeight ?? 0)" } ?? ""
            return NSAttributedString(string: "\(L10n.text(.kindImage)) \(size)", attributes: [.font: NSFont.systemFont(ofSize: fontSize)])
        }
        let title = PreviewFormatter.title(item.preview, showSpecialChars: showSpecialChars)
        return highlighted(title, matches: item.matches, fontSize: fontSize)
    }

    /// `text` with the UTF-16 `matches` in bold.
    public static func highlighted(_ text: String, matches: [TextRange], fontSize: CGFloat) -> NSAttributedString {
        let result = NSMutableAttributedString(string: text, attributes: [.font: NSFont.systemFont(ofSize: fontSize)])
        for range in matches {
            let nsRange = NSRange(location: Int(range.location), length: Int(range.length))
            guard NSMaxRange(nsRange) <= result.length else { continue }
            result.addAttribute(.font, value: NSFont.boldSystemFont(ofSize: fontSize), range: nsRange)
        }
        return result
    }

    /// The details under the preview: source, copies, times, size, sensitivity, truncation.
    public static func details(for item: ItemSummary) -> String {
        let date = { (ms: UInt64) in
            Date(timeIntervalSince1970: Double(ms) / 1000).formatted(date: .abbreviated, time: .shortened)
        }
        var lines: [String] = []
        if let name = item.sourceName { lines.append("\(L10n.text(.copiedFrom)): \(name)") }
        lines.append("\(L10n.text(.copies)): \(item.copyCount)")
        lines.append("\(L10n.text(.firstCopied)): \(date(item.firstCopiedMs))")
        lines.append("\(L10n.text(.lastUsed)): \(date(item.lastUsedMs))")
        lines.append("\(L10n.text(.size)): \(PreviewFormatter.size(item.sizeBytes))")
        if let until = item.sensitiveUntilMs { lines.append("\(L10n.text(.sensitiveUntil)): \(date(until))") }
        if item.truncated { lines.append(L10n.text(.truncated)) }
        return lines.joined(separator: "\n")
    }
}
