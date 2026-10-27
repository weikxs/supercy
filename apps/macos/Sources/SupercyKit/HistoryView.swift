import AppKit
import SwiftUI

/// The history panel: search field, list, preview pane and key hints. Keys are handled by the
/// panel (see `KeyCommand`); the view handles mouse clicks.
public struct HistoryView: View {
    @ObservedObject var model: HistoryViewModel
    let isPaused: Bool
    @FocusState private var searchFocused: Bool

    public init(model: HistoryViewModel, isPaused: Bool) {
        self.model = model
        self.isPaused = isPaused
    }

    private var settings: Settings? { model.settings }
    static let rowHeight: CGFloat = 26

    public var body: some View {
        VStack(spacing: 0) {
            searchField
            Divider()
            HStack(spacing: 0) {
                list.frame(width: 380)
                Divider()
                PreviewPane(model: model).frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            Divider()
            Text(L10n.text(.keyHints))
                .font(.caption)
                .foregroundStyle(.secondary)
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(width: 760, height: CGFloat(settings?.visibleItems ?? 20) * Self.rowHeight + 110)
        .onAppear { searchFocused = true }
    }

    private var searchField: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                TextField(L10n.text(.searchPlaceholder), text: $model.query)
                    .textFieldStyle(.plain)
                    .font(.title3)
                    .focused($searchFocused)
                    .accessibilityLabel(L10n.text(.searchPlaceholder))
                if isPaused {
                    Label(L10n.text(.paused), systemImage: "pause.circle").foregroundStyle(.orange).font(.caption)
                }
            }
            if model.queryProblem != nil {
                Text(L10n.text(.invalidRegex)).font(.caption).foregroundStyle(.red)
            }
        }
        .padding(10)
    }

    private var list: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(model.items.enumerated()), id: \.element.id) { index, item in
                        HistoryRow(item: item, index: index, selected: index == model.selectedIndex, settings: settings)
                            .id(item.id)
                            .contentShape(Rectangle())
                            .onTapGesture {
                                model.selectedIndex = index
                                model.choose(item)
                            }
                    }
                    if model.items.isEmpty {
                        Text(L10n.text(model.query.isEmpty ? .emptyHistory : .noMatches))
                            .foregroundStyle(.secondary)
                            .padding(20)
                    }
                }
            }
            .onChange(of: model.selectedIndex) { _, index in
                guard model.items.indices.contains(index) else { return }
                proxy.scrollTo(model.items[index].id)
            }
        }
    }
}

/// One row: kind icon, title with highlights, pin letter or ⌘ number.
struct HistoryRow: View {
    let item: ItemSummary
    let index: Int
    let selected: Bool
    let settings: Settings?

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: Self.symbol(item.kind)).frame(width: 16).foregroundStyle(.secondary)
            if item.pinned { Image(systemName: "pin.fill").font(.caption).foregroundStyle(.orange) }
            Text(title).lineLimit(1).truncationMode(.tail)
            Spacer(minLength: 4)
            if item.sensitiveUntilMs != nil { Image(systemName: "lock.fill").font(.caption).foregroundStyle(.secondary) }
            Text(shortcutHint).font(.caption.monospaced()).foregroundStyle(.secondary)
        }
        .padding(.horizontal, 10)
        .frame(height: HistoryView.rowHeight)
        .background(selected ? Color.accentColor.opacity(0.25) : Color.clear)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    private var title: AttributedString {
        let special = settings?.showSpecialChars ?? false
        if item.kind == .image {
            let size = item.imageWidth.map { w in "\(w)×\(item.imageHeight ?? 0)" } ?? ""
            return AttributedString("\(L10n.text(.kindImage)) \(size)")
        }
        let title = PreviewFormatter.title(item.preview, showSpecialChars: special)
        return PreviewFormatter.highlighted(title, matches: item.matches)
    }

    private var shortcutHint: String {
        if let key = item.pinKey { return "⌃" + key.uppercased() }
        return index < 9 ? "⌘\(index + 1)" : ""
    }

    static func symbol(_ kind: ItemKind) -> String {
        switch kind {
        case .text: return "text.alignleft"
        case .richText: return "textformat"
        case .url: return "link"
        case .image: return "photo"
        case .fileRef: return "doc"
        }
    }
}

/// The selected item's full content and details.
struct PreviewPane: View {
    @ObservedObject var model: HistoryViewModel
    /// Most characters of text shown in the preview.
    static let maxPreviewChars = 20_000

    var body: some View {
        if let item = model.selected {
            VStack(alignment: .leading, spacing: 8) {
                content(of: item)
                Divider()
                details(of: item)
            }
            .padding(10)
        } else {
            Color.clear
        }
    }

    @ViewBuilder
    private func content(of item: ItemSummary) -> some View {
        let payload = model.payload(of: item)
        if case let .image(_, _, _, bytes)? = payload.first(where: { if case .image = $0 { return true } else { return false } }),
           let image = NSImage(data: bytes) {
            Image(nsImage: image).resizable().scaledToFit().frame(maxHeight: .infinity)
        } else {
            ScrollView {
                Text(String((PasteboardWriter.plainTextOf(payload) ?? "").prefix(Self.maxPreviewChars)))
                    .font(.system(.body, design: .monospaced))
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private func details(of item: ItemSummary) -> some View {
        let date = { (ms: UInt64) in Date(timeIntervalSince1970: Double(ms) / 1000).formatted(date: .abbreviated, time: .shortened) }
        return Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 2) {
            if let name = item.sourceName { row(.copiedFrom, name) }
            row(.copies, "\(item.copyCount)")
            row(.firstCopied, date(item.firstCopiedMs))
            row(.lastUsed, date(item.lastUsedMs))
            row(.size, PreviewFormatter.size(item.sizeBytes))
            if let until = item.sensitiveUntilMs { row(.sensitiveUntil, date(until)) }
            if item.truncated { GridRow { Text(L10n.text(.truncated)).gridCellColumns(2) } }
        }
        .font(.caption)
        .foregroundStyle(.secondary)
    }

    private func row(_ key: L10nKey, _ value: String) -> some View {
        GridRow {
            Text(L10n.text(key))
            Text(value)
        }
    }
}
