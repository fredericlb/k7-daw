// K7ClientSettings.swift — the routes the native Settings calls, the web settings page's own: the preferences
// (`/api/preferences`), the AI key (`/api/ai/settings`), the commit identity (`<files>/git/identity`), the separation
// chains (`<files>/account/cold-chains`, `/api/engines/<id>/chains`), and what this backend is (`/api/local`).
// What a reader gets wrong: the preferences are written whole, so a change is a read, a merge and a write. See docs/native-ui-plan.md.

import Foundation

/// The AI key as the backend shows it: a hint of the key, never the key (`AiSettingsView`).
struct AiSettings: Decodable, Equatable {
    var model: String?
    var defaultModel: String
    var keyHint: String?
}

/// The device's commit author; a field left empty says where its suggestion came from.
struct Identity: Codable, Equatable {
    var name: String
    var email: String
    var nameSource: String?
    var emailSource: String?
}

/// The library's cold chains, as this machine can run them (`ColdChainsReply`).
struct ColdChains: Decodable, Equatable {
    struct Chain: Decodable, Equatable, Identifiable {
        struct Download: Decodable, Equatable { let file: String }
        let ref: String
        let instrumental: Bool
        var unavailable: String?
        var downloads: [Download]
        var id: String { ref }
    }
    let chains: [Chain]
}

/// What an engine's machine separates with and makes an instrumental with; absent is "none named".
struct EngineChains: Codable, Equatable {
    var stems: String?
    var instrumental: String?
}

/// One model download the file service runs as a job (`ModelDownloadJob`).
struct ModelJob: Decodable, Equatable {
    let id: String
    let file: String
    let received: Double
    var total: Double?
    let done: Bool
    var error: String?
}

/// `GET /api/local`: whether this backend runs the machine's engine, and its version.
struct LocalApp: Decodable, Equatable {
    let engine: Bool
    let version: String
}

extension K7Client {
    private struct ChainsReply: Decodable { var chains: EngineChains? }
    private struct Jobs: Decodable { let jobs: [ModelJob] }

    /// One of the front's own routes, with the session cookie.
    private func front<T: Decodable>(_ path: String, method: String = "GET", json: [String: Any]? = nil) async throws -> T {
        try await signIn()
        guard let url = URL(string: origin.absoluteString + path) else { throw K7Error(status: 0, message: path) }
        return try await send(url, method: method, headers: [:], json: json)
    }

    // MARK: the preferences

    /// The stored document, `{}` for one never saved.
    func prefs() async throws -> [String: Any] {
        let raw: RawJSON = try await front("/api/preferences")
        return raw.object
    }

    /// Merge `patch` into the stored document and write it whole, as the web's `patchPrefs` does.
    func patchPrefs(_ patch: [String: Any]) async throws {
        let current = (try? await prefs()) ?? [:]
        let next = current.merging(patch) { _, new in new }
        let _: AnyDecodable = try await front("/api/preferences", method: "PUT", json: next)
    }

    // MARK: the AI key

    func aiSettings() async throws -> AiSettings {
        try await front("/api/ai/settings")
    }

    /// A field left out is kept, `NSNull()` clears it: the key is never sent back to be kept.
    func saveAiSettings(_ patch: [String: Any]) async throws -> AiSettings {
        try await front("/api/ai/settings", method: "PUT", json: patch)
    }

    // MARK: the commit identity

    func identity() async throws -> Identity {
        try await files("/git/identity")
    }

    func saveIdentity(name: String, email: String) async throws {
        let _: AnyDecodable = try await files("/git/identity", method: "POST", json: ["name": name, "email": email])
    }

    // MARK: the separation chains

    func coldChains() async throws -> ColdChains {
        try await files("/account/cold-chains")
    }

    func engineChains(_ engine: String) async throws -> EngineChains {
        let r: ChainsReply = try await front("/api/engines/\(HomeRules.segment(engine))/chains")
        return r.chains ?? EngineChains()
    }

    /// Name one on that machine: a key left out is untouched, `""` unsays it; the answer is what it then holds.
    func setEngineChain(_ engine: String, which: String, ref: String) async throws -> EngineChains {
        let r: ChainsReply = try await front("/api/engines/\(HomeRules.segment(engine))/chains", method: "PUT", json: [which: ref])
        return r.chains ?? EngineChains()
    }

    /// Start the downloads of the models a cold chain lacks: one job each.
    func downloadChainModels(_ ref: String) async throws -> [ModelJob] {
        let r: Jobs = try await files("/account/cold-chains/download", method: "POST", json: ["ref": ref])
        return r.jobs
    }

    func modelJob(_ id: String) async throws -> ModelJob {
        try await files("/account/models/\(HomeRules.segment(id))")
    }

    // MARK: this backend

    func localApp() async throws -> LocalApp {
        try await front("/api/local")
    }
}

/// A JSON object kept as it came, for a document whose every key must survive a write.
struct RawJSON: Decodable {
    let object: [String: Any]

    init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer().decode(JSONValue.self)
        object = (value.any as? [String: Any]) ?? [:]
    }
}

/// Any JSON value, decoded into Foundation's own types.
private struct JSONValue: Decodable {
    let any: Any

    init(from decoder: Decoder) throws {
        if var list = try? decoder.unkeyedContainer() {
            var items: [Any] = []
            while !list.isAtEnd { items.append(try list.decode(JSONValue.self).any) }
            any = items
        } else if let keyed = try? decoder.container(keyedBy: Key.self) {
            var out: [String: Any] = [:]
            for k in keyed.allKeys { out[k.stringValue] = try keyed.decode(JSONValue.self, forKey: k).any }
            any = out
        } else {
            let one = try decoder.singleValueContainer()
            if one.decodeNil() { any = NSNull() }
            else if let b = try? one.decode(Bool.self) { any = b }
            else if let n = try? one.decode(Double.self) { any = n }
            else { any = try one.decode(String.self) }
        }
    }

    private struct Key: CodingKey {
        let stringValue: String
        var intValue: Int? { nil }
        init(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { nil }
    }
}
