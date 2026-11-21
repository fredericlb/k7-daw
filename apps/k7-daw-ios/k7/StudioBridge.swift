// StudioBridge.swift — what k7's page in AppViews' view may ask of this app, over one script message handler
// that `window.k7Ios` wraps in the protocol Android's `k7Android` speaks: the page reports its top colour, who
// is signed in, what it plays and what keeps it busy, and reaches the engine
// (apps/k7-daw-frontend/src/settings/androidEngine.ts) and the folder its exports go to (ExportFolders.swift). The handler is offered to every origin, so each message is
// checked for k7's own instead. See apps/k7-daw-engine/k7-runtime/docs/ios.md.

import UIKit
import UniformTypeIdentifiers
import WebKit

final class StudioBridge: NSObject, WKScriptMessageHandler, SupervisorListener, UIDocumentPickerDelegate {
    static let name = "k7Ios"

    /// Whether an origin is k7's own.
    private let isK7: (URL) -> Bool
    private let onColor: (UIColor) -> Void
    /// Who k7 says is signed in, nil for nobody (account.js).
    private let onAccount: (String?) -> Void
    // The k7 page listening now: a reload hands over a new one with its `listen`.
    private weak var k7: WKWebView?
    private var pickedCa: ((URL?) -> Void)?

    /// The document-start script that gives a page the channel, in the shape Android's WebMessageListener object has.
    static let channelScript = """
    (() => {
      if (window.k7Ios || !window.webkit || !window.webkit.messageHandlers.k7Ios) return;
      const listeners = new Set();
      const channel = {
        onmessage: null,
        postMessage: (data) => window.webkit.messageHandlers.k7Ios.postMessage(String(data)),
        addEventListener: (type, listener) => { if (type === "message") listeners.add(listener); },
        removeEventListener: (type, listener) => { if (type === "message") listeners.delete(listener); },
      };
      Object.defineProperty(window, "k7Ios", { value: channel });
      Object.defineProperty(window, "__k7IosDeliver", {
        value: (data) => {
          const e = { data };
          for (const l of listeners) l(e);
          if (channel.onmessage) channel.onmessage(e);
        },
      });
    })();
    """

    init(isK7: @escaping (URL) -> Bool, onColor: @escaping (UIColor) -> Void, onAccount: @escaping (String?) -> Void) {
        self.isK7 = isK7
        self.onColor = onColor
        self.onAccount = onAccount
        super.init()
        SupervisorHost.shared.listen(self)
    }

    func destroy() {
        SupervisorHost.shared.unlisten(self)
    }

    /// One message to a page, as the channel's `message` event; on the main thread.
    static func post(_ msg: [String: Any], to view: WKWebView) {
        guard let data = try? JSONSerialization.data(withJSONObject: msg),
              let literal = try? JSONSerialization.data(withJSONObject: [String(decoding: data, as: UTF8.self)]),
              let array = String(data: literal, encoding: .utf8) else { return }
        // A one-element JSON array is a JS literal of that string: no quoting by hand.
        let js = "window.__k7IosDeliver && window.__k7IosDeliver(\(array)[0])"
        if Thread.isMainThread { view.evaluateJavaScript(js) } else { DispatchQueue.main.async { view.evaluateJavaScript(js) } }
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let view = message.webView as? AppWebView else { return }
        // k7's own origin and nothing else: a page the view wandered off to gets no answer.
        let origin = message.frameInfo.securityOrigin
        var parts = URLComponents()
        parts.scheme = origin.protocol
        parts.host = origin.host
        if origin.port != 0 { parts.port = origin.port }
        guard message.frameInfo.isMainFrame, let url = parts.url, isK7(url),
              let text = message.body as? String, let data = text.data(using: .utf8),
              let msg = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return }
        switch msg["t"] as? String {
        case "color":
            if let hex = msg["value"] as? String, let color = UIColor(hex: hex) { onColor(color) }
        case "account":
            onAccount(msg["name"] as? String)
        case "media":
            MediaPlayback.update(msg, from: view)
        case "busy":
            Busy.shared.report(view: "k7", msg)
        case "listen":
            k7 = view
            push()
        case "call":
            call(view, id: msg["id"] as? Int ?? 0, method: msg["method"] as? String ?? "", arg: msg["arg"] as? [String: Any] ?? [:])
        default:
            break
        }
    }

    func onState(_ frame: [String: Any]) { push() }

    func onEngineExit(_ code: Int) { push() }

    private func push() {
        let known = SupervisorHost.shared.known()
        DispatchQueue.main.async { [self] in
            guard let k7 else { return }
            var snapshot: [String: Any] = ["t": "snapshot"]
            if let state = known.state { snapshot["state"] = state }
            if let exit = known.engineExit { snapshot["engineExit"] = exit }
            Self.post(snapshot, to: k7)
        }
    }

    private func call(_ view: WKWebView, id: Int, method: String, arg: [String: Any]) {
        let answer = { (done: Result<[String: Any], Error>) in
            var out: [String: Any] = ["t": "reply", "id": id]
            switch done {
            case .success(let value):
                out["ok"] = true
                out["value"] = value
            case .failure(let error):
                out["ok"] = false
                out["error"] = error.localizedDescription
            }
            Self.post(out, to: view)
        }
        switch method {
        case "command":
            command(arg["cmd"] as? String ?? "", arg["arg"] as? String ?? "", answer)
        case "lanAddress":
            answer(.success(EngineOps.lanAddress().map { ["address": $0] } ?? [:]))
        case "casInfo":
            answer(.success(["count": TrustedCas.count()]))
        case "importCa":
            pickCa { url in
                guard let url else { return answer(.success(["said": "", "count": TrustedCas.count()])) }
                EngineOps.importCa(url, done: answer)
            }
        case "forgetCas":
            EngineOps.forgetCas(done: answer)
        // The export folder: the system picker, then each file written into it from the page's chunks.
        case "pickExportFolder":
            ExportFolders.shared.pick(answer)
        case "exportBegin":
            ExportFolders.shared.begin(folder: arg["folder"] as? String ?? "", name: arg["name"] as? String ?? "", answer)
        case "exportChunk":
            ExportFolders.shared.chunk(token: arg["token"] as? String ?? "", data: arg["data"] as? String ?? "", answer)
        case "exportEnd":
            ExportFolders.shared.end(token: arg["token"] as? String ?? "", cancel: arg["cancel"] as? String == "1", answer)
        case "shareExport":
            ExportFolders.shared.share(where: arg["where"] as? String ?? "", answer)
        default:
            answer(.failure(Self.refusal("no method \(method)")))
        }
    }

    private func command(_ cmd: String, _ arg: String, _ answer: @escaping (Result<[String: Any], Error>) -> Void) {
        let send = {
            SupervisorHost.shared.command(cmd, arg)
            answer(.success([:]))
        }
        if Self.allowed.contains(cmd) { return send() }
        answer(.failure(Self.refusal("command \(cmd) is not allowed")))
    }

    // The document picker, for a certificate file; nil when the user cancelled.
    private func pickCa(_ then: @escaping (URL?) -> Void) {
        DispatchQueue.main.async { [self] in
            pickedCa = then
            let types = [UTType.x509Certificate, UTType(filenameExtension: "pem"), UTType(filenameExtension: "crt"), UTType.data]
            let picker = UIDocumentPickerViewController(forOpeningContentTypes: types.compactMap { $0 })
            picker.delegate = self
            Dialogs.top()?.present(picker, animated: true)
        }
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        pickedCa?(urls.first)
        pickedCa = nil
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        pickedCa?(nil)
        pickedCa = nil
    }

    private static func refusal(_ why: String) -> Error {
        NSError(domain: "k7", code: 4, userInfo: [NSLocalizedDescriptionKey: why])
    }

    /// The commands the tray's pages may send too (apps/k7-daw-backend/packages/k7-front/src/routes/local/routes.rs).
    private static let allowed: Set<String> = [
        "engine.start", "engine.stop", "engine.restart", "audio.priority", "lan.open",
        "plugins.rescan", "instruments.download", "instruments.remove",
    ]
}

extension UIColor {
    /// `#rrggbb`, which is what top-color.js reports.
    convenience init?(hex: String) {
        guard hex.hasPrefix("#"), hex.count == 7, let v = UInt32(hex.dropFirst(), radix: 16) else { return nil }
        self.init(red: CGFloat(v >> 16 & 0xFF) / 255, green: CGFloat(v >> 8 & 0xFF) / 255, blue: CGFloat(v & 0xFF) / 255, alpha: 1)
    }
}
