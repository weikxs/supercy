import AppKit
import SwiftUI

/// The settings window: General, Storage, Ignore, Appearance, Permissions, About.
public struct SettingsView: View {
    @ObservedObject var model: SettingsModel

    public init(model: SettingsModel) {
        self.model = model
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            TabView {
                GeneralTab(model: model).tabItem { Text(L10n.text(.tabGeneral)) }
                StorageTab(model: model).tabItem { Text(L10n.text(.tabStorage)) }
                IgnoreTab(model: model).tabItem { Text(L10n.text(.tabIgnore)) }
                AppearanceTab(model: model).tabItem { Text(L10n.text(.tabAppearance)) }
                PermissionsTab(model: model).tabItem { Text(L10n.text(.tabPermissions)) }
                AboutTab().tabItem { Text(L10n.text(.tabAbout)) }
            }
            if let error = model.error {
                Text(error).foregroundStyle(.red).font(.caption).padding(8)
            }
        }
        .padding(12)
        .frame(width: 560, height: 520)
    }
}

/// A binding that saves through the model on every change.
private func binding<Value>(_ model: SettingsModel, _ keyPath: WritableKeyPath<Settings, Value>) -> Binding<Value> {
    Binding(get: { model.settings[keyPath: keyPath] }, set: { value in model.update { $0[keyPath: keyPath] = value } })
}

private struct GeneralTab: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            Toggle(L10n.text(.launchAtLogin), isOn: $model.launchAtLogin)
            ShortcutField(title: L10n.text(.historyShortcut), text: binding(model, \.historyShortcut))
            Toggle(L10n.text(.pasteAutomatically), isOn: binding(model, \.pasteAutomatically))
            Toggle(L10n.text(.pastePlainByDefault), isOn: binding(model, \.pastePlainByDefault))
            Picker(L10n.text(.popupPosition), selection: binding(model, \.popupPosition)) {
                Text(L10n.text(.popupCursor)).tag(PopupPosition.cursor)
                Text(L10n.text(.popupMenuIcon)).tag(PopupPosition.menuIcon)
                Text(L10n.text(.popupCenter)).tag(PopupPosition.screenCenter)
                Text(L10n.text(.popupLast)).tag(PopupPosition.lastPosition)
            }
            Picker(L10n.text(.searchMode), selection: binding(model, \.searchMode)) {
                Text(L10n.text(.searchFuzzy)).tag(SearchMode.fuzzy)
                Text(L10n.text(.searchExact)).tag(SearchMode.exact)
                Text(L10n.text(.searchRegex)).tag(SearchMode.regex)
                Text(L10n.text(.searchMixed)).tag(SearchMode.mixed)
            }
            Picker(L10n.text(.sortOrder), selection: binding(model, \.sort)) {
                Text(L10n.text(.sortLastUsed)).tag(SortOrder.lastUsed)
                Text(L10n.text(.sortFirstCopied)).tag(SortOrder.firstCopied)
                Text(L10n.text(.sortCopyCount)).tag(SortOrder.copyCount)
            }
            Picker(L10n.text(.pinPosition), selection: binding(model, \.pinPosition)) {
                Text(L10n.text(.pinTop)).tag(PinPosition.top)
                Text(L10n.text(.pinBottom)).tag(PinPosition.bottom)
            }
            Picker(L10n.text(.language), selection: binding(model, \.language)) {
                Text(L10n.text(.languageSystem)).tag(Language.system)
                Text(L10n.text(.languageChinese)).tag(Language.simplifiedChinese)
                Text(L10n.text(.languageEnglish)).tag(Language.english)
            }
        }
    }
}

private struct StorageTab: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            Stepper(value: binding(model, \.maxItems), in: 50...10_000, step: 50) {
                Text("\(L10n.text(.maxItems)): \(model.settings.maxItems)")
            }
            Toggle(L10n.text(.recordText), isOn: binding(model, \.recordText))
            Toggle(L10n.text(.recordImages), isOn: binding(model, \.recordImages))
            Toggle(L10n.text(.recordFiles), isOn: binding(model, \.recordFiles))
            Toggle(L10n.text(.detectSensitive), isOn: binding(model, \.detectSensitive))
            Stepper(value: minutes, in: 1...1_440) {
                Text("\(model.settings.sensitiveTtlMs / 60_000) \(L10n.text(.sensitiveMinutes))")
            }
            .disabled(!model.settings.detectSensitive)
            Toggle(L10n.text(.clearOnQuit), isOn: binding(model, \.clearOnQuit))
            Stepper(value: binding(model, \.pollIntervalMs), in: 100...2_000, step: 100) {
                Text("\(L10n.text(.pollInterval)): \(model.settings.pollIntervalMs)")
            }
        }
    }

    private var minutes: Binding<UInt64> {
        Binding(
            get: { max(1, model.settings.sensitiveTtlMs / 60_000) },
            set: { value in model.update { $0.sensitiveTtlMs = value * 60_000 } }
        )
    }
}

private struct IgnoreTab: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                ListEditor(model: model, title: L10n.text(.ignoredApps), keyPath: \.ignoredApps, placeholder: "com.example.app")
                Toggle(L10n.text(.allowListMode), isOn: binding(model, \.allowListMode))
                if model.settings.allowListMode {
                    ListEditor(model: model, title: L10n.text(.allowedApps), keyPath: \.allowedApps, placeholder: "com.example.app")
                }
                ListEditor(model: model, title: L10n.text(.ignoredTypes), keyPath: \.ignoredTypes, placeholder: "com.example.type")
                ListEditor(
                    model: model,
                    title: L10n.text(.ignorePatterns),
                    keyPath: \.ignorePatterns,
                    placeholder: "^otp:",
                    validate: model.patternProblem
                )
            }
            .padding(4)
        }
    }
}

/// An editable list of strings with add and remove.
private struct ListEditor: View {
    @ObservedObject var model: SettingsModel
    let title: String
    let keyPath: WritableKeyPath<Settings, [String]>
    let placeholder: String
    var validate: ((String) -> String?)?
    @State private var draft = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.headline)
            ForEach(model.settings[keyPath: keyPath], id: \.self) { entry in
                HStack {
                    Text(entry).font(.body.monospaced())
                    Spacer()
                    Button(L10n.text(.remove)) { model.remove(entry, from: keyPath) }
                }
            }
            HStack {
                TextField(placeholder, text: $draft).textFieldStyle(.roundedBorder)
                Button(L10n.text(.add)) {
                    model.add(draft, to: keyPath)
                    draft = ""
                }
                .disabled(draft.isEmpty || problem != nil)
            }
            if problem != nil {
                Text(L10n.text(.invalidPattern)).font(.caption).foregroundStyle(.red)
            }
        }
    }

    private var problem: String? {
        draft.isEmpty ? nil : validate?(draft)
    }
}

private struct AppearanceTab: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            Toggle(L10n.text(.showMenuIcon), isOn: binding(model, \.showMenuIcon))
            Toggle(L10n.text(.showRecentInMenuBar), isOn: binding(model, \.showRecentInMenuBar))
            Toggle(L10n.text(.showSpecialChars), isOn: binding(model, \.showSpecialChars))
            Stepper(value: binding(model, \.visibleItems), in: 5...100) {
                Text("\(L10n.text(.visibleItems)): \(model.settings.visibleItems)")
            }
            Stepper(value: binding(model, \.imagePreviewHeight), in: 16...400, step: 8) {
                Text("\(L10n.text(.imagePreviewHeight)): \(model.settings.imagePreviewHeight)")
            }
        }
    }
}

private struct PermissionsTab: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(L10n.text(.accessibility)).font(.headline)
                Spacer()
                Text(L10n.text(model.isAccessibilityTrusted ? .accessibilityGranted : .accessibilityMissing))
                    .foregroundStyle(model.isAccessibilityTrusted ? .green : .orange)
            }
            Text(L10n.text(.accessibilityExplanation)).font(.callout).fixedSize(horizontal: false, vertical: true)
            Button(L10n.text(.openSystemSettings)) { model.openAccessibilitySettings() }
            Spacer()
        }
        .padding(8)
    }
}

private struct AboutTab: View {
    var body: some View {
        VStack(spacing: 8) {
            Text(L10n.text(.appName)).font(.title)
            Text("\(L10n.text(.version)) \(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "–")")
            Text("\(L10n.text(.coreVersion)) \(coreVersion())").foregroundStyle(.secondary)
            Text(L10n.text(.privacyNote)).font(.callout).multilineTextAlignment(.center).padding(.top, 8)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Shows a shortcut and records a new one from the next key press.
struct ShortcutField: View {
    let title: String
    @Binding var text: String
    @State private var recording = false
    @State private var monitor: Any?

    var body: some View {
        HStack {
            Text(title)
            Spacer()
            Text(recording ? L10n.text(.pressShortcut) : (Shortcut(text: text)?.display ?? text))
                .font(.body.monospaced())
            Button(L10n.text(.recordShortcut)) { recording ? stop() : start() }
        }
        .onDisappear(perform: stop)
    }

    private func start() {
        recording = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if let shortcut = Self.shortcut(from: event) {
                text = shortcut.text
                stop()
                return nil
            }
            return event
        }
    }

    private func stop() {
        recording = false
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }

    static func shortcut(from event: NSEvent) -> Shortcut? {
        guard let key = Shortcut.key(forKeyCode: event.keyCode) else { return nil }
        let flags = event.modifierFlags
        var modifiers = Set<Shortcut.Modifier>()
        if flags.contains(.control) { modifiers.insert(.ctrl) }
        if flags.contains(.option) { modifiers.insert(.opt) }
        if flags.contains(.shift) { modifiers.insert(.shift) }
        if flags.contains(.command) { modifiers.insert(.cmd) }
        return Shortcut(modifiers: modifiers, key: key)
    }
}
