// TestHooks.swift — what a functional run drives from outside the app, only when K7_TEST_HOOKS=1 in a debug build:
// the system's media buttons, which no tool can send to a simulator, through `notifyutil -p <name>`.
// Each hook runs the SAME handler the system calls (MediaPlayback.perform), never a copy.

import Foundation
import UIKit

enum TestHooks {
    private static let prefix = "rocks.k7.daw.test."
    private static let actions = ["play", "pause", "nexttrack", "previoustrack", "stop", "seekbackward", "seekforward", "seekto"]

    static func start() {
        #if DEBUG
        guard ProcessInfo.processInfo.environment["K7_TEST_HOOKS"] == "1" else { return }
        observe("media-arm") { arm() }
        for action in actions {
            observe("remote.\(action)") { remote(action) }
        }
        TestFlows.register()
        hostLog("test hooks on")
        #endif
    }

    #if DEBUG
    static func observe(_ name: String, _ run: @escaping () -> Void) {
        let box = Unmanaged.passRetained(Handler(run)).toOpaque()
        CFNotificationCenterAddObserver(CFNotificationCenterGetDarwinNotifyCenter(), box, { _, observer, _, _, _ in
            guard let observer else { return }
            let handler = Unmanaged<Handler>.fromOpaque(observer).takeUnretainedValue()
            DispatchQueue.main.async { handler.run() }
        }, (prefix + name) as CFString, nil, .deliverImmediately)
    }

    private final class Handler {
        let run: () -> Void
        init(_ run: @escaping () -> Void) { self.run = run }
    }

    /// A page that plays, as the studio's own does: a title, the playing state and a handler per action that answers back.
    private static func arm() {
        guard let view = find(MainViewController.current?.view) else { return hostLog("test hook: no k7 view to arm") }
        // A view the shell has not shown yet has no page: the studio's own, so its origin passes the bridge's check.
        if view.url == nil, let url = URL(string: "http://127.0.0.1:\(BackendHost.port)/") {
            view.load(URLRequest(url: url))
            DispatchQueue.main.asyncAfter(deadline: .now() + 4) { arm() }
            return
        }
        let list = ["play", "pause", "nexttrack", "stop", "seekto", "seekbackward", "seekforward"]
        // The page's side: handlers that note what reached them (the bridge's own channel answers only to a shown app).
        let js = """
        (() => {
          window.__k7Acks = [];
          const ms = navigator.mediaSession;
          for (const a of \(list)) ms.setActionHandler(a, () => window.__k7Acks.push(a));
        })()
        """
        view.evaluateJavaScript(js) { _, error in
            hostLog("test hook: armed the k7 view\(error.map { ": \($0.localizedDescription)" } ?? "") url=\(view.url?.absoluteString ?? "-")")
        }
        // What the bridge hands MediaPlayback when the page's own session says it plays.
        MediaPlayback.update(["t": "media", "title": "k7 test song", "artist": "k7", "album": "test", "state": "playing",
                              "duration": 120.0, "position": 10.0, "rate": 1.0, "actions": list + ["previoustrack"]], from: view)
    }

    /// The same handler the system calls, then what the page says reached it.
    private static func remote(_ action: String) {
        let status = MediaPlayback.perform(action, seekTime: action == "seekto" ? 30 : nil)
        guard status == .success, let view = find(MainViewController.current?.view) else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
            view.evaluateJavaScript("window.__k7Acks.join(',')") { result, _ in
                hostLog("test hook: the page handled [\(result as? String ?? "")] after \(action)")
            }
        }
    }

    static func find(_ root: UIView?) -> AppWebView? {
        guard let root else { return nil }
        if let web = root as? AppWebView { return web }
        for child in root.subviews { if let found = find(child) { return found } }
        return nil
    }
    #endif
}
