// HomeHost.swift — the native home (HomeViews.swift, SettingsViews.swift) over k7's web view, in AppViews' frame, and
// the handover both ways: a project or a page not native yet is opened in the web view, and the web page reaching an
// address the native home stands in for (`/`, `/settings`) brings it back. It also applies the preferences' theme.
// What a reader gets wrong: the web page stays loaded under the native home, so opening a project is k7's in-place `go`.
// See docs/native-ui-plan.md.

import SwiftUI
import UIKit
import WebKit

@MainActor
final class HomeHost {
    private let model: HomeModel
    private let settings: SettingsModel
    private let engine = EngineModel()
    private let client: K7Client
    private let hosting: UIHostingController<HomeRoot>
    private weak var web: WKWebView?
    private var urlWatch: NSKeyValueObservation?
    private var started = false
    /// Whether the native home covers the web view now.
    private(set) var shown = false
    /// Told when the home shows or hides, or its theme changes, for the status bar.
    var onShown: (Bool) -> Void = { _ in }

    /// Whether the status bar's text is dark: the native home is in front and drawn light.
    var darkStatusBar: Bool { shown && hosting.view.traitCollection.userInterfaceStyle == .light }

    init(parent: UIViewController?, frame: UIView) {
        let origin = URL(string: BackendHost.origin)!
        client = K7Client(origin: origin)
        model = HomeModel(client: client)
        settings = SettingsModel(client: client)
        hosting = UIHostingController(rootView: HomeRoot(model: model, settings: settings, engine: engine))
        hosting.view.backgroundColor = UIColor { $0.userInterfaceStyle == .light ? Self.lightGround : .black }
        // The theme set on the host reaches the split view's sidebar too, which the root's colour scheme does not.
        hosting.overrideUserInterfaceStyle = AppPrefs.shared.theme.style
        hosting.view.isHidden = true
        hosting.view.translatesAutoresizingMaskIntoConstraints = false
        parent?.addChild(hosting)
        frame.addSubview(hosting.view)
        NSLayoutConstraint.activate([
            hosting.view.topAnchor.constraint(equalTo: frame.topAnchor),
            hosting.view.bottomAnchor.constraint(equalTo: frame.bottomAnchor),
            hosting.view.leadingAnchor.constraint(equalTo: frame.leadingAnchor),
            hosting.view.trailingAnchor.constraint(equalTo: frame.trailingAnchor),
        ])
        hosting.didMove(toParent: parent)
        model.openWeb = { [weak self] path in self?.openWeb(path) }
        settings.openWeb = { [weak self] path in self?.openWeb(path) }
        settings.engineId = { [weak self] in self?.engine.engineId }
        AppPrefs.shared.onTheme = { [weak self] theme in
            guard let self else { return }
            hosting.overrideUserInterfaceStyle = theme.style
            onShown(shown)
        }
        // The web page reads the preferences again, so its words follow a language chosen here.
        AppPrefs.shared.onWritten = { [weak self] in
            self?.web?.evaluateJavaScript("dispatchEvent(new CustomEvent('k7-prefs-changed'))")
        }
    }

    private static let lightGround = UIColor(red: 242 / 255, green: 242 / 255, blue: 247 / 255, alpha: 1)

    /// The web view the home hands over to; its address says when to come back.
    func attach(_ view: WKWebView) {
        web = view
        urlWatch = view.observe(\.url, options: [.new]) { [weak self] view, _ in
            MainActor.assumeIsolated { self?.addressChanged(view.url) }
        }
    }

    private func addressChanged(_ url: URL?) {
        guard let url, let origin = URL(string: BackendHost.origin),
              let page = HomeRules.nativePage(url, origin: origin) else { return }
        switch page {
        case .projects:
            model.tab = .projects
        case .settings(let path):
            model.tab = .settings
            model.settingsPath = path
        }
        showHome()
    }

    /// The native home in front, its list read again at once and then polled.
    func showHome() {
        hosting.view.superview?.bringSubviewToFront(hosting.view)
        if !shown { hostLog("home: native") }
        shown = true
        hosting.view.isHidden = false
        // The page under the native home is out of sight, so out of VoiceOver's reach too.
        web?.accessibilityElementsHidden = true
        onShown(true)
        readLocale()
        model.start()
    }

    /// `path` on k7's origin in the web view: k7's own `go` when the page is up (it asks about unsaved work), a load otherwise.
    func openWeb(_ path: String) {
        guard let web, let url = URL(string: BackendHost.origin + path) else { return }
        hostLog("home: web \(path)")
        shown = false
        model.stop()
        engine.stop()
        hosting.view.isHidden = true
        web.accessibilityElementsHidden = false
        onShown(false)
        let before = web.url
        guard !web.isLoading, let at = before, Self.isK7(at) else {
            web.load(URLRequest(url: url))
            return
        }
        // The desktop app's own way to ask k7 for a page (appNav.ts), which a load would not do: unsaved work is asked about.
        web.evaluateJavaScript("dispatchEvent(new CustomEvent('k7-desktop-go', { detail: \(Self.jsString(path)) }))")
        // A page that did not hear it (still starting, say) is loaded at the address instead.
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.goGrace) { [weak self, weak web] in
            guard let self, let web, !self.shown, web.url == before else { return }
            web.load(URLRequest(url: url))
        }
    }

    private static let goGrace: TimeInterval = 3

    /// The preferences' language and theme; with no language stored, the web page's own (paraglide's `PARAGLIDE_LOCALE`).
    func readLocale() {
        Task { [weak self] in
            guard let self else { return }
            if await AppPrefs.shared.load(client) { return }
            web?.evaluateJavaScript("localStorage.getItem('PARAGLIDE_LOCALE')") { value, _ in
                MainActor.assumeIsolated { L10n.shared.use(value as? String) }
            }
        }
    }

    private static func isK7(_ url: URL) -> Bool {
        guard let origin = URL(string: BackendHost.origin) else { return false }
        return url.scheme == origin.scheme && url.host == origin.host && url.port == origin.port
    }

    private static func jsString(_ s: String) -> String {
        let data = try? JSONSerialization.data(withJSONObject: [s])
        return data.flatMap { String(data: $0, encoding: .utf8) }.map { String($0.dropFirst().dropLast()) } ?? "\"\""
    }
}
