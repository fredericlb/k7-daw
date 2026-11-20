// BackendHost.swift — k7's backend on the phone: the Rust front (apps/k7/backend-rs) linked into the app as
// k7-embed, answering the studio's own port by itself, with no node behind it. Port, environment and folders are
// the desktop's, from apps/k7/backend-rs/standalone.json. One front per PROCESS, like the supervisor and the engine:
// it is started once, and a start that failed is the one thing a retry redoes. See engine/k7-runtime/docs/ios.md.

import Foundation
import os

final class BackendHost {
    static let shared = BackendHost()

    /// standalone.json's table, read from the bundle: its port is the desktop app's.
    private static let table: [String: Any] = {
        let url = Bundle.main.resourceURL!.appendingPathComponent("backend/standalone.json")
        guard let data = try? Data(contentsOf: url),
              let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return [:] }
        return json
    }()
    // A test run names its own port, as Android's e2e build does, so it never meets the tray's on this loopback.
    static let port = Int(ProcessInfo.processInfo.environment["K7_STUDIO_PORT"] ?? "") ?? table["port"] as? Int ?? 18789
    static var origin: String { "http://127.0.0.1:\(port)" }
    /// What every part that derives a loopback address is told — the front, the supervisor, the pages.
    static var portsSpec: String { "k7=\(port)" }
    /// k7's first page: the page asks `/auth/me` for its token itself (apps/k7/backend-rs/docs/sessions.md).
    static var home: String { "\(origin)/" }

    /// What the shell page shows: `starting` with a step (`launching`), `ready`, or `failed` with why.
    struct Status {
        let state: String
        var step: String?
        var message: String?
    }

    private let log = Logger(subsystem: "se.frediver.k7.daw", category: "k7-backend")
    private let lock = NSLock()
    private var started = false
    private var running = false
    private var stopping = false
    private var ready = false
    // Why the front is given up on, until a view asks again; nil while it runs or starts.
    private var failure: String?
    private var waiting: [(then: () -> Void, failed: (String) -> Void)] = []
    private var status = Status(state: "starting")
    private var watchers: [UUID: (Status) -> Void] = [:]

    private init() {}

    func current() -> Status {
        lock.lock()
        defer { lock.unlock() }
        return status
    }

    /// Every status change from now on, called on whichever thread made it; the token unwatches.
    func watch(_ watcher: @escaping (Status) -> Void) -> UUID {
        let token = UUID()
        lock.lock()
        watchers[token] = watcher
        lock.unlock()
        return token
    }

    func unwatch(_ token: UUID) {
        lock.lock()
        watchers[token] = nil
        lock.unlock()
    }

    private func report(_ next: Status) {
        lock.lock()
        status = next
        let to = Array(watchers.values)
        lock.unlock()
        to.forEach { $0(next) }
    }

    /// Start the front, once per process.
    func start() {
        lock.lock()
        if started {
            lock.unlock()
            return
        }
        started = true
        lock.unlock()
        attempt()
    }

    /// `then` once the front has answered — at once if it has — or `failed` once it is given up on; either on any thread.
    func whenReady(_ then: @escaping () -> Void, failed: @escaping (String) -> Void) {
        lock.lock()
        let now = ready
        if !now { waiting.append((then, failed)) }
        lock.unlock()
        // A view asking after the start was given up on is the user's retry.
        if now { then() } else { retry() }
    }

    /// The user's retry — a view asking again, or the shell page's Retry: a start given up on is made again.
    func retry() {
        lock.lock()
        let again = failure != nil && started
        if again { failure = nil }
        lock.unlock()
        if again { attempt() }
    }

    /// The front stopped and waited for, at most `timeout` seconds.
    func stop(timeout: TimeInterval) {
        lock.lock()
        stopping = true
        let live = running
        lock.unlock()
        guard live else { return }
        let done = DispatchSemaphore(value: 0)
        Thread {
            _ = k7_front_stop()
            done.signal()
        }.start()
        _ = done.wait(timeout: .now() + timeout)
    }

    private func attempt() {
        Thread { [self] in
            report(Status(state: "starting", step: "launching"))
            let code = launch()
            guard code > 0 else {
                fail("k7's front did not start (code \(code))")
                return
            }
            do {
                try waitUp()
            } catch {
                log.error("front did not answer: \(error.localizedDescription)")
                fail(error.localizedDescription)
                return
            }
            log.info("front up on \(Self.origin, privacy: .public)")
            report(Status(state: "ready"))
            lock.lock()
            ready = true
            let run = waiting
            waiting.removeAll()
            lock.unlock()
            run.forEach { $0.then() }
        }.start()
    }

    /// Given up on: every view waiting is told why, and the next one to ask starts it again.
    private func fail(_ why: String) {
        lock.lock()
        if stopping {
            lock.unlock()
            return
        }
        failure = why
        ready = false
        let told = waiting
        waiting.removeAll()
        lock.unlock()
        report(Status(state: "failed", message: why))
        told.forEach { $0.failed(why) }
    }

    private var root: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return base.appendingPathComponent("standalone", isDirectory: true)
    }

    /// The workspaces and médiathèque, which the engine must be allowed to open (engine/k7-runtime/docs/ios.md).
    static var engineRoots: [URL] {
        let dirs = table["dirs"] as? [String: String] ?? [:]
        let roots = ["STUDIO_FILES_ROOT", "STUDIO_MEDIATEK"].compactMap { dirs[$0] }.map { shared.root.appendingPathComponent($0) }
        return roots
    }

    private var bundle: URL { Bundle.main.resourceURL!.appendingPathComponent("backend", isDirectory: true) }

    /// The front's settings: the table's environment and folders and every switch on `front`.
    private func environment() -> [String: String] {
        var env: [String: String] = [
            "STUDIO_PORTS": Self.portsSpec,
            "K7_ENGINE_CORE": "front",
            "K7_FILES": "front",
            "K7_FRONTEND_DIST": bundle.appendingPathComponent("dist").path,
            "K7_CODE_ROOT": bundle.appendingPathComponent("code").path,
            "K7_FRONT_CACHE": root.appendingPathComponent("front-cache").path,
        ]
        // The desktop's table: the local user on loopback.
        for (key, value) in Self.table["env"] as? [String: String] ?? [:] { env[key] = value }
        for (key, value) in Self.table["dirs"] as? [String: String] ?? [:] {
            env[key] = root.appendingPathComponent(value).path
        }
        // The hub copies the library from the bundle the first time it has no folder for it.
        if let library = Self.table["library"] as? String {
            env["STUDIO_LIBRARY_SEED"] = "k7-core=" + bundle.appendingPathComponent(library).path
        }
        if FileManager.default.fileExists(atPath: TrustedCas.file.path) {
            env["K7_EXTRA_CA_CERTS"] = TrustedCas.file.path
        }
        return env
    }

    /// The front's port, or the negative code it refused with.
    private func launch() -> Int32 {
        lock.lock()
        running = true
        lock.unlock()
        try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        // iOS starts an app at 256 descriptors, which a library's first capture alone runs out of.
        var limit = rlimit()
        if getrlimit(RLIMIT_NOFILE, &limit) == 0 {
            limit.rlim_cur = min(Self.openFiles, limit.rlim_max)
            setrlimit(RLIMIT_NOFILE, &limit)
        }
        RoamStore.install()
        let config: [String: Any] = ["listen": "127.0.0.1:\(Self.port)", "env": environment()]
        guard let data = try? JSONSerialization.data(withJSONObject: config),
              let text = String(data: data, encoding: .utf8) else { return -2 }
        let code = text.withCString { k7_front_start($0) }
        // One that runs already, since a start given up on may have bound before the wait failed.
        let bound = code == -3 ? Int32(Self.port) : code
        if bound <= 0 {
            lock.lock()
            running = false
            lock.unlock()
        }
        return bound
    }

    /// Until the front answers its registry's hello — what the supervisor asks before it connects.
    private func waitUp() throws {
        let end = Date().addingTimeInterval(Self.readyTimeout)
        let url = URL(string: "\(Self.origin)/api/registry/hello")!
        while Date() < end {
            var request = URLRequest(url: url)
            request.timeoutInterval = 0.5
            let answered = DispatchSemaphore(value: 0)
            var ok = false
            URLSession.shared.dataTask(with: request) { _, response, _ in
                ok = (response as? HTTPURLResponse)?.statusCode == 200
                answered.signal()
            }.resume()
            answered.wait()
            if ok { return }
            Thread.sleep(forTimeInterval: 0.2)
        }
        throw NSError(domain: "k7", code: 1, userInfo: [
            NSLocalizedDescriptionKey: "no answer on \(Self.origin) within \(Int(Self.readyTimeout)) s",
        ])
    }

    private static let readyTimeout: TimeInterval = 60
    private static let openFiles: rlim_t = 10240
}
