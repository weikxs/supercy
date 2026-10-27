import AppKit
import ApplicationServices
import Carbon.HIToolbox
import ServiceManagement

/// The user's real pasteboard. Only the app target can reach it; tests cannot.
final class SystemPasteboard: PasteboardReading, PasteboardWriting {
    private let pasteboard = NSPasteboard.general

    var changeCount: Int { pasteboard.changeCount }

    func readItems() -> [[PasteboardEntry]] {
        (pasteboard.pasteboardItems ?? []).map { item in
            item.types.map { PasteboardEntry(type: $0.rawValue, data: item.data(forType: $0)) }
        }
    }

    func write(items: [[PasteboardEntry]]) {
        pasteboard.clearContents()
        let pasteboardItems = items.map { entries -> NSPasteboardItem in
            let item = NSPasteboardItem()
            for entry in entries {
                if let data = entry.data { item.setData(data, forType: NSPasteboard.PasteboardType(entry.type)) }
            }
            return item
        }
        pasteboard.writeObjects(pasteboardItems)
    }
}

/// The front application from the workspace.
final class WorkspaceFrontApp: FrontAppProviding {
    func frontApp() -> SourceApp? {
        guard let app = NSWorkspace.shared.frontmostApplication, let id = app.bundleIdentifier else { return nil }
        return SourceApp(bundleId: id, name: app.localizedName ?? id)
    }
}

/// Sends ⌘V with Core Graphics events. Needs the Accessibility permission, which is asked for
/// only when the user first wants Supercy to paste.
final class AccessibilityPaster: Pasting {
    var isTrusted: Bool { AXIsProcessTrusted() }

    func requestTrust() {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
    }

    func paste() {
        // Give the panel time to close so the front app has the keyboard again.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) {
            let source = CGEventSource(stateID: .combinedSessionState)
            let vKey = CGKeyCode(kVK_ANSI_V)
            let down = CGEvent(keyboardEventSource: source, virtualKey: vKey, keyDown: true)
            let up = CGEvent(keyboardEventSource: source, virtualKey: vKey, keyDown: false)
            down?.flags = .maskCommand
            up?.flags = .maskCommand
            down?.post(tap: .cgSessionEventTap)
            up?.post(tap: .cgSessionEventTap)
        }
    }
}

/// The data key in the login keychain.
final class KeychainKeystore: Keystore, @unchecked Sendable {
    private let service = "io.github.weikxs.Supercy"

    private func query(_ account: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account]
    }

    func read(account: String) throws -> Data? {
        var request = query(account)
        request[kSecReturnData as String] = true
        request[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(request as CFDictionary, &result)
        switch status {
        case errSecSuccess: return result as? Data
        case errSecItemNotFound: return nil
        case errSecAuthFailed, errSecUserCanceled, errSecInteractionNotAllowed: throw KeystoreError.Denied
        default: throw KeystoreError.Unavailable(message: "keychain status \(status)")
        }
    }

    func write(account: String, secret: Data) throws {
        let update = SecItemUpdate(query(account) as CFDictionary, [kSecValueData as String: secret] as CFDictionary)
        if update == errSecSuccess { return }
        var add = query(account)
        add[kSecValueData as String] = secret
        add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let status = SecItemAdd(add as CFDictionary, nil)
        guard status == errSecSuccess else { throw KeystoreError.Unavailable(message: "keychain status \(status)") }
    }

    func delete(account: String) throws {
        let status = SecItemDelete(query(account) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KeystoreError.Unavailable(message: "keychain status \(status)")
        }
    }
}

/// Login item and Accessibility status for the settings screen.
final class MacSystemIntegration: SystemIntegration {
    var launchAtLogin: Bool {
        get { SMAppService.mainApp.status == .enabled }
        set {
            do {
                if newValue { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
            } catch {
                NSLog("Supercy: login item change failed: %@", error.localizedDescription)
            }
        }
    }

    var isAccessibilityTrusted: Bool { AXIsProcessTrusted() }

    func openAccessibilitySettings() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
            NSWorkspace.shared.open(url)
        }
    }
}

/// A global shortcut through Carbon's hot keys (no Accessibility permission needed).
final class HotKey {
    private var reference: EventHotKeyRef?
    private var handler: EventHandlerRef?
    private let action: () -> Void
    private static let signature: OSType = 0x5343_5059 // "SCPY"

    init(action: @escaping () -> Void) {
        self.action = action
    }

    deinit { unregister() }

    /// Registers `shortcut`, replacing any previous one; returns whether it worked.
    @discardableResult
    func register(_ shortcut: Shortcut) -> Bool {
        unregister()
        var type = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        let context = Unmanaged.passUnretained(self).toOpaque()
        InstallEventHandler(GetApplicationEventTarget(), { _, _, context in
            guard let context else { return noErr }
            Unmanaged<HotKey>.fromOpaque(context).takeUnretainedValue().action()
            return noErr
        }, 1, &type, context, &handler)
        let id = EventHotKeyID(signature: Self.signature, id: 1)
        let status = RegisterEventHotKey(shortcut.carbonKeyCode, shortcut.carbonModifiers, id,
                                         GetApplicationEventTarget(), 0, &reference)
        return status == noErr
    }

    func unregister() {
        if let reference { UnregisterEventHotKey(reference) }
        if let handler { RemoveEventHandler(handler) }
        reference = nil
        handler = nil
    }
}
