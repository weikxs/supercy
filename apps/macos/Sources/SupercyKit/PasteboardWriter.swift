import Foundation

/// Turns an item's representations into pasteboard items, ready to write.
public enum PasteboardWriter {
    /// The pasteboard items for `representations`; with `plainText`, only the text form.
    public static func items(for representations: [PasteRepresentation], plainText: Bool) -> [[PasteboardEntry]] {
        if plainText, let text = plainTextOf(representations) {
            return [[entry(PasteboardType.plainText, text)]]
        }
        let files = representations.compactMap { rep -> [String]? in
            if case let .fileRef(names) = rep { return names }
            return nil
        }.flatMap { $0 }
        if !files.isEmpty {
            return files.map { [entry(PasteboardType.fileURL, $0)] }
        }
        return [representations.flatMap(entries)]
    }

    /// The plain text of an item, if it has any.
    public static func plainTextOf(_ representations: [PasteRepresentation]) -> String? {
        for rep in representations {
            switch rep {
            case let .plainText(text): return text
            case let .richText(_, _, plain): return plain
            case let .url(url, _): return url
            case let .fileRef(names): return names.joined(separator: "\n")
            case .image: continue
            }
        }
        return nil
    }

    private static func entries(_ rep: PasteRepresentation) -> [PasteboardEntry] {
        switch rep {
        case let .plainText(text):
            return [entry(PasteboardType.plainText, text)]
        case let .richText(rtf, html, plain):
            var out: [PasteboardEntry] = []
            if let rtf { out.append(PasteboardEntry(type: PasteboardType.rtf, data: rtf)) }
            if let html { out.append(entry(PasteboardType.html, html)) }
            out.append(entry(PasteboardType.plainText, plain))
            return out
        case let .url(url, _):
            return [entry(PasteboardType.url, url), entry(PasteboardType.plainText, url)]
        case let .image(format, _, _, bytes):
            return [PasteboardEntry(type: imageType(format), data: bytes)]
        case let .fileRef(names):
            return names.map { entry(PasteboardType.fileURL, $0) }
        }
    }

    private static func imageType(_ format: ImageFormat) -> String {
        switch format {
        case .png: return PasteboardType.png
        case .tiff: return PasteboardType.tiff
        case .jpeg: return PasteboardType.jpeg
        case .gif: return PasteboardType.gif
        }
    }

    private static func entry(_ type: String, _ text: String) -> PasteboardEntry {
        PasteboardEntry(type: type, data: Data(text.utf8))
    }
}
