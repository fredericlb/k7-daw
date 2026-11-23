// L10n.swift — the native pages' words: the web app's own catalogues (apps/k7-daw-frontend/messages/<locale>.json),
// bundled as they are and read at runtime, with `{name}` holes filled and `.one`/`.other` keys picked by count.
// What a reader gets wrong: the language is k7's own setting (the web page's), not the system's. See docs/native-ui-plan.md.

import Foundation
import SwiftUI

final class L10n: ObservableObject {
    static let shared = L10n()
    static let locales = ["en", "fr"]

    @Published private(set) var locale: String
    private var catalogues: [String: [String: String]] = [:]

    init(bundle: Bundle = .main, locale: String? = nil) {
        for l in Self.locales {
            guard let url = bundle.url(forResource: l, withExtension: "json", subdirectory: "messages"),
                  let data = try? Data(contentsOf: url),
                  let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { continue }
            catalogues[l] = json.compactMapValues { $0 as? String }
        }
        self.locale = locale ?? Self.systemLocale()
    }

    /// The system's language when it is one k7 speaks, else k7's base language; the page's own choice replaces it once read.
    static func systemLocale() -> String {
        let preferred = Locale.preferredLanguages.first.map { String($0.prefix(2)) } ?? "en"
        return locales.contains(preferred) ? preferred : "en"
    }

    /// Follow the language the web page chose (its Settings), when it names one k7 speaks.
    func use(_ chosen: String?) {
        guard let chosen, Self.locales.contains(chosen), chosen != locale else { return }
        locale = chosen
    }

    /// Whether a key is in both catalogues, for the test that every key a native page asks for exists.
    func has(_ key: String) -> Bool {
        Self.locales.allSatisfy { catalogues[$0]?[key] != nil }
    }

    /// The message `key` in the current language with its holes filled; English, then the key itself, when it is missing.
    func t(_ key: String, _ args: [String: String] = [:]) -> String {
        let text = catalogues[locale]?[key] ?? catalogues["en"]?[key] ?? key
        return args.reduce(text) { $0.replacingOccurrences(of: "{\($1.key)}", with: $1.value) }
    }

    /// `<key>.one` or `<key>.other` by `n`, with `{n}` filled, as the web's plural pairs are written.
    func plural(_ key: String, _ n: Int, _ args: [String: String] = [:]) -> String {
        var all = args
        all["n"] = String(n)
        return t(n == 1 ? "\(key).one" : "\(key).other", all)
    }

    /// The locale for dates and relative times.
    var formatLocale: Locale { Locale(identifier: locale) }
}

/// The shared catalogue, for a view's one-line lookups.
func tr(_ key: String, _ args: [String: String] = [:]) -> String {
    L10n.shared.t(key, args)
}
