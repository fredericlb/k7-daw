// SceneDelegate.swift — the app's one window: the scene iOS connects at launch gets the window and the shell page's
// controller (MainViewController). The process's parts start in AppDelegate, once, before any scene; a scene that
// disconnects and comes back gets a new window over the same backend, supervisor and engine. See apps/k7-daw-engine/k7-runtime/docs/ios.md.

import UIKit

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: scene)
        window.rootViewController = AppDelegate.unitTestHost ? UIViewController() : MainViewController()
        window.makeKeyAndVisible()
        self.window = window
    }
}
