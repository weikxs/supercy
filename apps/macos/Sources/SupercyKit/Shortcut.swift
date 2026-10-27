import Foundation

/// A keyboard shortcut stored as text, such as `shift+cmd+v`, with its Carbon key code and
/// modifier flags for registering a global hot key.
public struct Shortcut: Equatable {
    public enum Modifier: String, CaseIterable {
        case ctrl, opt, shift, cmd

        /// Carbon modifier flag (`controlKey`, `optionKey`, `shiftKey`, `cmdKey`).
        public var carbonFlag: UInt32 {
            switch self {
            case .cmd: return 1 << 8
            case .shift: return 1 << 9
            case .opt: return 1 << 11
            case .ctrl: return 1 << 12
            }
        }

        /// How macOS writes it.
        public var symbol: String {
            switch self {
            case .ctrl: return "⌃"
            case .opt: return "⌥"
            case .shift: return "⇧"
            case .cmd: return "⌘"
            }
        }
    }

    public let modifiers: Set<Modifier>
    /// The key: a lowercase letter, a digit, `space` or `f1`–`f12`.
    public let key: String

    public init?(modifiers: Set<Modifier>, key: String) {
        let key = key.lowercased()
        guard !modifiers.isEmpty || key.hasPrefix("f"), Self.keyCodes[key] != nil else { return nil }
        self.modifiers = modifiers
        self.key = key
    }

    /// Parses `shift+cmd+v`; `nil` for anything else.
    public init?(text: String) {
        var parts = text.lowercased().split(separator: "+").map(String.init)
        guard let key = parts.popLast() else { return nil }
        var modifiers = Set<Modifier>()
        for part in parts {
            guard let modifier = Modifier(rawValue: part), modifiers.insert(modifier).inserted else { return nil }
        }
        self.init(modifiers: modifiers, key: key)
    }

    /// The stored form, modifiers in a fixed order.
    public var text: String {
        (Modifier.allCases.filter(modifiers.contains).map(\.rawValue) + [key]).joined(separator: "+")
    }

    /// How macOS shows it, such as `⇧⌘V`.
    public var display: String {
        Modifier.allCases.filter(modifiers.contains).map(\.symbol).joined() + key.uppercased()
    }

    public var carbonModifiers: UInt32 { modifiers.reduce(0) { $0 | $1.carbonFlag } }

    public var carbonKeyCode: UInt32 { Self.keyCodes[key] ?? 0 }

    /// The key for a Carbon key code, if it is one Supercy supports.
    public static func key(forKeyCode code: UInt16) -> String? {
        keyCodes.first { $0.value == UInt32(code) }?.key
    }

    /// ANSI key codes (`kVK_ANSI_*`, `kVK_F*`, `kVK_Space`).
    static let keyCodes: [String: UInt32] = [
        "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11,
        "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17, "1": 18, "2": 19, "3": 20, "4": 21,
        "6": 22, "5": 23, "9": 25, "7": 26, "8": 28, "0": 29, "o": 31, "u": 32, "i": 34, "p": 35,
        "l": 37, "j": 38, "k": 40, "n": 45, "m": 46, "space": 49,
        "f1": 122, "f2": 120, "f3": 99, "f4": 118, "f5": 96, "f6": 97, "f7": 98, "f8": 100,
        "f9": 101, "f10": 109, "f11": 103, "f12": 111,
    ]
}
