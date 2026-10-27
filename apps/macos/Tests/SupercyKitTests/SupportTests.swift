import XCTest

final class ShortcutTests: XCTestCase {
    func testParsesShowsAndConvertsToCarbon() throws {
        let history = try XCTUnwrap(Shortcut(text: "shift+cmd+v"))
        let capture = try XCTUnwrap(Shortcut(text: "cmd+shift+2"))

        XCTAssertEqual(history.display, "⇧⌘V")
        XCTAssertEqual(history.carbonKeyCode, 9)
        XCTAssertEqual(history.carbonModifiers, (1 << 8) | (1 << 9))
        XCTAssertEqual(capture.text, "shift+cmd+2")
        XCTAssertEqual(Shortcut(text: "f5")?.display, "F5")
    }

    func testRefusesBareKeysUnknownKeysAndRepeatedModifiers() {
        XCTAssertNil(Shortcut(text: "v"))
        XCTAssertNil(Shortcut(text: "cmd+§"))
        XCTAssertNil(Shortcut(text: "cmd+cmd+v"))
        XCTAssertNil(Shortcut(text: "hyper+v"))
        XCTAssertNil(Shortcut(text: ""))
    }
}

final class L10nTests: XCTestCase {
    func testEveryTextExistsInBothLanguages() {
        for key in L10nKey.allCases {
            XCTAssertFalse(L10n.english[key, default: ""].isEmpty, "English text missing for \(key)")
            XCTAssertFalse(L10n.chinese[key, default: ""].isEmpty, "Chinese text missing for \(key)")
        }
    }

    func testLanguageFollowsTheSettingOrTheSystem() {
        XCTAssertEqual(UILanguage.resolve(.system, preferred: ["zh-Hans-CN"]), .chinese)
        XCTAssertEqual(UILanguage.resolve(.system, preferred: ["de-DE"]), .english)
        XCTAssertEqual(UILanguage.resolve(.simplifiedChinese, preferred: ["en"]), .chinese)
        XCTAssertEqual(UILanguage.resolve(.english, preferred: ["zh-Hans"]), .english)
        XCTAssertEqual(L10n.text(.paste, .chinese), "粘贴")
    }
}

final class LaunchOptionsTests: XCTestCase {
    private let standard = URL(fileURLWithPath: "/var/empty/Supercy")

    private func parse(_ arguments: [String], _ environment: [String: String]) -> Result<LaunchOptions, LaunchOptions.Problem> {
        LaunchOptions.parse(arguments: ["Supercy"] + arguments, environment: environment, defaultDataDirectory: standard)
    }

    func testNormalLaunchUsesTheStandardDirectoryAndEverything() throws {
        let options = try parse([], [:]).get()

        XCTAssertEqual(options, LaunchOptions(mode: .normal, dataDirectory: standard))
        XCTAssertTrue(options.mode.usesLoginKeychain && options.mode.allowsLoginItem)
    }

    func testIsolatedLaunchNeedsADataDirectoryUnderTmpAndNeverReadsThePasteboard() throws {
        let ok = try parse(["--isolated"], ["SUPERCY_DATA_DIR": "/tmp/supercy/data"]).get()

        XCTAssertEqual(ok.mode, .isolated)
        XCTAssertFalse(ok.mode.readsSystemPasteboard || ok.mode.usesLoginKeychain || ok.mode.registersGlobalShortcut)
        XCTAssertEqual(parse(["--isolated"], [:]), .failure(.needsTemporaryDataDirectory))
        XCTAssertEqual(parse(["--isolated"], ["SUPERCY_DATA_DIR": "/var/data"]), .failure(.needsTemporaryDataDirectory))
        XCTAssertEqual(parse(["--isolated"], ["SUPERCY_DATA_DIR": "/tmp/../var"]), .failure(.needsTemporaryDataDirectory))
    }

    func testTrialUsesThePasteboardButNeverTheKeychainOrLoginItems() throws {
        let environment = ["SUPERCY_DATA_DIR": "/tmp/supercy/data", "HOME": "/tmp/supercy/home",
                           "CFFIXED_USER_HOME": "/tmp/supercy/home"]
        let trial = try parse(["--trial"], environment).get()

        XCTAssertEqual(trial, LaunchOptions(mode: .trial, dataDirectory: URL(fileURLWithPath: "/tmp/supercy/data", isDirectory: true)))
        XCTAssertTrue(trial.mode.readsSystemPasteboard && trial.mode.registersGlobalShortcut)
        XCTAssertFalse(trial.mode.usesLoginKeychain || trial.mode.allowsLoginItem)
    }

    func testTrialRefusesDataOrHomeOutsideTmpAndBothModesAtOnce() {
        let data = ["SUPERCY_DATA_DIR": "/tmp/supercy/data"]

        XCTAssertEqual(parse(["--trial"], ["HOME": "/tmp/supercy/home"]), .failure(.needsTemporaryDataDirectory))
        XCTAssertEqual(parse(["--trial"], data), .failure(.trialNeedsTemporaryHome))
        XCTAssertEqual(parse(["--trial"], data.merging(["HOME": "/var/empty"]) { $1 }), .failure(.trialNeedsTemporaryHome))
        XCTAssertEqual(parse(["--trial"], data.merging(["HOME": "/tmp/supercy/home", "CFFIXED_USER_HOME": "/var/empty"]) { $1 }),
                       .failure(.trialNeedsTemporaryHome))
        XCTAssertEqual(parse(["--trial", "--isolated"], data), .failure(.conflictingModes))
    }

    func testTheTrialNeverRegistersALoginItem() {
        let base = FakeSystem()
        base.isAccessibilityTrusted = true
        let trial = TrialSystemIntegration(base: base)

        trial.launchAtLogin = true

        XCTAssertFalse(trial.launchAtLogin)
        XCTAssertFalse(base.launchAtLogin)
        XCTAssertTrue(trial.isAccessibilityTrusted)
    }
}

final class PreviewFormatterTests: XCTestCase {
    func testTitlesAreOneLineAndSpecialCharactersCanBeShown() {
        XCTAssertEqual(PreviewFormatter.title("line one\nline\ttwo  ", showSpecialChars: false), "line one line two")
        XCTAssertEqual(PreviewFormatter.title("a b\tc\n", showSpecialChars: true), "a·b⇥c↩")
        XCTAssertEqual(PreviewFormatter.title(String(repeating: "x", count: 200), showSpecialChars: false).count, 121)
    }

    func testHighlightsMarkTheMatchedRanges() {
        let attributed = PreviewFormatter.highlighted("Hello from Supercy", matches: [TextRange(location: 6, length: 4)])
        let marked = attributed.runs.filter { $0.inlinePresentationIntent == .stronglyEmphasized }
            .map { String(attributed[$0.range].characters) }

        XCTAssertEqual(marked, ["from"])
    }
}

final class SettingsModelTests: XCTestCase {
    func testValidChangesAreSavedAndInvalidOnesReportedAndUndone() throws {
        let engine = try makeEngine(self)
        let model = try SettingsModel(engine: engine, system: FakeSystem())
        var saved = 0
        model.onSaved = { _ in saved += 1 }

        model.update { $0.maxItems = 200 }
        model.update { $0.maxItems = 1 }

        XCTAssertEqual(try engine.settings().maxItems, 200)
        XCTAssertEqual(model.settings.maxItems, 200)
        XCTAssertNotNil(model.error)
        XCTAssertEqual(saved, 1)
    }

    func testListsAddAndRemoveAndPatternsAreChecked() throws {
        let engine = try makeEngine(self)
        let model = try SettingsModel(engine: engine, system: FakeSystem())

        model.add("  com.example.notes ", to: \.ignoredApps)
        model.add("com.example.notes", to: \.ignoredApps)
        model.add("^otp", to: \.ignorePatterns)
        model.remove("com.1password.1password", from: \.ignoredApps)

        let settings = try engine.settings()
        XCTAssertEqual(settings.ignoredApps.filter { $0 == "com.example.notes" }.count, 1)
        XCTAssertFalse(settings.ignoredApps.contains("com.1password.1password"))
        XCTAssertEqual(settings.ignorePatterns, ["^otp"])
        XCTAssertNotNil(model.patternProblem("("))
        XCTAssertNil(model.patternProblem("^otp"))
    }

    func testLaunchAtLoginGoesToTheSystem() throws {
        let system = FakeSystem()
        let model = try SettingsModel(engine: try makeEngine(self), system: system)

        model.launchAtLogin = true

        XCTAssertTrue(system.launchAtLogin)
    }
}
