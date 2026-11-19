// EngineBridge.swift — the engine in this process: k7runtime.framework, serving on a thread of its own whose
// run loop becomes JUCE's message loop. The SUPERVISOR decides when, and with which identity, exactly as it
// spawns the engine on desktop. Once per PROCESS: JUCE stays bound to that thread, so a stopped engine is
// restarted by a new process (engine/k7-runtime/docs/ios.md).

import AVFAudio
import Foundation
import os

enum EngineBridge {
    private static let log = Logger(subsystem: "se.frediver.k7.daw", category: "k7-runtime")
    private static let lock = NSLock()
    private static var started = false
    private static var serving = false
    private static let exited = DispatchSemaphore(value: 0)

    /// Whether the microphone was granted when the engine opened its device; nil before it started. Its inputs are fixed then.
    private(set) static var startedWithMic: Bool?

    /// Serve with the supervisor's identity, once; `onExit` gets serve()'s exit code, on the engine's thread.
    static func start(id: String, token: String, port: Int, host: String, origins: String, onExit: @escaping (Int) -> Void) {
        lock.lock()
        defer { lock.unlock() }
        if started {
            log.warning("engine already started in this process — a restart needs a new process")
            return
        }
        // A simulator whose audio server does not answer aborts the app in the device's open: this runs the app without the engine.
        if ProcessInfo.processInfo.environment["K7_NO_ENGINE"] == "1" {
            log.warning("K7_NO_ENGINE=1: the engine is not started")
            return
        }
        started = true
        serving = true
        // What the supervisor's spawn passes on desktop, read by serve() when it starts.
        setenv("STUDIO_HOME", stateDir.path, 1)
        // The backend sends its own paths, outside `.k7`: without these the engine drops every clip and sample.
        setenv("STUDIO_ENGINE_ROOTS", BackendHost.engineRoots.map(\.path).joined(separator: ":"), 1)
        setenv("STUDIO_ENGINE_HOST", host, 1)
        setenv("STUDIO_ENGINE_ID", id, 1)
        setenv("STUDIO_ENGINE_TOKEN", token, 1)
        setenv("STUDIO_ENGINE_ALLOWED_ORIGINS", origins, 1)
        let mic = AVAudioApplication.shared.recordPermission == .granted
        startedWithMic = mic
        // iOS asks for the microphone as soon as inputs open: without it granted, outputs only (MicAccess.swift).
        // On a Mac a granted microphone opens its input at the first armed track ("arm"), not at start (docs/ios.md).
        setenv("STUDIO_ENGINE_INPUTS", mic ? (ProcessInfo.processInfo.isiOSAppOnMac ? "arm" : "1") : "0", 1)
        let thread = Thread {
            let code = Int(k7_engine_serve(Int32(port)))
            lock.lock()
            serving = false
            lock.unlock()
            exited.signal()
            onExit(code)
        }
        thread.name = "k7-runtime"
        // The engine's frames are deep, and a secondary thread's default stack is 512 KiB.
        thread.stackSize = 16 << 20
        thread.qualityOfService = .userInteractive
        thread.start()
    }

    /// Ask the engine to stop and wait for it, at most `timeout` seconds; true when it did.
    static func stop(timeout: TimeInterval) -> Bool {
        lock.lock()
        let running = serving
        lock.unlock()
        guard running else { return true }
        k7_engine_quit()
        let done = exited.wait(timeout: .now() + timeout) == .success
        if done { exited.signal() }
        return done
    }

    /// The device closed while the app sits idle behind another, so iOS may suspend it, and reopened on its return (Busy.swift).
    static func suspendAudio(_ suspended: Bool) {
        lock.lock()
        let running = serving
        lock.unlock()
        if running {
            hostLog("audio device \(suspended ? "closed" : "reopened") behind another app")
            k7_engine_suspend_audio(suspended ? 1 : 0)
        }
    }

    /// `~/.k7` on a phone: the app's own files, shared by the engine and the supervisor.
    static var stateDir: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let dir = base.appendingPathComponent(".k7", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }
}
