import Foundation

/// Which image the menu bar shows for an icon style and state.
public enum MenuBarIconStyle {
    public enum Image: Equatable {
        /// An image from the asset catalog (template).
        case asset(String)
        /// A system symbol.
        case symbol(String)
    }

    public static func image(for style: MenuIcon, paused: Bool) -> Image {
        switch style {
        case .clipboard: return .asset(paused ? "MenuBarIconPaused" : "MenuBarIcon")
        case .paperclip: return .symbol(paused ? "pause.circle" : "paperclip")
        case .scissors: return .symbol(paused ? "pause.circle" : "scissors")
        case .stack: return .symbol(paused ? "pause.circle" : "square.stack")
        }
    }
}
