// Busy.swift — whether the app is at work, and what that holds: the audio device open behind another app, and the
// screen on. iOS keeps a background-audio app running for as long as its device runs, so the hold is the default
// and its RELEASE is the decision: idle in the background, the engine closes its device and iOS may suspend the
// app. Two sources, as on Android: the supervisor's snapshot says when a session PLAYS, whoever drives it; the views
// say what only a page knows (`busy`, apps/k7/frontend/src/androidBusy.ts and media-session.js). See engine/k7-runtime/docs/ios.md.

import os
import UIKit

final class Busy: SupervisorListener {
    static let shared = Busy()

    // Each source's reasons, keyed `<view>/<from>`; touched on the main thread only.
    private var reasons: [String: Set<String>] = [:]
    private var enginePlaying = false
    private var rendering = false
    private var behind = false
    private var armedBefore = false
    private var suspended = false
    private var cutWatch: Timer?
    private var cutBefore = false
    private let log = Logger(subsystem: "se.frediver.k7.daw", category: "k7-busy")

    private static let known: Set<String> = ["recording", "armed", "playing", "live", "capture", "awake"]

    private init() {}

    /// Once per process.
    func start() {
        SupervisorHost.shared.listen(self)
        let center = NotificationCenter.default
        center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in
            self?.behind = true
            self?.apply()
        }
        center.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main) { [weak self] _ in
            self?.behind = false
            self?.apply()
        }
    }

    /// True while anything at all is reported: what a restart offered to the user waits for.
    var busy: Bool { enginePlaying || rendering || cutRunning || !reasons.isEmpty }

    /// A stem cut, queued or running in the supervisor: minutes of work that must outlive the app going behind.
    private var cutRunning: Bool { k7_supervisor_cuts_in_flight() > 0 }

    /// One view's `busy` message, on the main thread, where the message handler delivers it.
    func report(view: String, _ msg: [String: Any]) {
        let said = Set((msg["reasons"] as? [String] ?? []).filter(Self.known.contains))
        let key = "\(view)/\(msg["from"] as? String ?? "")"
        reasons[key] = said.isEmpty ? nil : said
        apply()
    }

    func onState(_ frame: [String: Any]) {
        let sessions = frame["sessions"] as? [[String: Any]] ?? []
        let playing = sessions.contains { $0["playing"] as? Bool == true }
        // A render the engine runs for a server's job (export, freeze, stems) holds the app as playing does.
        let rendering = (frame["rendering"] as? Int ?? 0) > 0
        DispatchQueue.main.async {
            guard playing != self.enginePlaying || rendering != self.rendering else { return }
            if rendering != self.rendering { hostLog("busy: a render \(rendering ? "began" : "ended")") }
            self.enginePlaying = playing
            self.rendering = rendering
            self.apply()
        }
    }

    func onEngineExit(_ code: Int) {
        DispatchQueue.main.async {
            self.enginePlaying = false
            self.rendering = false
            self.apply()
        }
    }

    private func apply() {
        let all = reasons.values.reduce(into: Set<String>()) { $0.formUnion($1) }
        // The screen stays on for what a person is watching; the engine played from another device leaves it alone.
        UIApplication.shared.isIdleTimerDisabled = !all.isEmpty
        // Behind another app with nothing at work, the device goes: no silent audio keeps a phone awake for nothing.
        let cut = cutRunning
        let release = behind && all.subtracting(["awake"]).isEmpty && !enginePlaying && !rendering && !cut
        // A cut sends no event when it ends, so a timer looks again for as long as the app is behind.
        if behind && cutWatch == nil {
            cutWatch = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in self?.apply() }
        } else if !behind {
            cutWatch?.invalidate()
            cutWatch = nil
        }
        if cut != cutBefore {
            cutBefore = cut
            log.info("[host] busy: cut=\(cut) behind=\(self.behind)")
        }
        if release != suspended {
            suspended = release
            log.info("[host] busy: release=\(release) behind=\(self.behind) cut=\(cut) playing=\(self.enginePlaying)")
            EngineBridge.suspendAudio(release)
        }
        // A track armed is the engine asked for its inputs: the moment to ask for the microphone, if nobody has.
        let armed = all.contains("armed")
        if armed && !armedBefore { MicAccess.request { _ in } }
        armedBefore = armed
    }
}
