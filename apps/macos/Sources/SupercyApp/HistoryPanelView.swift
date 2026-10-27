import AppKit
import Combine

/// The history panel in plain AppKit: search field, list, preview and key hints. AppKit keeps
/// the menu bar process small (no SwiftUI runtime); the state and actions are the view model's,
/// keys are handled by the panel controller (`KeyCommand`).
final class HistoryPanelView: NSView, NSTableViewDataSource, NSTableViewDelegate, NSTextFieldDelegate {
    private let model: HistoryViewModel
    private let settings: Settings?
    private var subscriptions: Set<AnyCancellable> = []

    private let searchField = NSTextField()
    private let problemLabel = NSTextField(labelWithString: "")
    private let table = NSTableView()
    /// A plain label, not a text view: text views pull in the Services menu machinery.
    private let previewText = NSTextField(wrappingLabelWithString: "")
    private let previewScroll = NSView()
    private let detailsLabel = NSTextField(wrappingLabelWithString: "")
    private static let listWidth: CGFloat = 380
    private static let panelWidth: CGFloat = 760
    private static let fontSize: CGFloat = 13
    /// Most characters of text shown in the preview.
    private static let maxPreviewChars = 4_000
    /// Measurement diagnosis only (isolated mode): no thumbnails.
    private static let thumbnailsOff = ProcessInfo.processInfo.environment["SUPERCY_MEASURE_NO_THUMBNAILS"] == "1"

    init(model: HistoryViewModel, isPaused: Bool) {
        self.model = model
        settings = model.settings
        let rows = CGFloat(settings?.visibleItems ?? 20)
        super.init(frame: NSRect(x: 0, y: 0, width: Self.panelWidth, height: rows * RowPresentation.rowHeight + 110))
        build(isPaused: isPaused)
        model.$items.sink { [weak self] _ in self?.reload() }.store(in: &subscriptions)
        model.$selectedIndex.sink { [weak self] index in self?.select(index) }.store(in: &subscriptions)
        model.$queryProblem.sink { [weak self] problem in
            self?.problemLabel.stringValue = problem == nil ? "" : L10n.text(.invalidRegex)
        }.store(in: &subscriptions)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    /// The search field takes the keyboard when the panel opens.
    func focusSearch() {
        window?.makeFirstResponder(searchField)
    }

    // MARK: Layout

    private func build(isPaused: Bool) {
        searchField.placeholderString = L10n.text(.searchPlaceholder)
        searchField.font = .systemFont(ofSize: 17)
        searchField.isBordered = false
        searchField.drawsBackground = false
        searchField.focusRingType = .none
        searchField.delegate = self
        searchField.setAccessibilityLabel(L10n.text(.searchPlaceholder))
        problemLabel.textColor = .systemRed
        problemLabel.font = .systemFont(ofSize: 11)
        let paused = NSTextField(labelWithString: isPaused ? L10n.text(.paused) : "")
        paused.textColor = .systemOrange
        let header = NSStackView(views: [searchField, paused])
        let hints = NSTextField(labelWithString: L10n.text(.keyHints))
        hints.font = .systemFont(ofSize: 11)
        hints.textColor = .secondaryLabelColor
        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("item"))
        table.addTableColumn(column)
        table.headerView = nil
        table.dataSource = self
        table.delegate = self
        table.target = self
        table.action = #selector(clicked)
        table.style = .plain
        table.intercellSpacing = .zero
        let listScroll = NSScrollView()
        listScroll.documentView = table
        listScroll.hasVerticalScroller = true
        listScroll.drawsBackground = false
        previewText.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        previewText.isSelectable = false
        previewText.maximumNumberOfLines = 40
        previewText.lineBreakMode = .byCharWrapping
        previewText.translatesAutoresizingMaskIntoConstraints = false
        previewScroll.addSubview(previewText)
        NSLayoutConstraint.activate([
            previewText.topAnchor.constraint(equalTo: previewScroll.topAnchor),
            previewText.leadingAnchor.constraint(equalTo: previewScroll.leadingAnchor),
            previewText.trailingAnchor.constraint(equalTo: previewScroll.trailingAnchor),
        ])
        detailsLabel.font = .systemFont(ofSize: 11)
        detailsLabel.textColor = .secondaryLabelColor
        layout(header: header, list: listScroll, hints: hints)
    }

    private func layout(header: NSView, list: NSScrollView, hints: NSView) {
        let preview = NSView()
        for view in [previewScroll, detailsLabel] { preview.addSubview(view) }
        for view in [header, problemLabel, list, preview, hints] { addSubview(view) }
        for view in [header, problemLabel, list, preview, hints, previewScroll, detailsLabel] {
            view.translatesAutoresizingMaskIntoConstraints = false
        }
        NSLayoutConstraint.activate([
            header.topAnchor.constraint(equalTo: topAnchor, constant: 10),
            header.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            header.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            problemLabel.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 2),
            problemLabel.leadingAnchor.constraint(equalTo: header.leadingAnchor),
            list.topAnchor.constraint(equalTo: problemLabel.bottomAnchor, constant: 6),
            list.leadingAnchor.constraint(equalTo: leadingAnchor),
            list.widthAnchor.constraint(equalToConstant: Self.listWidth),
            list.bottomAnchor.constraint(equalTo: hints.topAnchor, constant: -6),
            preview.topAnchor.constraint(equalTo: list.topAnchor),
            preview.leadingAnchor.constraint(equalTo: list.trailingAnchor, constant: 10),
            preview.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
            preview.bottomAnchor.constraint(equalTo: list.bottomAnchor),
            previewScroll.topAnchor.constraint(equalTo: preview.topAnchor),
            previewScroll.leadingAnchor.constraint(equalTo: preview.leadingAnchor),
            previewScroll.trailingAnchor.constraint(equalTo: preview.trailingAnchor),
            previewScroll.bottomAnchor.constraint(equalTo: detailsLabel.topAnchor, constant: -8),
            detailsLabel.leadingAnchor.constraint(equalTo: preview.leadingAnchor),
            detailsLabel.trailingAnchor.constraint(equalTo: preview.trailingAnchor),
            detailsLabel.bottomAnchor.constraint(equalTo: preview.bottomAnchor),
            hints.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            hints.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -6),
        ])
    }

    // MARK: Data

    private func reload() {
        table.reloadData()
        select(model.selectedIndex)
    }

    private func select(_ index: Int) {
        guard model.items.indices.contains(index) else {
            showPreview(nil)
            return
        }
        table.selectRowIndexes(IndexSet(integer: index), byExtendingSelection: false)
        table.scrollRowToVisible(index)
        showPreview(model.items[index])
    }

    private func showPreview(_ item: ItemSummary?) {
        guard let item else {
            previewText.stringValue = model.query.isEmpty ? L10n.text(.emptyHistory) : L10n.text(.noMatches)
            detailsLabel.stringValue = ""
            return
        }
        // Images show only their row thumbnail and details: no large preview is decoded.
        if item.kind == .image {
            previewText.stringValue = L10n.text(.kindImage)
        } else {
            let payload = model.payload(of: item)
            previewText.stringValue = String((PasteboardWriter.plainTextOf(payload) ?? "").prefix(Self.maxPreviewChars))
        }
        detailsLabel.stringValue = RowPresentation.details(for: item)
    }

    @objc private func clicked() {
        let row = table.clickedRow
        guard model.items.indices.contains(row) else { return }
        model.selectedIndex = row
        model.choose(model.items[row])
    }

    func controlTextDidChange(_ notification: Notification) {
        model.query = searchField.stringValue
    }

    func numberOfRows(in tableView: NSTableView) -> Int {
        model.items.count
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        let row = table.selectedRow
        if row >= 0, row != model.selectedIndex { model.selectedIndex = row }
    }

    private var imageHeight: CGFloat { CGFloat(settings?.imagePreviewHeight ?? 40) }

    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
        guard model.items.indices.contains(row), model.items[row].kind == .image else { return RowPresentation.rowHeight }
        return max(RowPresentation.rowHeight, imageHeight + 6)
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard model.items.indices.contains(row) else { return nil }
        if row >= model.items.count - 20 { DispatchQueue.main.async { [weak self] in self?.model.loadMore() } }
        let cell = (tableView.makeView(withIdentifier: HistoryCell.identifier, owner: self) as? HistoryCell) ?? HistoryCell()
        let item = model.items[row]
        let thumbnail = item.kind == .image && !Self.thumbnailsOff
            ? model.thumbnail(of: item, maxPixels: Int(imageHeight * (window?.backingScaleFactor ?? 2)))
            : nil
        cell.show(item: item, index: row, settings: settings, thumbnail: thumbnail, imageHeight: imageHeight,
                  fontSize: Self.fontSize)
        return cell
    }
}

/// One row: icon or thumbnail, title, pin mark and shortcut hint.
final class HistoryCell: NSTableCellView {
    static let identifier = NSUserInterfaceItemIdentifier("HistoryCell")
    private let icon = NSImageView()
    private let title = NSTextField(labelWithString: "")
    private let hint = NSTextField(labelWithString: "")
    private var iconWidth: NSLayoutConstraint!
    private var iconHeight: NSLayoutConstraint!

    init() {
        super.init(frame: .zero)
        identifier = Self.identifier
        title.lineBreakMode = .byTruncatingTail
        hint.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        hint.textColor = .secondaryLabelColor
        icon.imageScaling = .scaleProportionallyDown
        for view in [icon, title, hint] {
            view.translatesAutoresizingMaskIntoConstraints = false
            addSubview(view)
        }
        iconWidth = icon.widthAnchor.constraint(equalToConstant: 16)
        iconHeight = icon.heightAnchor.constraint(equalToConstant: 16)
        title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        NSLayoutConstraint.activate([
            iconWidth, iconHeight,
            icon.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            icon.centerYAnchor.constraint(equalTo: centerYAnchor),
            title.leadingAnchor.constraint(equalTo: icon.trailingAnchor, constant: 8),
            title.centerYAnchor.constraint(equalTo: centerYAnchor),
            hint.leadingAnchor.constraint(greaterThanOrEqualTo: title.trailingAnchor, constant: 6),
            hint.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
            hint.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    func show(item: ItemSummary, index: Int, settings: Settings?, thumbnail: CGImage?, imageHeight: CGFloat,
              fontSize: CGFloat) {
        if let thumbnail {
            icon.image = NSImage(cgImage: thumbnail, size: .zero)
            iconHeight.constant = imageHeight
            iconWidth.constant = min(imageHeight * 3, imageHeight * CGFloat(thumbnail.width) / CGFloat(max(1, thumbnail.height)))
        } else {
            icon.image = NSImage(systemSymbolName: RowPresentation.symbol(item.kind), accessibilityDescription: nil)
            icon.contentTintColor = .secondaryLabelColor
            iconHeight.constant = 16
            iconWidth.constant = 16
        }
        title.attributedStringValue = RowPresentation.title(for: item, showSpecialChars: settings?.showSpecialChars ?? false,
                                                           fontSize: fontSize)
        let pinned = item.pinned ? "📌 " : ""
        let locked = item.sensitiveUntilMs != nil ? "🔒 " : ""
        hint.stringValue = locked + pinned + RowPresentation.hint(for: item, index: index)
        setAccessibilityLabel(title.stringValue)
    }
}
