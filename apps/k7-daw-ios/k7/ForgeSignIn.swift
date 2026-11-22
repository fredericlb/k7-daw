// ForgeSignIn.swift — a Forgejo sign-in in iOS's web sheet (ASWebAuthenticationSession), asked by k7's page through
// StudioBridge's `forgeSignIn`. The server sends the sheet back to `rocks.k7.daw://callback`, which closes it by itself;
// its query is forwarded here to the front's own callback, and the page's poll says how it ended.
// What a reader gets wrong: the sheet never loads the loopback page; see apps/k7-daw-engine/k7-runtime/docs/ios.md.

import AuthenticationServices
import UIKit

final class ForgeSignIn: NSObject, ASWebAuthenticationPresentationContextProviding {
    static let shared = ForgeSignIn()
    static let scheme = "rocks.k7.daw"
    /// The front's route the server's answer is forwarded to (apps/k7-daw-backend/docs/files/git.md).
    static let callbackPath = "/api/files/git/forge/callback"

    // The sheet up now: a second sign-in closes the first.
    private var session: ASWebAuthenticationSession?

    typealias Answer = (Result<[String: Any], Error>) -> Void

    /// The front's callback for what the sheet came back with; nil for anything but `rocks.k7.daw://callback`.
    static func forward(_ back: URL, origin: String) -> URL? {
        guard back.scheme?.lowercased() == scheme, back.host == "callback",
              let query = URLComponents(url: back, resolvingAgainstBaseURL: false)?.percentEncodedQuery else { return nil }
        return URL(string: "\(origin)\(callbackPath)?\(query)")
    }

    func open(_ address: String, _ answer: @escaping Answer) {
        guard let url = URL(string: address), ["http", "https"].contains(url.scheme?.lowercased() ?? "") else {
            return answer(.failure(Self.refusal("not a web address")))
        }
        DispatchQueue.main.async { [self] in
            session?.cancel()
            let made = Self.session(url) { [weak self] back, error in
                self?.session = nil
                guard let back else {
                    hostLog("forge sign-in: sheet closed (\(error?.localizedDescription ?? "no answer"))")
                    return answer(.failure(error ?? Self.refusal("closed")))
                }
                Self.deliver(back, answer)
            }
            made.presentationContextProvider = self
            // No cookie shared with Safari, and so no "wants to use … to sign in" alert to tap through.
            made.prefersEphemeralWebBrowserSession = true
            session = made
            if !made.start() {
                session = nil
                answer(.failure(Self.refusal("the sign-in sheet did not open")))
            }
        }
    }

    private static func session(_ url: URL, _ done: @escaping (URL?, Error?) -> Void) -> ASWebAuthenticationSession {
        if #available(iOS 17.4, *) {
            return ASWebAuthenticationSession(url: url, callback: .customScheme(scheme), completionHandler: done)
        }
        return ASWebAuthenticationSession(url: url, callbackURLScheme: scheme, completionHandler: done)
    }

    // The code and state go to the front, which exchanges them; the page hears the end from its own poll.
    private static func deliver(_ back: URL, _ answer: @escaping Answer) {
        guard let target = forward(back, origin: BackendHost.origin) else {
            return answer(.failure(refusal("an unexpected address came back")))
        }
        URLSession.shared.dataTask(with: target) { _, response, error in
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            hostLog("forge sign-in: forwarded, front answered \(status)")
            if let error { return answer(.failure(error)) }
            answer(.success(["status": status]))
        }.resume()
    }

    // The app's own window, even while a system alert holds the key window; a bare anchor is refused as invalid.
    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainViewController.current?.view.window ?? Dialogs.top()?.view.window ?? ASPresentationAnchor()
    }

    private static func refusal(_ why: String) -> Error {
        NSError(domain: "k7", code: 5, userInfo: [NSLocalizedDescriptionKey: why])
    }
}
