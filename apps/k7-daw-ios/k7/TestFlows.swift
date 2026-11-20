// TestFlows.swift — the native flows a functional run cannot reach from the host's WebKit, started by `notifyutil -p
// se.frediver.k7.daw.test.flow.<name>` (debug builds, K7_TEST_HOOKS=1): a download, the CA import, an https load, the microphone, a killed page.
// Each hook runs the app's own code path and ends in one `[host] flow:` line; arguments come as files in the app's tmp folder.

import Foundation
import UIKit
import WebKit

#if DEBUG
enum TestFlows {
    static func register() {
        TestHooks.observe("flow.download") { download() }
        TestHooks.observe("flow.import-ca") { importCa() }
        TestHooks.observe("flow.forget-ca") {
            TrustedCas.forget()
            hostLog("flow: forgot the imported CAs, \(TrustedCas.count()) left")
        }
        TestHooks.observe("flow.open") { open() }
        TestHooks.observe("flow.mic") { mic() }
        TestHooks.observe("flow.kill") { kill() }
    }

    /// The text of the argument file `name` the run left in the app's tmp folder.
    private static func argument(_ name: String) -> String? {
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("k7-flow-\(name)")
        return (try? String(contentsOf: file, encoding: .utf8))?.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// The k7 view showing the studio, loaded first when it shows something else (a hook may have sent it elsewhere).
    private static func studioView(tries: Int = 8, _ then: @escaping (AppWebView) -> Void) {
        guard let view = TestHooks.find(MainViewController.current?.view) else { return hostLog("flow: no k7 view") }
        let home = BackendHost.origin
        if !view.isLoading, view.url?.absoluteString.hasPrefix(home) == true { return then(view) }
        guard tries > 0 else { return hostLog("flow: the k7 view never showed the studio") }
        if !view.isLoading || view.url == nil { view.load(URLRequest(url: URL(string: home + "/")!)) }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { studioView(tries: tries - 1, then) }
    }

    /// A click on an `<a download>` of a blob, which WebKit hands to a WKDownload: Downloads.swift's delegate saves it.
    private static func download() {
        let name = argument("download") ?? "flow-take.wav"
        studioView { view in
            let js = """
            const a = document.createElement('a');
            a.href = URL.createObjectURL(new Blob(['k7 flow download'], { type: 'audio/wav' }));
            a.download = '\(name)';
            document.body.appendChild(a);
            a.click();
            """
            view.evaluateJavaScript(js) { _, error in
                hostLog("flow: download clicked\(error.map { ": \($0.localizedDescription)" } ?? "")")
            }
        }
    }

    /// The import the settings page offers, from a file the run wrote.
    private static func importCa() {
        guard let path = argument("ca") else { return hostLog("flow: no CA file named") }
        do {
            let names = try TrustedCas.importFile(URL(fileURLWithPath: path))
            hostLog("flow: imported CA \(names.joined(separator: ", ")), \(TrustedCas.count()) trusted")
        } catch {
            hostLog("flow: CA import failed: \(error.localizedDescription)")
        }
    }

    /// An address loaded in the k7 view, whose challenge the views' own delegate answers; then where the view stands.
    private static func open() {
        guard let target = argument("url"), let url = URL(string: target),
              let view = TestHooks.find(MainViewController.current?.view) else { return hostLog("flow: no address or no view") }
        view.load(URLRequest(url: url))
        DispatchQueue.main.asyncAfter(deadline: .now() + 6) {
            view.evaluateJavaScript("document.body ? document.body.innerText.slice(0, 40) : ''") { text, _ in
                let shown = view.url?.absoluteString == target
                hostLog("flow: open \(target) \(shown ? "shown" : "not shown") body=\(text as? String ?? "")")
            }
        }
    }

    /// The page's own getUserMedia, which reaches MicAccess through the view's delegate; then the dialog it raised, answered Not now.
    private static func mic() {
        studioView { view in
            let js = """
            try {
              const s = await navigator.mediaDevices.getUserMedia({ audio: true });
              s.getTracks().forEach((t) => t.stop());
              return 'granted';
            } catch (e) { return 'refused ' + e.name; }
            """
            view.callAsyncJavaScript(js, in: nil, in: .page) { result in
                var said = "no answer"
                if case .success(let value) = result { said = "\(value)" }
                hostLog("flow: microphone getUserMedia \(said)")
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) { answerDialog() }
        }
    }

    private static func answerDialog() {
        guard let alert = Dialogs.top() as? UIAlertController else { return hostLog("flow: microphone no dialog") }
        let buttons = alert.actions.compactMap(\.title).joined(separator: "|")
        hostLog("flow: microphone dialog \"\(alert.title ?? "")\" buttons \(buttons)")
        // The cancel button's own handler, so the dialog's bookkeeping ends as a tap would end it.
        if let cancel = alert.actions.first(where: { $0.style == .cancel }),
           let handler = cancel.value(forKey: "handler") {
            typealias Handler = @convention(block) (UIAlertAction) -> Void
            unsafeBitCast(handler as AnyObject, to: Handler.self)(cancel)
        }
        alert.dismiss(animated: false)
    }

    /// The k7 view's content process killed from outside it, as the system does under memory pressure.
    private static func kill() {
        guard let view = TestHooks.find(MainViewController.current?.view) else { return hostLog("flow: no k7 view") }
        guard let pid = (view.value(forKey: "_webProcessIdentifier") as? NSNumber)?.int32Value, pid > 0 else {
            return hostLog("flow: kill: the view has no content process")
        }
        hostLog("flow: kill: content process \(pid)")
        Darwin.kill(pid, SIGKILL)
    }
}
#endif
