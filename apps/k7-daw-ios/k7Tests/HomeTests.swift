// HomeTests.swift — the Projects home's rules and model: the list's order and filter, a new folder's name, a
// project's address, the git pill, which address the native home stands in for, and one read of the model.

import XCTest
@testable import k7

private func ws(_ name: String, _ paths: [String]) -> Workspace {
    Workspace(name: name, isSingleProject: false, projects: paths.map { Project(path: $0, app: "k7") })
}

final class HomeRulesTests: XCTestCase {
    func testExamplesLastAndPeoplesOrder() {
        let ordered = HomeRules.pageOrder([ws("Examples", []), ws("song 10", []), ws("Song 2", ["b/x10.k7", "b/x2.k7", "a.k7"])])
        XCTAssertEqual(ordered.map(\.name), ["Song 2", "song 10", "Examples"])
        XCTAssertEqual(ordered[0].projects.map(\.path), ["a.k7", "b/x2.k7", "b/x10.k7"])
    }

    func testFilterKeepsAWorkspaceByNameAndProjectsByPath() {
        let all = [ws("Winter", ["a.k7", "b.k7"]), ws("Summer", ["winter-song.k7", "c.k7"])]
        let hit = HomeRules.filter(all, " WINTER ")
        XCTAssertEqual(hit.map(\.name), ["Winter", "Summer"])
        XCTAssertEqual(hit[0].projects.count, 2)
        XCTAssertEqual(hit[1].projects.map(\.path), ["winter-song.k7"])
    }

    func testFolderNamesAsTheWebDialogsMakeThem() {
        XCTAssertEqual(HomeRules.projectFolder("  demo ").folder, "demo.k7")
        XCTAssertEqual(HomeRules.projectFolder("demo.K7").folder, "demo.k7")
        XCTAssertEqual(HomeRules.workspaceFolder("a/b").error, "name.noSlash")
        XCTAssertEqual(HomeRules.workspaceFolder(".hidden").error, "name.noDot")
        XCTAssertEqual(HomeRules.workspaceFolder("   ").error, "name.required")
    }

    func testProjectAddressesEncodeEachSegmentKeepingTheAt() {
        XCTAssertEqual(HomeRules.projectPath(workspace: "My songs", project: "live/a b.k7"), "/@local/My%20songs/live/a%20b.k7")
        XCTAssertEqual(HomeRules.projectPath(workspace: "solo.k7", project: "."), "/@local/solo.k7")
        XCTAssertEqual(HomeRules.projectPath(workspace: "été", project: "x@y.k7"), "/@local/%C3%A9t%C3%A9/x@y.k7")
    }

    func testOnlyTheBareHomeIsNative() {
        let origin = URL(string: "http://127.0.0.1:18789")!
        XCTAssertTrue(HomeRules.isNativeHome(URL(string: "http://127.0.0.1:18789/")!, origin: origin))
        XCTAssertTrue(HomeRules.isNativeHome(URL(string: "http://127.0.0.1:18789")!, origin: origin))
        XCTAssertFalse(HomeRules.isNativeHome(URL(string: "http://127.0.0.1:18789/?sync=Songs")!, origin: origin))
        XCTAssertFalse(HomeRules.isNativeHome(URL(string: "http://127.0.0.1:18789/?web")!, origin: origin))
        XCTAssertFalse(HomeRules.isNativeHome(URL(string: "http://127.0.0.1:18789/settings")!, origin: origin))
        XCTAssertFalse(HomeRules.isNativeHome(URL(string: "http://127.0.0.1:18789/@local/Songs")!, origin: origin))
        XCTAssertFalse(HomeRules.isNativeHome(URL(string: "http://127.0.0.1:9999/")!, origin: origin))
    }

    func testThePillFollowsTheWebCards() {
        XCTAssertEqual(HomeRules.pill(nil, examples: false), .none)
        XCTAssertEqual(HomeRules.pill(GitState(mode: "local"), examples: false), .connect)
        XCTAssertEqual(HomeRules.pill(GitState(mode: "local"), examples: true), .none)
        XCTAssertEqual(HomeRules.pill(GitState(mode: "connected", merge: 2), examples: false), .merge(2))
        XCTAssertEqual(HomeRules.pill(GitState(mode: "connected", behind: 3, changed: 1), examples: false), .pull(3))
        XCTAssertEqual(HomeRules.pill(GitState(mode: "connected", ahead: 1, changed: 4), examples: false), .save(4))
        XCTAssertEqual(HomeRules.pill(GitState(mode: "connected"), examples: false), .none)
    }

    func testRemotesReadAsHostAndRepository() {
        XCTAssertEqual(HomeRules.remoteName("git@github.com:me/songs.git"), "github.com/me/songs")
        XCTAssertEqual(HomeRules.remoteName("ssh://git@git.example.org:2222/me/songs.git"), "git.example.org/me/songs")
    }

    func testShapesFromSurfaces() {
        XCTAssertEqual(HomeRules.shape(["turntables"]), "mix")
        XCTAssertEqual(HomeRules.shape(["timeline", "viewport"]), "video")
        XCTAssertEqual(HomeRules.shape(["timeline"]), "arrangement")
        XCTAssertEqual(HomeRules.shape([]), "live")
        XCTAssertNil(HomeRules.shape(nil))
    }
}

@MainActor
final class HomeModelTests: XCTestCase {
    func testOneReadListsOwnWorkspacesAndHidesExamples() async {
        UserDefaults.standard.removeObject(forKey: HomeModel.examplesKey)
        StubProtocol.answer = { r in
            switch r.url!.path {
            case "/auth/me": return (200, "{}")
            case "/api/files-hub": return (200, #"{"path":"/f","ticket":"T"}"#)
            case "/f/workspaces/projects":
                return (200, #"{"workspaces":[{"name":"Examples","isSingleProject":false,"projects":[]},{"name":"Mine","isSingleProject":false,"projects":[]}]}"#)
            case "/f/account/libraries": return (200, #"{"libraries":[]}"#)
            default: return (200, #"{"mode":"local","branch":"main"}"#)
            }
        }
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [StubProtocol.self]
        let model = HomeModel(client: K7Client(origin: URL(string: "http://127.0.0.1:1")!, session: URLSession(configuration: config)))
        await model.refresh()
        XCTAssertEqual(model.listed.map(\.name), ["Mine"])
        XCTAssertEqual(model.git["Mine"]?.branch, "main")
        XCTAssertEqual(model.workspaceBlocked, "ws.needLibraries")
        model.chosenExamples = true
        XCTAssertEqual(model.listed.map(\.name), ["Mine", "Examples"])
        UserDefaults.standard.removeObject(forKey: HomeModel.examplesKey)
    }

    func testOpeningAProjectHandsItsAddressOver() {
        let model = HomeModel(client: K7Client(origin: URL(string: "http://127.0.0.1:1")!))
        var opened: String?
        model.openWeb = { opened = $0 }
        model.open(workspace: "Songs", project: "a.k7")
        XCTAssertEqual(opened, "/@local/Songs/a.k7")
    }
}

final class L10nTests: XCTestCase {
    /// Every key a native page names in its sources, read off them as they are on this Mac.
    private func keysUsed() throws -> Set<String> {
        let sources = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("k7")
        let files = try FileManager.default.contentsOfDirectory(at: sources, includingPropertiesForKeys: nil)
        let call = try NSRegularExpression(pattern: #"\b(?:tr|t|plural)\("([a-z][A-Za-z0-9.]*)""#)
        var keys: Set<String> = []
        for f in files where f.pathExtension == "swift" {
            let text = try String(contentsOf: f, encoding: .utf8)
            for m in call.matches(in: text, range: NSRange(text.startIndex..., in: text)) {
                let key = String(text[Range(m.range(at: 1), in: text)!])
                let isPlural = text[Range(m.range, in: text)!].hasPrefix("plural")
                keys.formUnion(isPlural ? ["\(key).one", "\(key).other"] : [key])
            }
        }
        keys.formUnion(HomeTab.allCases.map(\.key))
        keys.formUnion(HomeRules.shapes.flatMap { [HomeRules.shapeKey($0), HomeRules.shapeKey($0) + "What"] })
        return keys
    }

    func testEveryKeyANativePageAsksForIsInBothCatalogues() throws {
        let used = try keysUsed()
        XCTAssertGreaterThan(used.count, 30)
        let l = L10n(bundle: Bundle(for: HomeModel.self))
        for key in used.sorted() { XCTAssertTrue(l.has(key), "missing \(key)") }
    }

    func testHolesAndPluralsAreFilled() {
        let l = L10n(bundle: Bundle(for: HomeModel.self), locale: "fr")
        XCTAssertEqual(l.t("home.projects"), "Projets")
        XCTAssertEqual(l.plural("repos.workspaces", 3), "3 espaces de travail")
        l.use("en")
        XCTAssertEqual(l.t("repos.card", ["name": "Songs"]), "Workspace Songs")
        l.use("de")
        XCTAssertEqual(l.locale, "en")
    }
}
