// AppDelegate.swift — the app, on an iPhone: starts k7's backend (BackendHost) and the supervisor (which starts the
// engine); its one scene (SceneDelegate) hosts the shell page and, over it, the studio apps in views of their own
// (MainViewController, AppViews). Everything lives in this one process, as long as iOS lets it; the app never ends it
// itself (App Store review), so an engine starts again only when the person closes k7 and opens it (EngineBridge).
// See apps/k7-daw-engine/k7-runtime/docs/ios.md.

import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    static let supervisorStop: TimeInterval = 4
    /// Launched by XCTest to host the `k7Tests` bundle (apps/k7-daw-ios/k7Tests).
    static let unitTestHost = ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil

    func application(_ application: UIApplication,
                     didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        // A unit test host starts nothing: the stdio relay would echo XCTest's own output back into itself.
        if Self.unitTestHost { return true }
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
}
