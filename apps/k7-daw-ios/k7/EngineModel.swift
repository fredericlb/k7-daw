// EngineModel.swift — this phone's engine for the native Engine page and the instruments: the supervisor's latest
// state, pushed as the web page's `k7Ios` channel gets it (StudioBridge.swift), and the commands that page may send.
// What a reader gets wrong: it asks SupervisorHost directly, as the bridge does; the engine has no route of its own here.
// See docs/native-ui-plan.md and apps/k7-daw-engine/k7-runtime/docs/ios.md.

import Foundation
import SwiftUI

@MainActor
final class EngineModel: ObservableObject, SupervisorListener {
    @Published private(set) var state: [String: Any]?
    @Published private(set) var exit: Int?
    @Published private(set) var lan: String?
    @Published private(set) var caCount: Int?
    @Published private(set) var caNote: String?
    @Published private(set) var caBusy = false
    private var listening = false

    /// The supervisor's picture and where commands go; the tests hand their own.
    private let known: () -> (state: [String: Any]?, engineExit: Int?)
    private let send: (String, String) -> Void

    init(known: @escaping () -> (state: [String: Any]?, engineExit: Int?) = { SupervisorHost.shared.known() },
         send: @escaping (String, String) -> Void = { SupervisorHost.shared.command($0, $1) }) {
        self.known = known
        self.send = send
    }

    var engineId: String? { state?["engineId"] as? String }
    var lanOpen: Bool { state?["lanOpen"] as? Bool ?? false }
    var instruments: [SettingsRules.Instrument] { SettingsRules.instruments(state) }

    func line(_ l: L10n) -> String { SettingsRules.engineLine(state: state, exit: exit, lan: lan, l: l) }

    /// Follow the supervisor while a page shows the engine.
    func start() {
        read()
        if !listening {
            listening = true
            SupervisorHost.shared.listen(self)
        }
        lan = EngineOps.lanAddress()
        caCount = TrustedCas.count()
    }

    func stop() {
        guard listening else { return }
        listening = false
        SupervisorHost.shared.unlisten(self)
    }

    func read() {
        let k = known()
        state = k.state
        exit = k.engineExit
    }

    nonisolated func onState(_ frame: [String: Any]) {
        Task { @MainActor in self.read() }
    }

    nonisolated func onEngineExit(_ code: Int) {
        Task { @MainActor in self.read() }
    }

    func setLanOpen(_ on: Bool) { send("lan.open", on ? "1" : "0") }

    func instrument(_ cmd: String, _ name: String) { send(cmd, name) }

    /// Trust a certificate the person picks, or forget theirs; the supervisor restarts with the new roots.
    func cas(_ method: String, doing: String, pick: (@escaping (URL?) -> Void) -> Void) {
        caBusy = true
        caNote = L10n.shared.t("android.reconnecting", ["doing": doing])
        let done: (Result<[String: Any], Error>) -> Void = { result in
            Task { @MainActor [self] in
                switch result {
                case .success(let v):
                    self.caCount = v["count"] as? Int ?? TrustedCas.count()
                    self.caNote = (v["said"] as? String).flatMap { $0.isEmpty ? nil : $0 }
                case .failure(let e):
                    self.caNote = e.localizedDescription
                }
                self.caBusy = false
            }
        }
        if method == "forgetCas" { return EngineOps.forgetCas(done: done) }
        pick { url in
            guard let url else { return done(.success(["said": "", "count": TrustedCas.count()])) }
            EngineOps.importCa(url, done: done)
        }
    }
}
