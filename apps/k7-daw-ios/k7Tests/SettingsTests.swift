// SettingsTests.swift — the native Settings: which address it stands in for, its rules beside the web's, the client's
// settings routes against a stubbed backend, the language and theme read from and written to the preferences.

import XCTest
@testable import k7

private func stubbed() -> K7Client {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [StubProtocol.self]
    return K7Client(origin: URL(string: "http://127.0.0.1:1")!, session: URLSession(configuration: config))
}

private func body(_ r: URLRequest) -> [String: Any] {
    var data = r.httpBody ?? Data()
    if data.isEmpty, let stream = r.httpBodyStream {
        stream.open()
        defer { stream.close() }
        var buffer = [UInt8](repeating: 0, count: 4096)
        while stream.hasBytesAvailable {
            let n = stream.read(&buffer, maxLength: buffer.count)
            if n <= 0 { break }
            data.append(buffer, count: n)
        }
    }
    return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
}

/// The session and the hub's hello, then `rest` for every other path.
private func backend(_ rest: @escaping (URLRequest) -> (Int, String)) -> (URLRequest) -> (Int, String) {
    { r in
        switch r.url!.path {
        case "/auth/me": return (200, #"{"user":{"email":"dev@example.com"}}"#)
        case "/api/files-hub": return (200, #"{"path":"/f","ticket":"T"}"#)
        default: return rest(r)
        }
    }
}

final class SettingsRulesTests: XCTestCase {
    private let origin = URL(string: "http://127.0.0.1:18789")!

    private func page(_ s: String) -> HomeRules.NativePage? {
        HomeRules.nativePage(URL(string: "http://127.0.0.1:18789" + s)!, origin: origin)
    }

    func testSettingsAddressesAreNativeUnlessAWebTabOrWebAsked() {
        XCTAssertEqual(page("/"), .projects)
        XCTAssertEqual(page("/settings"), .settings([]))
        XCTAssertEqual(page("/settings?tab=general"), .settings([.tab(.general)]))
        XCTAssertEqual(page("/settings?tab=nope"), .settings([]))
        XCTAssertEqual(page("/settings/engine"), .settings([.tab(.devices), .engine]))
        XCTAssertNil(page("/settings?tab=services"))
        XCTAssertNil(page("/settings?tab=downloads"))
        XCTAssertNil(page("/settings?tab=devices&web"))
        XCTAssertNil(page(SettingsTab.about.webRoute))
        XCTAssertNil(page("/libraries"))
        XCTAssertNil(HomeRules.nativePage(URL(string: "http://127.0.0.1:9/settings")!, origin: origin))
    }

    func testTheEngineLineReadsAsTheWebPhoneSection() {
        let l = L10n(bundle: Bundle(for: HomeModel.self), locale: "en")
        XCTAssertEqual(SettingsRules.engineLine(state: nil, exit: nil, lan: nil, l: l), l.t("android.starting"))
        XCTAssertEqual(SettingsRules.engineLine(state: ["status": "running", "enginePort": 9001], exit: nil, lan: "10.0.0.2", l: l),
                       "Engine running on ws://10.0.0.2:9001")
        XCTAssertEqual(SettingsRules.engineLine(state: ["status": "running"], exit: 3, lan: nil, l: l), "Engine stopped (3)")
        XCTAssertEqual(SettingsRules.engineLine(state: ["status": "error", "error": "no audio"], exit: nil, lan: nil, l: l),
                       "Engine: no audio")
    }

    func testInstrumentsTakeTheirDownloadsPhaseAndOneAction() {
        let state: [String: Any] = [
            "instruments": [
                ["name": "piano", "label": "Piano", "bytes": 748_000_000, "wanted": true, "installed": false],
                ["name": "drums", "label": "Drums", "bytes": 1_300_000_000, "wanted": false, "installed": true],
                ["name": "harp", "label": "Harp", "bytes": 5, "wanted": false, "installed": false],
            ],
            "downloads": [["name": "piano", "received": 10, "total": 748_000_000, "phase": "queued"]],
        ]
        let rows = SettingsRules.instruments(state)
        XCTAssertEqual(rows.map(\.phase), ["queued", nil, nil])
        XCTAssertEqual(SettingsRules.action(rows[0])?.cmd, "instruments.remove")
        XCTAssertEqual(SettingsRules.action(rows[1])?.key, "engine.instrument.remove")
        XCTAssertEqual(SettingsRules.action(rows[2])?.cmd, "instruments.download")
        let l = L10n(bundle: Bundle(for: HomeModel.self), locale: "en")
        XCTAssertEqual(SettingsRules.instrumentLine(rows[1], l: l), "installed · 1.3 GB")
        XCTAssertEqual(SettingsRules.instrumentLine(rows[2], l: l), "1 MB")
        XCTAssertEqual(SettingsRules.sizeOf(748_000_000), "748 MB")
    }

    func testAModelDownloadLineAndAnAuthorsSource() {
        let l = L10n(bundle: Bundle(for: HomeModel.self), locale: "en")
        let mb = 1024.0 * 1024.0
        let half = ModelJob(id: "1", file: "m.onnx", received: 50 * mb, total: 100 * mb, done: false, error: nil)
        XCTAssertEqual(SettingsRules.download(half, l: l), "downloading m.onnx — 50% (50 of 100 MB)")
        XCTAssertEqual(SettingsRules.typed("Ada ", shown: "Ada", source: "github"), "")
        XCTAssertEqual(SettingsRules.typed("Ada", shown: "Ada", source: nil), "Ada")
        XCTAssertEqual(SettingsRules.sourceKey("system"), "git.identityFromComputer")
        XCTAssertEqual(SettingsRules.options([], held: "@lib/gone").map(\.ref), ["@lib/gone"])
    }
}

final class K7ClientSettingsTests: XCTestCase {
    override func setUp() { StubProtocol.asked = [] }

    func testAPreferencePatchKeepsEveryOtherKey() async throws {
        var written: [String: Any] = [:]
        StubProtocol.answer = backend { r in
            if r.httpMethod == "PUT" {
                written = body(r)
                return (200, #"{"ok":true}"#)
            }
            return (200, #"{"engineId":"e1","commands":{"x":{"icon":"Scissors","order":2}},"locale":"en"}"#)
        }
        try await stubbed().patchPrefs(["locale": "fr"])
        XCTAssertEqual(written["locale"] as? String, "fr")
        XCTAssertEqual(written["engineId"] as? String, "e1")
        XCTAssertEqual(((written["commands"] as? [String: Any])?["x"] as? [String: Any])?["icon"] as? String, "Scissors")
        XCTAssertEqual(StubProtocol.asked.first?.url?.path, "/auth/me")
    }

    func testTheAuthorIsTheFileServicesAndTheChainsTheEnginesRoute() async throws {
        StubProtocol.answer = backend { r in
            switch r.url!.path {
            case "/f/git/identity": return (200, #"{"name":"Ada","email":"","emailSource":null}"#)
            case "/api/engines/ios phone/chains": return (200, #"{"chains":{"stems":"@k7/stems"}}"#)
            default: return (200, #"{"ok":true}"#)
            }
        }
        let client = stubbed()
        let author = try await client.identity()
        XCTAssertEqual(author.name, "Ada")
        try await client.saveIdentity(name: "Ada", email: "a@b.c")
        XCTAssertEqual(StubProtocol.asked.last?.httpMethod, "POST")
        XCTAssertEqual(body(StubProtocol.asked.last!)["email"] as? String, "a@b.c")
        let held = try await client.engineChains("ios phone")
        XCTAssertEqual(held.stems, "@k7/stems")
        XCTAssertEqual(StubProtocol.asked.last?.url?.absoluteString, "http://127.0.0.1:1/api/engines/ios%20phone/chains")
    }

    func testRemovingTheAiKeySendsNull() async throws {
        StubProtocol.answer = backend { r in
            r.httpMethod == "PUT" && body(r)["apiKey"] is NSNull
                ? (200, #"{"model":null,"defaultModel":"claude-x","keyHint":null}"#)
                : (400, #"{"error":"bad"}"#)
        }
        let v = try await stubbed().saveAiSettings(["apiKey": NSNull()])
        XCTAssertNil(v.keyHint)
        XCTAssertEqual(v.defaultModel, "claude-x")
    }
}

@MainActor
final class PrefsTests: XCTestCase {
    override func setUp() { StubProtocol.asked = [] }

    func testAThemeIsReadAsOneOfThreeAndDarkOtherwise() {
        XCTAssertEqual(Theme.of("light"), .light)
        XCTAssertEqual(Theme.of("system"), .system)
        XCTAssertEqual(Theme.of("sepia"), .dark)
        XCTAssertEqual(Theme.of(nil), .dark)
        XCTAssertNil(AppPrefs.locale("de"))
    }

    func testLoadFollowsTheStoredLanguageAndTheme() async {
        StubProtocol.answer = backend { _ in (200, #"{"locale":"fr","theme":"light"}"#) }
        let prefs = AppPrefs()
        let l = L10n(bundle: Bundle(for: HomeModel.self), locale: "en")
        let named = await prefs.load(stubbed(), l10n: l)
        XCTAssertTrue(named)
        XCTAssertEqual(prefs.theme, .light)
        XCTAssertEqual(l.locale, "fr")
    }

    func testARefusedThemeIsTakenBack() async {
        StubProtocol.answer = backend { r in r.httpMethod == "PUT" ? (500, #"{"error":"disk"}"#) : (200, "{}") }
        let prefs = AppPrefs()
        var told: [Theme] = []
        prefs.onTheme = { told.append($0) }
        do {
            try await prefs.setTheme(.light, client: stubbed())
            XCTFail("a refusal must throw")
        } catch {
            XCTAssertEqual(prefs.theme, .dark)
            XCTAssertEqual(told, [.light, .dark])
        }
    }

    func testChoosingALanguageWritesItAndTellsTheWebPage() async throws {
        var written: [String: Any] = [:]
        StubProtocol.answer = backend { r in
            if r.httpMethod == "PUT" { written = body(r) }
            return (200, "{}")
        }
        let prefs = AppPrefs()
        var told = 0
        prefs.onWritten = { told += 1 }
        let l = L10n(bundle: Bundle(for: HomeModel.self), locale: "en")
        try await prefs.setLocale("fr", client: stubbed(), l10n: l)
        XCTAssertEqual(written["locale"] as? String, "fr")
        XCTAssertEqual(l.locale, "fr")
        XCTAssertEqual(told, 1)
    }
}

@MainActor
final class SettingsModelTests: XCTestCase {
    override func setUp() { StubProtocol.asked = [] }

    func testAnAuthorStillShowingItsSuggestionIsSentEmpty() async {
        var sent: [String: Any] = [:]
        StubProtocol.answer = backend { r in
            if r.httpMethod == "POST" { sent = body(r) }
            return (200, #"{"name":"Ada","email":"ada@x.org","emailSource":"github"}"#)
        }
        let model = SettingsModel(client: stubbed(), prefs: AppPrefs())
        await model.loadGeneral()
        await model.saveIdentity(name: "Ada L", email: "ada@x.org")
        XCTAssertEqual(sent["name"] as? String, "Ada L")
        XCTAssertEqual(sent["email"] as? String, "")
        XCTAssertEqual(model.identityNote?.error, false)
    }

    func testChainsAreNamedOnThisDevicesEngine() async {
        StubProtocol.answer = backend { r in
            switch r.url!.path {
            case "/f/account/cold-chains":
                return (200, #"{"chains":[{"ref":"@k7/stems","instrumental":true,"unavailable":null,"downloads":[]}],"stemsChain":null}"#)
            case "/api/engines/e1/chains":
                return r.httpMethod == "PUT" ? (200, #"{"chains":{"stems":"@k7/stems"}}"#) : (200, #"{"chains":{}}"#)
            default: return (404, "{}")
            }
        }
        let model = SettingsModel(client: stubbed(), prefs: AppPrefs())
        model.engineId = { "e1" }
        await model.loadChains()
        XCTAssertEqual(model.cold?.chains.map(\.ref), ["@k7/stems"])
        XCTAssertNil(model.held?.stems)
        await model.choose("stems", "@k7/stems")
        XCTAssertEqual(model.held?.stems, "@k7/stems")
    }

    func testTheEnginePageSendsItsCommandsToTheSupervisor() {
        var sent: [String] = []
        let engine = EngineModel(known: { (["status": "running", "engineId": "e1", "lanOpen": true], nil) },
                                 send: { sent.append("\($0) \($1)") })
        engine.read()
        XCTAssertEqual(engine.engineId, "e1")
        XCTAssertTrue(engine.lanOpen)
        engine.setLanOpen(false)
        engine.instrument("instruments.download", "piano")
        XCTAssertEqual(sent, ["lan.open 0", "instruments.download piano"])
    }

    func testTheSettingsTabComesToTheFrontAndAgainPopsItsPages() {
        let home = HomeModel(client: stubbed())
        var opened: [String] = []
        home.openWeb = { opened.append($0) }
        home.choose(.settings)
        XCTAssertEqual(home.tab, .settings)
        home.settingsPath = [.tab(.general)]
        home.choose(.settings)
        XCTAssertEqual(home.settingsPath, [])
        home.choose(.tasks)
        XCTAssertEqual(opened, [HomeRules.tasksRoute])
        XCTAssertEqual(home.tab, .settings)
    }
}
