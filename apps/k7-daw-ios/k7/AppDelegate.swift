// AppDelegate.swift — the app, on an iPhone: starts k7's backend (BackendHost) and the supervisor (which starts the
// engine); its one scene (SceneDelegate) hosts the shell page and, over it, the studio apps in views of their own
// (MainViewController, AppViews). Everything lives in this one process, as long as iOS lets it: Quit stops the supervisor, the backend
// and the engine and ends the process, which is also the only way to restart an engine (EngineBridge).
// See apps/k7-daw-engine/k7-runtime/docs/ios.md.

import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    static let supervisorStop: TimeInterval = 4
    private static let engineStop: TimeInterval = 3
    // Longer than the front's own stop, so its accounts are not ended mid-teardown (apps/k7-daw-backend/docs/embed.md).
    private static let backendStop: TimeInterval = 10

    func application(_ application: UIApplication,
                     didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        // First of all: the engine, the supervisor and the front write to stdio, which only this makes visible.
        StdioLog.start()
        AudioSession.start()
        TestHooks.start()
        // The studio the views open, before the engine that lists itself there (BackendHost.swift).
        BackendHost.shared.start()
        // Held up by the microphone's first prompt only, so a grant opens the engine's inputs without a restart (MicAccess.swift).
        MicAccess.askAtLaunch { SupervisorHost.shared.start() }
        Busy.shared.start()
        // The window is the scene's (SceneDelegate.swift): iOS 27 stops an app that has none.
        return true
    }

    func application(_ application: UIApplication, configurationForConnecting connectingSceneSession: UISceneSession,
                     options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        UISceneConfiguration(name: "Default", sessionRole: connectingSceneSession.role)
    }

    /// The app's Quit — k7's settings, the tray, a microphone grant: the teardown Android's activity runs, then the end of the process.
    static func quit() {
        MediaPlayback.clear()
        Thread {
            // Supervisor FIRST: it deregisters while the engine and the backend still answer, then asks the engine to stop.
            SupervisorHost.shared.stop(timeout: supervisorStop)
            let backend = Thread { BackendHost.shared.stop(timeout: backendStop) }
            backend.start()
            _ = EngineBridge.stop(timeout: engineStop)
            while backend.isExecuting { Thread.sleep(forTimeInterval: 0.1) }
            exit(0)
        }.start()
    }
}
