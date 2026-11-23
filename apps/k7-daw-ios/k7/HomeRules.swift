// HomeRules.swift — the Projects home's rules, pure: the order and filter of the list, a new folder's name, a project's
// address, what a workspace's git state offers, and which web address the native home or Settings stands in for.
// Each mirrors the web home's own (apps/k7-daw-frontend/src/projects/, src/git/gitState.ts), so both say the same.
// See docs/native-ui-plan.md.

import Foundation

enum HomeRules {
    /// The bundled demos' workspace: listed last, and taking no new project.
    static let examples = "Examples"
    static let suffix = ".k7"

    static func isExamples(_ w: Workspace) -> Bool { w.name == examples }

    /// People's order (numbers as numbers, case aside), the web's `byPeople`.
    static func byPeople(_ a: String, _ b: String) -> Bool {
        let c = a.localizedStandardCompare(b)
        return c == .orderedSame ? a < b : c == .orderedAscending
    }

    /// Two slash paths, a segment at a time, so a folder's projects stay together.
    static func byPeoplePath(_ a: String, _ b: String) -> Bool {
        let p = a.split(separator: "/").map(String.init)
        let q = b.split(separator: "/").map(String.init)
        for (x, y) in zip(p, q) where x != y { return byPeople(x, y) }
        return p.count < q.count
    }

    /// Examples last, the rest in people's order, each one's projects by path.
    static func pageOrder(_ all: [Workspace]) -> [Workspace] {
        all.map { Workspace(name: $0.name, isSingleProject: $0.isSingleProject,
                            projects: $0.projects.sorted { byPeoplePath($0.path, $1.path) }) }
            .sorted { a, b in
                isExamples(a) != isExamples(b) ? !isExamples(a) : byPeople(a.name, b.name)
            }
    }

    /// What `query` leaves: a workspace by its name (all its projects), else its projects by path.
    static func filter(_ all: [Workspace], _ query: String) -> [Workspace] {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        if q.isEmpty { return all }
        return all.compactMap { w in
            if w.name.lowercased().contains(q) { return w }
            let hits = w.projects.filter { $0.path.lowercased().contains(q) }
            return hits.isEmpty ? nil : Workspace(name: w.name, isSingleProject: w.isSingleProject, projects: hits)
        }
    }

    /// A workspace's folder from what was typed, or the message key saying why not (the web's `workspaceFolderName`).
    static func workspaceFolder(_ raw: String) -> (folder: String, error: String?) {
        let name = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if name.isEmpty { return ("", "name.required") }
        if name.contains("/") || name.contains("\\") { return ("", "name.noSlash") }
        if name.hasPrefix(".") { return ("", "name.noDot") }
        let stem = name.count > suffix.count && name.lowercased().hasSuffix(suffix) ? String(name.dropLast(suffix.count)) : name
        return stem.isEmpty ? ("", "name.required") : (stem, nil)
    }

    /// …and a project's, which carries the suffix.
    static func projectFolder(_ raw: String) -> (folder: String, error: String?) {
        let w = workspaceFolder(raw)
        return w.error == nil ? (w.folder + suffix, nil) : w
    }

    /// The name a row shows: the folder without its suffix, or the workspace's when the workspace is the project.
    static func label(_ w: Workspace, _ p: Project) -> String {
        let last = p.path == "." ? w.name : (p.path.split(separator: "/").last.map(String.init) ?? p.path)
        return last.count > suffix.count && last.hasSuffix(suffix) ? String(last.dropLast(suffix.count)) : last
    }

    /// The folder holding the project, or nil at the workspace's root.
    static func parent(_ p: Project) -> String? {
        guard let cut = p.path.range(of: "/", options: .backwards) else { return nil }
        return String(p.path[..<cut.lowerBound])
    }

    /// The archetype a project's surfaces read as (the web's `shapeOfSurfaces`), or nil when the service does not say.
    static func shape(_ surfaces: [String]?) -> String? {
        guard let s = surfaces else { return nil }
        if s.contains("turntables") { return "mix" }
        if s.contains("timeline") { return s.contains("viewport") ? "video" : "arrangement" }
        if s.contains("viewport") { return "live-video" }
        return "live"
    }

    /// The message key of an archetype's label.
    static func shapeKey(_ shape: String) -> String {
        shape == "live-video" ? "shape.liveVideo" : "shape.\(shape)"
    }

    static let shapes = ["arrangement", "mix", "video", "live", "live-video"]

    /// One path segment as `encodeURIComponent` writes it, `@` kept (web-protocol's `encodeSegment`).
    static func segment(_ s: String) -> String {
        let keep = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.!~*'()@")
        return s.addingPercentEncoding(withAllowedCharacters: keep) ?? s
    }

    /// A project's address on k7's origin: `/@local/<workspace>/<project…>`, the workspace alone for `.`.
    static func projectPath(workspace: String, project: String) -> String {
        let parts = [workspace] + (project == "." ? [] : project.split(separator: "/").map(String.init))
        return "/@local/" + parts.map(segment).joined(separator: "/")
    }

    // MARK: the web pages the native home hands over to

    static let surfacesRoute = "/surfaces"
    static let tasksRoute = "/tasks"
    static let librariesRoute = "/libraries"
    static let settingsRoute = "/settings"
    static let engineRoute = "/settings/engine"
    static let cloneRoute = "/?clone"
    /// The web home asked for by name: its cards still do Save, Pull and the merge until those are native.
    static let webHomeRoute = "/?web"

    static func syncRoute(_ workspace: String) -> String {
        "/?sync=\(workspace.addingPercentEncoding(withAllowedCharacters: .urlQueryValueAllowed) ?? workspace)"
    }

    static func browseRoute(_ workspace: String) -> String {
        "/?ws=\(workspace.addingPercentEncoding(withAllowedCharacters: .urlQueryValueAllowed) ?? workspace)"
    }

    /// What the native home draws in place of the web page at `url`: the bare Projects home, or a native Settings page.
    enum NativePage: Equatable {
        case projects
        case settings([SettingsPage])
    }

    /// The native page standing in for `url`, or nil when it stays web: a `web` item in the query keeps any address web.
    static func nativePage(_ url: URL, origin: URL) -> NativePage? {
        guard url.scheme == origin.scheme, url.host == origin.host, url.port == origin.port else { return nil }
        let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        if items.contains(where: { $0.name == "web" }) { return nil }
        switch url.path {
        case "", "/":
            return url.query == nil || url.query == "" ? .projects : nil
        case settingsRoute:
            guard let asked = items.first(where: { $0.name == "tab" })?.value else { return .settings([]) }
            guard let tab = SettingsTab(rawValue: asked) else { return .settings([]) }
            return tab.native ? .settings([.tab(tab)]) : nil
        case engineRoute:
            return .settings([.tab(.devices), .engine])
        default:
            return nil
        }
    }

    /// Whether the web page, at `url`, is on the bare Projects home, which the native home replaces.
    static func isNativeHome(_ url: URL, origin: URL) -> Bool {
        nativePage(url, origin: origin) == .projects
    }

    // MARK: git

    /// What a workspace's header offers, read off its git state as the web card's pills are.
    enum GitPill: Equatable {
        case none
        case connect
        case merge(Int)
        case pull(Int)
        case save(Int)
    }

    static func pill(_ s: GitState?, examples: Bool) -> GitPill {
        guard let s else { return .none }
        if s.mode != "connected" { return examples ? .none : .connect }
        if let m = s.merge { return .merge(m) }
        let behind = s.behind ?? 0
        if behind > 0 { return .pull(behind) }
        let changed = s.changed ?? 0
        let ahead = s.ahead ?? 0
        if changed > 0 { return .save(changed) }
        if ahead > 0 { return .save(ahead) }
        return .none
    }

    /// A remote as a person reads it: host and repository, never the user or the scheme (`gitState.ts`'s `remoteName`).
    static func remoteName(_ url: String) -> String {
        let u = url.trimmingCharacters(in: .whitespaces)
        var host = ""
        var path = ""
        if u.hasPrefix("ssh://") || u.hasPrefix("https://") || u.hasPrefix("http://"),
           let parsed = URLComponents(string: u), let h = parsed.host {
            host = h
            path = parsed.path
        } else if let colon = u.firstIndex(of: ":") {
            let left = u[..<colon]
            host = String(left.split(separator: "@").last ?? left)
            path = String(u[u.index(after: colon)...])
        } else {
            return u
        }
        path = path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        if path.hasSuffix(".git") { path = String(path.dropLast(4)) }
        return path.isEmpty ? host : "\(host)/\(path)"
    }

    /// The line under a workspace's name: where it syncs, or that it stays on this device.
    static func gitLine(_ s: GitState?, l: L10n) -> String? {
        guard let s, s.mode != "none" else { return nil }
        if s.mode == "connected", let r = s.remote { return remoteName(r) }
        return l.t("git.headlineLocal", ["branch": s.branch ?? ""])
    }
}
