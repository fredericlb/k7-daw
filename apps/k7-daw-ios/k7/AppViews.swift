// AppViews.swift — k7 in one full-screen WKWebView over the shell page (web/src/App.tsx, AppViews), under a
// status-bar strip painted the colour of k7's own header. It loads from this phone's own backend, never from this
// app: the page is same-origin with its backend (`/api`, `/auth`, cookie sessions). The view is made the first time it
// is shown and then only hidden, so k7 keeps its state while the shell page is on screen.
// The same plugin, methods and events as AppViewsPlugin.kt on Android. See engine/k7-runtime/docs/ios.md.

import Capacitor
import UIKit
import WebKit

/// k7's view, told apart from the shell page's own WKWebView.
final class AppWebView: WKWebView {}

@objc(AppViewsPlugin)
public class AppViewsPlugin: CAPPlugin, CAPBridgedPlugin, WKNavigationDelegate, WKUIDelegate {
    public let identifier = "AppViewsPlugin"
    public let jsName = "AppViews"
    public let pluginMethods: [CAPPluginMethod] = [
        CAPPluginMethod(name: "show", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "hideAll", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "reload", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "backend", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "restartBackend", returnType: CAPPluginReturnPromise),
    ]

    private let frame = UIView()
    // Under the status bar, in k7's top colour.
    private let strip = UIView()
    private var appBridge: StudioBridge!
    private var color: UIColor = .clear
    private var view: AppWebView?
    private var loaded = false
    private var visible = false
    private var localSignInAt = Date.distantPast
    private var backendWatch: UUID?
    private var scripts: [WKUserScript] = []
    // When the view's content process died lately, for the reload cap.
    private var contentLost: [Date] = []
    private static let maxReloads = 2
    private static let reloadWindow: TimeInterval = 60

    override public func load() {
        guard let host = bridge?.viewController?.view else { return }
        frame.isHidden = true
        frame.backgroundColor = UIColor(red: 0.04, green: 0.04, blue: 0.05, alpha: 1)
        frame.translatesAutoresizingMaskIntoConstraints = false
        host.addSubview(frame)
        strip.translatesAutoresizingMaskIntoConstraints = false
        frame.addSubview(strip)
        NSLayoutConstraint.activate([
            frame.topAnchor.constraint(equalTo: host.topAnchor),
            frame.bottomAnchor.constraint(equalTo: host.bottomAnchor),
            frame.leadingAnchor.constraint(equalTo: host.leadingAnchor),
            frame.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            strip.topAnchor.constraint(equalTo: frame.topAnchor),
            strip.leadingAnchor.constraint(equalTo: frame.leadingAnchor),
            strip.trailingAnchor.constraint(equalTo: frame.trailingAnchor),
            strip.bottomAnchor.constraint(equalTo: frame.safeAreaLayoutGuide.topAnchor),
        ])
        scripts = Self.pageScripts()
        appBridge = StudioBridge(
            isK7: { [weak self] in self?.isK7($0) ?? false },
            onColor: { [weak self] color in
                self?.color = color
                if self?.visible == true { self?.paint() }
            },
            onAccount: { [weak self] name in DispatchQueue.main.async { self?.onAccount(name) } }
        )
        backendWatch = BackendHost.shared.watch { [weak self] status in
            DispatchQueue.main.async { self?.notifyListeners("backend", data: Self.statusJs(status)) }
        }
    }

    deinit {
        if let backendWatch { BackendHost.shared.unwatch(backendWatch) }
        appBridge?.destroy()
    }

    /// The scripts the view runs: the channel, the loopback ports, the media session at document start; colour and account once loaded.
    private static func pageScripts() -> [WKUserScript] {
        func asset(_ name: String) -> String {
            guard let url = Bundle.main.url(forResource: name, withExtension: "js", subdirectory: "page-scripts"),
                  let text = try? String(contentsOf: url, encoding: .utf8) else { return "" }
            return text
        }
        // web-protocol's PORTS_GLOBAL, for the phone's own backend only, whose pages derive each service's address from it.
        let ports = "if (location.origin === \(Self.jsString(BackendHost.origin))) window.__STUDIO_PORTS__ = \(Self.jsString(BackendHost.portsSpec));"
        return [
            WKUserScript(source: StudioBridge.channelScript, injectionTime: .atDocumentStart, forMainFrameOnly: true),
            WKUserScript(source: ports, injectionTime: .atDocumentStart, forMainFrameOnly: true),
            WKUserScript(source: asset("media-session"), injectionTime: .atDocumentStart, forMainFrameOnly: true),
            WKUserScript(source: asset("top-color"), injectionTime: .atDocumentEnd, forMainFrameOnly: true),
            WKUserScript(source: asset("account"), injectionTime: .atDocumentEnd, forMainFrameOnly: true),
        ]
    }

    private static func jsString(_ s: String) -> String {
        let data = try? JSONSerialization.data(withJSONObject: [s])
        return data.flatMap { String(data: $0, encoding: .utf8) }.map { String($0.dropFirst().dropLast()) } ?? "\"\""
    }

    @objc func show(_ call: CAPPluginCall) {
        // The phone's own backend may still be starting: wait rather than fail.
        BackendHost.shared.whenReady({ [weak self] in
            DispatchQueue.main.async { self?.showNow(call) }
        }, failed: { [weak self] why in
            DispatchQueue.main.async {
                self?.loadFailed("k7 could not start: \(why)")
                call.resolve()
            }
        })
    }

    /// Where the phone's own backend is: `starting` (with `step`), `ready` or `failed` (with `message`); then `backend` events.
    @objc func backend(_ call: CAPPluginCall) {
        call.resolve(Self.statusJs(BackendHost.shared.current()))
    }

    private static func statusJs(_ s: BackendHost.Status) -> [String: Any] {
        var out: [String: Any] = ["state": s.state]
        if let step = s.step { out["step"] = step }
        if let message = s.message { out["message"] = message }
        return out
    }

    /// The shell page's Retry: BackendHost's user retry; the `backend` events say how it goes.
    @objc func restartBackend(_ call: CAPPluginCall) {
        BackendHost.shared.retry()
        call.resolve()
    }

    private func showNow(_ call: CAPPluginCall) {
        let v = view ?? create()
        // k7 opens as the local user, as the desktop window does.
        if !loaded, let to = URL(string: BackendHost.home) { v.load(URLRequest(url: to)) }
        loaded = true
        v.isHidden = false
        visible = true
        frame.isHidden = false
        frame.superview?.bringSubviewToFront(frame)
        paint()
        call.resolve()
    }

    @objc func hideAll(_ call: CAPPluginCall) {
        DispatchQueue.main.async { [self] in
            frame.isHidden = true
            visible = false
            MainViewController.current?.statusBarDark = false
            call.resolve()
        }
    }

    @objc func reload(_ call: CAPPluginCall) {
        DispatchQueue.main.async { [self] in
            view?.reload()
            call.resolve()
        }
    }

    /// Who k7 says is signed in; nobody means back to the local user.
    private func onAccount(_ name: String?) {
        guard name == nil else { return }
        // Once in a while at most: a local sign-in that keeps failing must not reload forever.
        if Date().timeIntervalSince(localSignInAt) < Self.localSignInGap { return }
        localSignInAt = Date()
        if let to = URL(string: BackendHost.home) { view?.load(URLRequest(url: to)) }
    }

    // Until the page has said, the shell page's own background shows through.
    private func paint() {
        strip.backgroundColor = color
        var white: CGFloat = 0
        if color != .clear, color.getWhite(&white, alpha: nil) {
            MainViewController.current?.statusBarDark = white > 0.5
        }
    }

    private func create() -> AppWebView {
        let config = WKWebViewConfiguration()
        config.allowsInlineMediaPlayback = true
        config.mediaTypesRequiringUserActionForPlayback = []
        // How the page can tell it is inside this app, should it ever need to.
        config.applicationNameForUserAgent = "k7-ios"
        config.preferences.javaScriptCanOpenWindowsAutomatically = true
        let content = config.userContentController
        content.add(appBridge, name: StudioBridge.name)
        scripts.forEach(content.addUserScript)
        let view = AppWebView(frame: .zero, configuration: config)
        view.navigationDelegate = self
        view.uiDelegate = self
        // The edge swipe is iOS's Back: the view's own history first.
        view.allowsBackForwardNavigationGestures = true
        view.isOpaque = false
        view.backgroundColor = .clear
        view.isInspectable = Self.inspectable
        view.translatesAutoresizingMaskIntoConstraints = false
        frame.addSubview(view)
        NSLayoutConstraint.activate([
            view.topAnchor.constraint(equalTo: frame.safeAreaLayoutGuide.topAnchor),
            view.bottomAnchor.constraint(equalTo: frame.safeAreaLayoutGuide.bottomAnchor),
            view.leadingAnchor.constraint(equalTo: frame.safeAreaLayoutGuide.leadingAnchor),
            view.trailingAnchor.constraint(equalTo: frame.safeAreaLayoutGuide.trailingAnchor),
        ])
        self.view = view
        return view
    }

    #if DEBUG
    private static let inspectable = true
    #else
    private static let inspectable = false
    #endif

    /// Whether `url` is k7's own origin.
    private func isK7(_ url: URL) -> Bool {
        guard let home = URL(string: BackendHost.origin) else { return false }
        return home.scheme == url.scheme && home.host == url.host && home.port == url.port
    }

    /// Somewhere this app does not show: Safari, Mail, the phone — and only those, by scheme.
    private func openOutside(_ url: URL) {
        // A page may name any scheme: one handed to another app could launch what it should not.
        guard let scheme = url.scheme?.lowercased(), Self.outsideSchemes.contains(scheme) else { return }
        UIApplication.shared.open(url)
    }

    /// k7 did not load: the shell page takes over, with the tray and a retry, since no page is there to reach settings in.
    private func loadFailed(_ why: String) {
        notifyListeners("loadError", data: ["message": why])
    }

    // MARK: WKNavigationDelegate

    public func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
                        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let url = action.request.url else { return decisionHandler(.allow) }
        if action.shouldPerformDownload { return decisionHandler(.download) }
        let scheme = url.scheme?.lowercased()
        if scheme != "http" && scheme != "https" {
            // about:blank and the page's own blob: and data: frames are the page's business.
            if ["about", "blob", "data"].contains(scheme ?? "") { return decisionHandler(.allow) }
            openOutside(url)
            return decisionHandler(.cancel)
        }
        decisionHandler(.allow)
    }

    public func webView(_ webView: WKWebView, decidePolicyFor response: WKNavigationResponse,
                        decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        decisionHandler(response.canShowMIMEType ? .allow : .download)
    }

    public func webView(_ webView: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload) {
        download.delegate = Downloads.shared
    }

    public func webView(_ webView: WKWebView, navigationResponse: WKNavigationResponse, didBecome download: WKDownload) {
        download.delegate = Downloads.shared
    }

    public func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        failed(webView, error)
    }

    public func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        failed(webView, error)
    }

    // A navigation this file cancelled, or one turned into a download, is no failure of the studio's.
    private func failed(_ webView: WKWebView, _ error: Error) {
        let e = error as NSError
        if e.domain == NSURLErrorDomain && e.code == NSURLErrorCancelled { return }
        if e.domain == "WebKitErrorDomain" && e.code == 102 { return }
        loadFailed(e.localizedDescription)
    }

    // A page killed as it loads would reload forever: past two reloads in a minute, the shell's Retry takes over.
    public func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        let now = Date()
        let recent = contentLost.filter { now.timeIntervalSince($0) < Self.reloadWindow }
        if recent.count >= Self.maxReloads {
            hostLog("content process lost a third time in a minute: no reload, the shell's Retry takes over")
            contentLost = []
            return loadFailed("the page stopped again as it loaded")
        }
        contentLost = recent + [now]
        hostLog("content process lost: reload \(recent.count + 1) of \(Self.maxReloads)")
        webView.reload()
    }

    // A backend behind the user's own CA: the view knows only iOS's roots, so the CAs the tray was given are asked too.
    public func webView(_ webView: WKWebView, didReceive challenge: URLAuthenticationChallenge,
                        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        guard challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust,
              let trust = challenge.protectionSpace.serverTrust else { return completionHandler(.performDefaultHandling, nil) }
        if SecTrustEvaluateWithError(trust, nil) { return completionHandler(.performDefaultHandling, nil) }
        let host = challenge.protectionSpace.host
        if TrustedCas.vouch(trust, host: host) {
            hostLog("trust: \(host) vouched for by an imported CA")
            return completionHandler(.useCredential, URLCredential(trust: trust))
        }
        hostLog("trust: \(host) refused")
        completionHandler(.cancelAuthenticationChallenge, nil)
        // k7's own certificate, not some embed's: nothing of the app can load.
        if URL(string: BackendHost.origin)?.host == host { loadFailed("k7's certificate is not trusted") }
    }

    // MARK: WKUIDelegate

    // A new window is never opened: where it was headed goes outside instead.
    public func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
                        for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        if let url = action.request.url, url.absoluteString != "about:blank" { openOutside(url) }
        return nil
    }

    // The microphone, to k7's own page only, once iOS grants it, asked here on first need (MicAccess.swift).
    public func webView(_ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin,
                        initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType,
                        decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        var parts = URLComponents()
        parts.scheme = origin.protocol
        parts.host = origin.host
        if origin.port != 0 { parts.port = origin.port }
        guard type == .microphone, let url = parts.url, isK7(url) else {
            hostLog("microphone: \(type == .microphone ? "not k7's origin (\(parts.url?.absoluteString ?? "-"))" : "not a microphone"), denied")
            return decisionHandler(.deny)
        }
        MicAccess.request { [weak self] answer in
            decisionHandler(answer == .granted ? .grant : .deny)
            // Blocked for good: the page hears why its getUserMedia failed, beside the native dialog.
            if answer == .blocked {
                self?.view?.evaluateJavaScript("dispatchEvent(new CustomEvent('k7:mic-denied'))")
            }
        }
    }

    public func webView(_ webView: WKWebView, runJavaScriptAlertPanelWithMessage message: String,
                        initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping () -> Void) {
        let alert = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default) { _ in completionHandler() })
        guard let top = Dialogs.top() else { return completionHandler() }
        top.present(alert, animated: true)
    }

    public func webView(_ webView: WKWebView, runJavaScriptConfirmPanelWithMessage message: String,
                        initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping (Bool) -> Void) {
        let alert = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in completionHandler(false) })
        alert.addAction(UIAlertAction(title: "OK", style: .default) { _ in completionHandler(true) })
        guard let top = Dialogs.top() else { return completionHandler(false) }
        top.present(alert, animated: true)
    }

    public func webView(_ webView: WKWebView, runJavaScriptTextInputPanelWithPrompt prompt: String, defaultText: String?,
                        initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping (String?) -> Void) {
        let alert = UIAlertController(title: nil, message: prompt, preferredStyle: .alert)
        alert.addTextField { $0.text = defaultText }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in completionHandler(nil) })
        alert.addAction(UIAlertAction(title: "OK", style: .default) { _ in completionHandler(alert.textFields?.first?.text) })
        guard let top = Dialogs.top() else { return completionHandler(nil) }
        top.present(alert, animated: true)
    }

    private static let localSignInGap: TimeInterval = 5
    // What openOutside hands to another app; anything else is dropped.
    private static let outsideSchemes: Set<String> = ["http", "https", "mailto", "tel"]
}
