import Combine
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
    /// Most rows shown; the list scrolls.
    public static let maxRows: UInt32 = 10_000

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
            items = (try? engine.list(kind: nil, offset: 0, limit: Self.maxRows)) ?? []
        } else {
            items = (try? engine.search(query: trimmed, mode: mode, kind: nil, limit: Self.maxRows)) ?? []
        }
        selectedIndex = items.isEmpty ? 0 : min(selectedIndex, items.count - 1)
    }

    public func moveSelection(by offset: Int) {
        guard !items.isEmpty else { return }
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
}
