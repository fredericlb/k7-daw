// SettingsModel.swift — the native Settings' state: the language and theme (Prefs.swift), the commit author, the AI
// key, the separation chains and what this backend is, read when a page shows and written as the web page writes them.
// Everything goes through K7Client; what a value means is SettingsRules'. See docs/native-ui-plan.md.

import Foundation
import SwiftUI

@MainActor
final class SettingsModel: ObservableObject {
    let client: K7Client
    let prefs: AppPrefs

    @Published private(set) var identity: Identity?
    /// "Saved." or a refusal, under the field it is said of.
    @Published private(set) var identityNote: (field: String, text: String, error: Bool)?
    @Published private(set) var ai: AiSettings?
    @Published private(set) var aiError: String?
    @Published private(set) var aiBusy = false
    @Published private(set) var cold: ColdChains?
    @Published private(set) var held: EngineChains?
    @Published private(set) var chainsError: String?
    @Published private(set) var chainsBusy = false
    /// A model download started here: which chain, its line, and whether it still runs.
    @Published private(set) var fetching: (ref: String, text: String, running: Bool, failed: Bool)?
    @Published private(set) var local: LocalApp?
    @Published var error: String?
    /// The engine whose chains are named: this device's, from its supervisor.
    var engineId: () -> String? = { nil }
    /// Where a page not native yet is opened: the shell's handover (HomeHost).
    var openWeb: (String) -> Void = { _ in }

    init(client: K7Client, prefs: AppPrefs? = nil) {
        self.client = client
        self.prefs = prefs ?? .shared
    }

    /// What a failed call says on the page; nothing for a read cancelled because the page was drawn again.
    static func said(_ error: Error) -> String? {
        error is CancellationError || (error as? URLError)?.code == .cancelled ? nil : error.localizedDescription
    }

    // MARK: General

    func loadGeneral() async {
        async let id = try? client.identity()
        async let key = client.aiSettings()
        identity = await id
        do { ai = try await key } catch { aiError = Self.said(error) }
        await loadChains()
    }

    func setLocale(_ locale: String) async {
        do { try await prefs.setLocale(locale, client: client) } catch { self.error = Self.said(error) }
    }

    func setTheme(_ theme: Theme) async {
        do { try await prefs.setTheme(theme, client: client) } catch { self.error = Self.said(error) }
    }

    /// Save what the fields hold, unless it is what is stored; a field showing its suggestion is sent empty.
    func saveIdentity(name: String, email: String) async {
        guard let known = identity else { return }
        let n = name.trimmingCharacters(in: .whitespaces)
        let e = email.trimmingCharacters(in: .whitespaces)
        if n == known.name && e == known.email { return }
        do {
            try await client.saveIdentity(name: SettingsRules.typed(n, shown: known.name, source: known.nameSource),
                                          email: SettingsRules.typed(e, shown: known.email, source: known.emailSource))
            identity = (try? await client.identity()) ?? Identity(name: n, email: e)
            identityNote = ("email", L10n.shared.t("git.identitySaved"), false)
        } catch {
            // The route names no field: an empty name is the name's, anything else the email's.
            identityNote = (n.isEmpty ? "name" : "email", error.localizedDescription, true)
        }
    }

    func clearIdentityNote() { identityNote = nil }

    /// A PATCH of the AI settings: an absent field is kept, `NSNull()` clears it.
    func saveAi(_ patch: [String: Any]) async {
        aiBusy = true
        aiError = nil
        do { ai = try await client.saveAiSettings(patch) } catch { aiError = Self.said(error) }
        aiBusy = false
    }

    func loadChains() async {
        do { cold = try await client.coldChains() } catch { chainsError = Self.said(error) }
        guard let id = engineId() else { return }
        do { held = try await client.engineChains(id) } catch { chainsError = Self.said(error) }
    }

    /// Name a chain on the engine's machine; its answer is what it then holds, so a refusal snaps back.
    func choose(_ which: String, _ ref: String) async {
        guard let id = engineId() else { return }
        chainsBusy = true
        chainsError = nil
        do { held = try await client.setEngineChain(id, which: which, ref: ref) } catch { chainsError = Self.said(error) }
        chainsBusy = false
    }

    /// Fetch every model the chain lacks, one after the other, then read the list again.
    func download(_ ref: String) async {
        fetching = (ref, "…", true, false)
        do {
            for var job in try await client.downloadChainModels(ref) {
                while !job.done {
                    fetching = (ref, SettingsRules.download(job, l: .shared), true, false)
                    try await Task.sleep(nanoseconds: 1_000_000_000)
                    job = (try? await client.modelJob(job.id)) ?? ModelJob(id: job.id, file: job.file, received: job.received,
                                                                       total: job.total, done: true, error: "?")
                }
                fetching = (ref, SettingsRules.download(job, l: .shared), false, job.error != nil)
                if job.error != nil { return }
            }
            cold = try await client.coldChains()
        } catch {
            fetching = (ref, error.localizedDescription, false, true)
        }
    }

    // MARK: About

    func loadLocal() async {
        local = try? await client.localApp()
    }
}
