import Foundation

/// Simulates ⌘V in the front app. Needs the Accessibility permission.
public protocol Pasting: AnyObject {
    /// Whether Supercy may send key events.
    var isTrusted: Bool { get }
    /// Asks the system for the permission (shows the system prompt).
    func requestTrust()
    /// Sends ⌘V to the front app.
    func paste()
}

/// What happened when the user chose an item.
public enum ChooseResult: Equatable {
    /// Copied and pasted into the front app.
    case pasted
    /// Copied only (the setting says so).
    case copied
    /// Copied, but pasting needs the Accessibility permission.
    case needsAccessibility
    /// The item could not be read.
    case failed(String)
}
