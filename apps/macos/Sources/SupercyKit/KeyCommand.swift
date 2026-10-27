import AppKit

/// What a key press in the history panel does. The mapping is a pure function so it is
/// tested without a window.
public enum KeyCommand: Equatable {
    case moveUp, moveDown, close, togglePin, delete, clearHistory, clearAll, openSettings
    /// ⌥⌘↑ / ⌥⌘↓: move the selected pinned item among the pinned items.
    case movePinUp, movePinDown
    /// Choose the selected item; `plainText` and `copyOnly` from the modifiers.
    case choose(plainText: Bool?, copyOnly: Bool)
    /// ⌘1–⌘9.
    case selectNumber(Int)
    /// ⌃ + the letter of a pinned item.
    case pinKey(String)

    private enum Code {
        static let returnKey: UInt16 = 36, enter: UInt16 = 76, escape: UInt16 = 53
        static let up: UInt16 = 126, down: UInt16 = 125, delete: UInt16 = 51
        static let p: UInt16 = 35, comma: UInt16 = 43
        static let digits: [UInt16: Int] = [18: 1, 19: 2, 20: 3, 21: 4, 23: 5, 22: 6, 26: 7, 28: 8, 25: 9]
    }

    /// The command for a key press, or `nil` to let the search field have it.
    public static func from(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> KeyCommand? {
        let flags = modifiers.intersection([.command, .option, .shift, .control])
        switch keyCode {
        case Code.up where flags == [.option, .command]: return .movePinUp
        case Code.down where flags == [.option, .command]: return .movePinDown
        case Code.up: return .moveUp
        case Code.down: return .moveDown
        case Code.escape: return .close
        case Code.returnKey, Code.enter: return chooseCommand(flags)
        case Code.delete: return deleteCommand(flags)
        case Code.p where flags == .option: return .togglePin
        case Code.comma where flags == .command: return .openSettings
        default: break
        }
        if flags == .command, let number = Code.digits[keyCode] { return .selectNumber(number) }
        if flags == .control, let key = Shortcut.key(forKeyCode: keyCode), key.count == 1, key.first?.isLetter == true {
            return .pinKey(key)
        }
        return nil
    }

    private static func chooseCommand(_ flags: NSEvent.ModifierFlags) -> KeyCommand? {
        switch flags {
        case []: return .choose(plainText: nil, copyOnly: false)
        case .option: return .choose(plainText: nil, copyOnly: true)
        case [.option, .shift]: return .choose(plainText: true, copyOnly: false)
        default: return nil
        }
    }

    private static func deleteCommand(_ flags: NSEvent.ModifierFlags) -> KeyCommand? {
        switch flags {
        case .option: return .delete
        case [.option, .command]: return .clearHistory
        case [.option, .command, .shift]: return .clearAll
        default: return nil
        }
    }
}

extension HistoryViewModel {
    /// Runs `command`; returns whether it was handled.
    @discardableResult
    public func perform(_ command: KeyCommand, nowMs: UInt64) -> Bool {
        switch command {
        case .moveUp: moveSelection(by: -1)
        case .moveDown: moveSelection(by: 1)
        case .close: onClose?()
        case .togglePin: togglePinSelected(nowMs: nowMs)
        case .movePinUp: moveSelectedPin(by: -1, nowMs: nowMs)
        case .movePinDown: moveSelectedPin(by: 1, nowMs: nowMs)
        case .delete: deleteSelected()
        case .clearHistory: clear(keepPinned: true)
        case .clearAll: clear(keepPinned: false)
        case .openSettings: onOpenSettings?()
        case let .choose(plainText, copyOnly): chooseSelected(plainText: plainText, copyOnly: copyOnly)
        case let .selectNumber(number):
            guard let item = select(number: number) else { return false }
            choose(item)
        case let .pinKey(key):
            guard let item = select(pinKey: key) else { return false }
            choose(item)
        }
        return true
    }
}
