import Combine
import CoreGraphics
import Foundation

/// The history list's state: query, results, selection and the actions on them. Keyboard
/// handling in the view maps keys to these methods.
public final class HistoryViewModel: ObservableObject {
    @Published public var query = "" { didSet { refresh() } }
    @Published public private(set) var items: [ItemSummary] = []
    @Published public var selectedIndex = 0
    /// Why the query is not a valid regular expression (regex mode), if it is not.
    @Published public private(set) var queryProblem: String?

    private let controller: ClipboardController
    /// Rows fetched at a time: the list fetches more while scrolling, a search shows the best
    /// `pageSize` matches. Keeps the work (and memory) of opening the panel small.
    public static let pageSize: UInt32 = 200
    /// Whether the plain list has more rows than fetched so far.
    public private(set) var hasMore = false

    /// Called when the panel should close (after choosing an item).
    public var onClose: (() -> Void)?
    /// Called when pasting needs the Accessibility permission.
    public var onNeedsAccessibility: (() -> Void)?
    /// Called when the user asks for the settings (⌘,).
    public var onOpenSettings: (() -> Void)?

    public init(controller: ClipboardController) {
        self.controller = controller
        refresh()
    }

    public var settings: Settings? { try? controller.engine.settings() }

    public var selected: ItemSummary? {
        items.indices.contains(selectedIndex) ? items[selectedIndex] : nil
    }

    /// Reloads the list for the current query, keeping the selection in range.
    public func refresh() {
        cachedPayload = nil
        let engine = controller.engine
        let mode = settings?.searchMode ?? .fuzzy
        queryProblem = queryError(query: query, mode: mode)
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty {
            let page = (try? engine.list(kind: nil, offset: 0, limit: Self.pageSize)) ?? []
            hasMore = page.count == Int(Self.pageSize)
            items = page
        } else {
            hasMore = false
            items = (try? engine.search(query: trimmed, mode: mode, kind: nil, limit: Self.pageSize)) ?? []
        }
        selectedIndex = items.isEmpty ? 0 : min(selectedIndex, items.count - 1)
    }

    /// Fetches the next page of the plain list (called when scrolling near the end).
    public func loadMore() {
        guard hasMore, query.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        let page = (try? controller.engine.list(kind: nil, offset: UInt32(items.count), limit: Self.pageSize)) ?? []
        hasMore = page.count == Int(Self.pageSize)
        if !page.isEmpty { items += page }
    }

    public func moveSelection(by offset: Int) {
        guard !items.isEmpty else { return }
        if selectedIndex + offset >= items.count - 1 { loadMore() }
        selectedIndex = max(0, min(items.count - 1, selectedIndex + offset))
    }

    /// ⌘1–⌘9: the n-th visible item (1-based).
    public func select(number: Int) -> ItemSummary? {
        let index = number - 1
        guard items.indices.contains(index) else { return nil }
        selectedIndex = index
        return items[index]
    }

    /// The pinned item with letter `key`.
    public func select(pinKey key: String) -> ItemSummary? {
        guard let index = items.firstIndex(where: { $0.pinned && $0.pinKey == key.lowercased() }) else { return nil }
        selectedIndex = index
        return items[index]
    }

    /// Chooses the selected item: copy, and paste unless `copyOnly`.
    @discardableResult
    public func chooseSelected(plainText: Bool? = nil, copyOnly: Bool = false) -> ChooseResult {
        guard let item = selected else { return .failed("nothing selected") }
        return choose(item, plainText: plainText, copyOnly: copyOnly)
    }

    @discardableResult
    public func choose(_ item: ItemSummary, plainText: Bool? = nil, copyOnly: Bool = false) -> ChooseResult {
        let settings = self.settings
        let plain = plainText ?? settings?.pastePlainByDefault ?? false
        let paste = !copyOnly && (settings?.pasteAutomatically ?? true)
        onClose?()
        let result = controller.choose(id: item.id, plainText: plain, paste: paste)
        if result == .needsAccessibility { onNeedsAccessibility?() }
        return result
    }

    public func togglePinSelected(nowMs: UInt64) {
        guard let item = selected else { return }
        try? controller.engine.setPinned(id: item.id, pinned: !item.pinned, nowMs: nowMs)
        refresh()
    }

    /// Rows of the pinned items when they can be rearranged: in the plain list, not in search
    /// results (pinned items are listed together, in their own order).
    public var pinnedRows: ClosedRange<Int>? {
        guard query.trimmingCharacters(in: .whitespaces).isEmpty,
              let first = items.firstIndex(where: \.pinned),
              let last = items.lastIndex(where: \.pinned) else { return nil }
        return first...last
    }

    /// Moves the pinned item at `fromRow` to the gap above `toRow` (one past the last pinned row
    /// means last), as a drop between rows does. Returns whether it moved.
    @discardableResult
    public func movePin(fromRow: Int, toRow: Int, nowMs: UInt64) -> Bool {
        guard let rows = pinnedRows, rows.contains(fromRow),
              (rows.lowerBound...rows.upperBound + 1).contains(toRow) else { return false }
        let item = items[fromRow]
        // The place among the pinned items once the moved one is taken out.
        let place = (toRow > fromRow ? toRow - 1 : toRow) - rows.lowerBound
        guard (try? controller.engine.movePin(id: item.id, toIndex: UInt32(place), nowMs: nowMs)) != nil else {
            return false
        }
        refresh()
        selectedIndex = items.firstIndex { $0.id == item.id } ?? selectedIndex
        return true
    }

    /// Moves the selected pinned item one place up (`-1`) or down (`1`) among the pinned items.
    public func moveSelectedPin(by offset: Int, nowMs: UInt64) {
        guard let rows = pinnedRows, rows.contains(selectedIndex), rows.contains(selectedIndex + offset) else { return }
        movePin(fromRow: selectedIndex, toRow: selectedIndex + (offset > 0 ? offset + 1 : offset), nowMs: nowMs)
    }

    public func deleteSelected() {
        guard let item = selected else { return }
        _ = try? controller.engine.delete(id: item.id)
        refresh()
    }

    public func clear(keepPinned: Bool) {
        _ = try? controller.engine.clear(keepPinned: keepPinned)
        refresh()
    }

    /// The full content of an item, for the preview pane. The last one is kept, because the
    /// pane redraws often; it is dropped with the panel.
    public func payload(of item: ItemSummary) -> [PasteRepresentation] {
        if let cached = cachedPayload, cached.id == item.id { return cached.representations }
        let representations = (try? controller.engine.payload(id: item.id)) ?? []
        cachedPayload = (item.id, representations)
        return representations
    }

    private var cachedPayload: (id: String, representations: [PasteRepresentation])?

    /// Thumbnails of image items, decoded at display size; dropped with the panel.
    public let thumbnails = ThumbnailCache()

    /// The thumbnail of an image item, at most `maxPixels` on its longer side.
    public func thumbnail(of item: ItemSummary, maxPixels: Int) -> CGImage? {
        guard item.kind == .image else { return nil }
        return thumbnails.thumbnail(for: item.id, maxPixels: maxPixels) {
            // The small thumbnail stored at copy time; only items from before it existed fall
            // back to the full image.
            if let stored = try? controller.engine.thumbnail(id: item.id) { return stored }
            let representations = (try? controller.engine.payload(id: item.id)) ?? []
            for case let .image(_, _, _, bytes) in representations { return bytes }
            return nil
        }
    }
}
