// HomeModel.swift — the Projects home's state: the account's workspaces read every three seconds while the home is on
// screen, each one's git state less often, and the two writes the home makes itself (a workspace, a project).
// Everything it reads or writes goes through K7Client; what the list means is HomeRules'. See docs/native-ui-plan.md.

import Foundation
import SwiftUI

@MainActor
final class HomeModel: ObservableObject {
    @Published private(set) var workspaces: [Workspace] = []
    @Published private(set) var git: [String: GitState] = [:]
    @Published private(set) var loading = true
    @Published private(set) var error: String?
    /// A warning that is not a failure: the modules a just-made project still owes.
    @Published var notice: String?
    @Published private(set) var libraries: Int?
    @Published var query = ""
    /// The ⋮'s Show examples, kept on this device as the web keeps it in the page.
    @Published var chosenExamples = UserDefaults.standard.bool(forKey: HomeModel.examplesKey) {
        didSet { UserDefaults.standard.set(chosenExamples, forKey: Self.examplesKey) }
    }
    static let examplesKey = "k7.home.showExamples"

    private let client: K7Client
    /// Where a project or a web page is opened: the shell's handover (HomeHost).
    var openWeb: (String) -> Void = { _ in }
    private var poll: Task<Void, Never>?
    private var ticks = 0
    // The list as last read, so an unchanged answer leaves the views alone.
    private var lastRead: [Workspace] = []

    /// The web home's pace: brisk enough that another device's change shows, cheap on a short list.
    static let pollSeconds: UInt64 = 3
    /// Git states are read on every fifth list read: each is a call per workspace.
    static let gitEvery = 5

    init(client: K7Client) {
        self.client = client
    }

    var hasOwn: Bool { workspaces.contains { !HomeRules.isExamples($0) } }
    /// Examples cannot be hidden while it is all there is: the page would be blank.
    var showExamples: Bool { chosenExamples || !hasOwn }

    /// What the page lists: the shown workspaces, in order, filtered.
    var listed: [Workspace] {
        let shown = workspaces.filter { showExamples || !HomeRules.isExamples($0) }
        return HomeRules.filter(HomeRules.pageOrder(shown), query)
    }

    var projectTotal: Int { workspaces.filter { showExamples || !HomeRules.isExamples($0) }.reduce(0) { $0 + $1.projects.count } }

    /// Why a new workspace cannot be made, as a message key, or nil.
    var workspaceBlocked: String? { libraries == 0 ? "ws.needLibraries" : nil }

    /// Read now and then every few seconds, until `stop`.
    func start() {
        guard poll == nil else { return }
        ticks = 0
        poll = Task { [weak self] in
            while !Task.isCancelled {
                await self?.refresh()
                try? await Task.sleep(nanoseconds: Self.pollSeconds * 1_000_000_000)
            }
        }
    }

    func stop() {
        poll?.cancel()
        poll = nil
    }

    /// One read of the list, and of the git states on every fifth.
    func refresh() async {
        do {
            let next = try await client.workspaces()
            if next != lastRead {
                lastRead = next
                workspaces = next
            }
            error = nil
            if libraries == nil { libraries = try? await client.libraryCount() }
        } catch {
            // The list IS the page's subject: say it failed rather than draw an empty one.
            self.error = error.localizedDescription
        }
        loading = false
        if ticks % Self.gitEvery == 0 { await refreshGit() }
        ticks += 1
    }

    private func refreshGit() async {
        var states: [String: GitState] = [:]
        for w in workspaces {
            if let s = try? await client.gitState(w.name) { states[w.name] = s }
        }
        if states != git { git = states }
    }

    /// Make an empty workspace; the first project is chosen by opening it, as on the web.
    func createWorkspace(_ folder: String) async throws {
        try await client.createWorkspace(folder)
        await refresh()
    }

    /// Make a project and open it, or stay here with the notice when it owes library modules.
    func createProject(workspace: String, folder: String, shape: String) async throws {
        let made = try await client.createProject(workspace: workspace, folder: folder, shape: shape)
        await refresh()
        if made.missing.isEmpty {
            notice = nil
            open(workspace: workspace, project: made.project)
        } else {
            notice = L10n.shared.t("project.missingModules",
                                   ["project": made.project, "modules": made.missing.joined(separator: ", ")])
        }
    }

    func open(workspace: String, project: String) {
        openWeb(HomeRules.projectPath(workspace: workspace, project: project))
    }
}
