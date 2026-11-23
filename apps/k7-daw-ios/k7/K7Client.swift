// K7Client.swift — the native pages' one way to k7's backend on this device: the same local HTTP routes the web
// home calls (the session at /auth/me, the files hub's hello, then the account's file service and its git routes).
// What a reader gets wrong: nothing here is a second backend; a native page and the web page share every rule.
// See docs/native-ui-plan.md.

import Foundation

/// One workspace of the account with its projects: `GET <files>/workspaces/projects`.
struct Workspace: Decodable, Equatable, Identifiable {
    let name: String
    let isSingleProject: Bool
    let projects: [Project]
    var id: String { name }
}

/// One project folder in a workspace, as the file service lists it.
struct Project: Decodable, Equatable, Identifiable {
    let path: String
    let app: String
    var tags: [String]?
    var surfaces: [String]?
    var active: Bool?
    /// Epoch milliseconds.
    var editedAt: Double?
    var demo: Bool?
    var id: String { path }
}

/// A workspace's git state: `GET <files>/git/state?workspace=`.
struct GitState: Decodable, Equatable {
    let mode: String
    var branch: String?
    var remote: String?
    var ahead: Int?
    var behind: Int?
    var changed: Int?
    var merge: Int?
    var remoteEmpty: Bool?
}

/// A refused call: the status and the sentence the backend said.
struct K7Error: LocalizedError, Equatable {
    let status: Int
    let message: String
    var errorDescription: String? { message }
}

final class K7Client {
    let origin: URL
    private let session: URLSession
    private var files: (base: URL, ticket: String, user: String)?
    // Whether `/auth/me` has handed the session cookie over, which the front's own routes ask for.
    private var signed = false

    init(origin: URL, session: URLSession = .shared) {
        self.origin = origin
        self.session = session
    }

    private struct Me: Decodable {
        struct User: Decodable {
            var email: String?
            var sub: String?
        }
        var user: User?
    }

    private struct Hello: Decodable {
        let path: String
        let ticket: String
    }

    private struct WorkspaceList: Decodable { let workspaces: [Workspace] }

    private struct Libraries: Decodable {
        struct Library: Decodable { let name: String? }
        let libraries: [Library]
    }

    private struct Created: Decodable {
        let project: String
        var missing: [String]?
    }

    /// The session cookie first (`/auth/me` hands it over), then the hub's hello: where the files are, and the ticket.
    private func filesService() async throws -> (base: URL, ticket: String, user: String) {
        if let files { return files }
        let me: Me = try await get(origin.appendingPathComponent("auth/me"), headers: [:])
        signed = true
        let hello: Hello = try await get(origin.appendingPathComponent("api/files-hub"), headers: [:])
        let user = me.user?.email ?? me.user?.sub ?? "unknown"
        let found = (URL(string: origin.absoluteString + hello.path)!, hello.ticket, user)
        files = found
        return found
    }

    /// One call to the file service with its headers; a 401 asks the hub for a fresh ticket once.
    func files<T: Decodable>(_ path: String, method: String = "GET", workspace: String? = nil,
                                     json: [String: Any]? = nil, retried: Bool = false) async throws -> T {
        let f = try await filesService()
        var headers = ["x-studio-user": f.user, "x-studio-engine-token": f.ticket]
        if let workspace { headers["x-studio-workspace"] = workspace }
        guard let url = URL(string: f.base.absoluteString + path) else { throw K7Error(status: 0, message: path) }
        do {
            return try await send(url, method: method, headers: headers, json: json)
        } catch let e as K7Error where e.status == 401 && !retried {
            files = nil
            return try await self.files(path, method: method, workspace: workspace, json: json, retried: true)
        }
    }

    /// The session cookie, once: the front's own routes (the preferences, the AI key, an engine's chains) hold to it.
    func signIn() async throws {
        if signed { return }
        let _: AnyDecodable = try await get(origin.appendingPathComponent("auth/me"), headers: [:])
        signed = true
    }

    private func get<T: Decodable>(_ url: URL, headers: [String: String]) async throws -> T {
        try await send(url, method: "GET", headers: headers, json: nil)
    }

    func send<T: Decodable>(_ url: URL, method: String, headers: [String: String], json: [String: Any]?) async throws -> T {
        var request = URLRequest(url: url)
        request.httpMethod = method
        for (k, v) in headers { request.setValue(v, forHTTPHeaderField: k) }
        if let json {
            request.setValue("application/json", forHTTPHeaderField: "content-type")
            request.httpBody = try JSONSerialization.data(withJSONObject: json)
        }
        let (data, response) = try await session.data(for: request)
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(status) else {
            let body = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
            throw K7Error(status: status, message: body?["error"] as? String ?? "\(url.path) \(status)")
        }
        return try JSONDecoder().decode(T.self, from: data)
    }

    // MARK: the routes the home calls

    func workspaces() async throws -> [Workspace] {
        let list: WorkspaceList = try await files("/workspaces/projects")
        return list.workspaces
    }

    /// How many libraries the account has: none means no workspace can be made yet.
    func libraryCount() async throws -> Int {
        let r: Libraries = try await files("/account/libraries")
        return r.libraries.count
    }

    func gitState(_ workspace: String) async throws -> GitState {
        let q = workspace.addingPercentEncoding(withAllowedCharacters: .urlQueryValueAllowed) ?? workspace
        return try await files("/git/state?workspace=\(q)")
    }

    func createWorkspace(_ name: String) async throws {
        let _: AnyDecodable = try await files("/workspaces", method: "POST", json: ["name": name])
    }

    /// Scaffold a project of `shape` named `folder` (suffix included); the answer is its path and the modules it still owes.
    func createProject(workspace: String, folder: String, shape: String) async throws -> (project: String, missing: [String]) {
        let r: Created = try await files("/project/create", method: "POST", workspace: workspace,
                                         json: ["shape": shape, "name": folder])
        return (r.project, r.missing ?? [])
    }
}

/// Any JSON value, read and dropped: for answers whose content nothing uses.
struct AnyDecodable: Decodable {
    init(from decoder: Decoder) throws {}
}

extension CharacterSet {
    /// A query value: ASCII unreserved characters only, so `&`, `=` and `+` in a name are escaped.
    static let urlQueryValueAllowed: CharacterSet = {
        CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
    }()
}
