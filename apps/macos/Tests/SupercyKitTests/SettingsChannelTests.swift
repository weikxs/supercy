import XCTest

final class SettingsChannelTests: XCTestCase {
    private func changed(_ base: Settings) -> Settings {
        var s = base
        s.maxItems = 200; s.maxImageTotalBytes = 2 * 1024 * 1024; s.detectSensitive = false
        s.ignoredApps = ["com.example.notes"]; s.allowListMode = true; s.allowedApps = ["com.example.editor"]
        s.ignoredTypes = ["com.example.private-type"]; s.ignorePatterns = ["^otp"]; s.recordImages = false
        s.searchMode = .mixed; s.sort = .copyCount; s.pinPosition = .bottom; s.pasteAutomatically = false
        s.popupPosition = .screenCenter; s.historyShortcut = "ctrl+opt+v"; s.pollIntervalMs = 700
        s.showSpecialChars = true; s.menuIcon = .scissors; s.showRecentInMenuBar = true; s.clearOnQuit = true
        s.visibleItems = 30; s.imagePreviewHeight = 64; s.language = .simplifiedChinese
        return s
    }

    func testEveryFieldSurvivesTheJSONForm() throws {
        let settings = changed(try makeEngine(self).settings())

        XCTAssertEqual(SettingsCoding.decode(SettingsCoding.encode(settings)), settings)
    }

    func testAMissingOrMistypedFieldIsRefused() throws {
        var object = SettingsCoding.encode(try makeEngine(self).settings())
        object["sort"] = "sideways"
        XCTAssertNil(SettingsCoding.decode(object))
        object.removeValue(forKey: "sort")
        XCTAssertNil(SettingsCoding.decode(object))
    }

    func testTheSettingsProcessSavesThroughTheMainProcess() throws {
        let engine = try makeEngine(self)
        let host = EngineSettingsStore(engine: engine)
        var savedInMain: [Settings] = []
        var pending: String?
        let open = try XCTUnwrap(SettingsChannel.parseOpen(SettingsChannel.openMessage(try engine.settings(), language: .chinese)))
        let remote = RemoteSettingsStore(
            initial: open.settings,
            send: { line in pending = SettingsChannel.handle(line, store: host, saved: { savedInMain.append($0) }) },
            receive: { pending }
        )
        let model = try SettingsModel(store: remote, system: FakeSystem())

        model.update { $0.maxItems = 300 }
        model.update { $0.maxItems = 1 }

        XCTAssertEqual(open.language, .chinese)
        XCTAssertEqual(try engine.settings().maxItems, 300)
        XCTAssertEqual(model.settings.maxItems, 300)
        XCTAssertEqual(savedInMain.map(\.maxItems), [300])
        XCTAssertNotNil(model.error, "the refused value comes back as an error from the main process")
    }

    func testABrokenOrMissingAnswerIsAnError() throws {
        let settings = try makeEngine(self).settings()
        let gone = RemoteSettingsStore(initial: settings, send: { _ in }, receive: { nil })

        XCTAssertThrowsError(try gone.save(settings))
        XCTAssertEqual(SettingsChannel.parseReply("not json"), .failure(SettingsChannelError(message: "no answer from Supercy")))
        XCTAssertNil(SettingsChannel.parseOpen("{\"type\":\"save\"}"))
        XCTAssertTrue(SettingsChannel.handle("{}", store: EngineSettingsStore(engine: try makeEngine(self)), saved: { _ in })
            .contains("error"))
    }
}
