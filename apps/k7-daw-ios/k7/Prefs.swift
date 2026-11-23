// Prefs.swift — k7's language and theme as the account's preferences hold them (`locale`, `theme` in
// `/api/preferences`), which the web page reads too: read at each return home, written by the native Settings.
// What a reader gets wrong: the web draws dark only, so the theme is the native pages' alone. See docs/native-ui-plan.md.

import SwiftUI
import UIKit

/// The native pages' look: the mockups' dark, a light one, or the system's.
enum Theme: String, CaseIterable, Identifiable {
    case dark, light, system
    var id: String { rawValue }

    /// The stored value read as a theme; anything else is the mockups' dark.
    static func of(_ stored: Any?) -> Theme {
        (stored as? String).flatMap(Theme.init(rawValue:)) ?? .dark
    }

    var key: String {
        switch self {
        case .dark: return "settings.themeDark"
        case .light: return "settings.themeLight"
        case .system: return "settings.themeSystem"
        }
    }

    var style: UIUserInterfaceStyle {
        switch self {
        case .dark: return .dark
        case .light: return .light
        case .system: return .unspecified
        }
    }

    var scheme: ColorScheme? {
        switch self {
        case .dark: return .dark
        case .light: return .light
        case .system: return nil
        }
    }
}

@MainActor
final class AppPrefs: ObservableObject {
    static let shared = AppPrefs()

    @Published private(set) var theme: Theme = .dark
    /// Told after the theme changed, for the hosting view's own style and the status bar.
    var onTheme: (Theme) -> Void = { _ in }
    /// Told after the language changed here, so the web page reads the preferences again.
    var onWritten: () -> Void = {}

    /// The stored language when it is one k7 speaks, else nil (the web page's own choice is then the one followed).
    static func locale(_ stored: Any?) -> String? {
        (stored as? String).flatMap { L10n.locales.contains($0) ? $0 : nil }
    }

    /// Read both; true when the preferences named a language.
    @discardableResult
    func load(_ client: K7Client, l10n: L10n = .shared) async -> Bool {
        guard let p = try? await client.prefs() else { return false }
        apply(Theme.of(p["theme"]))
        guard let locale = Self.locale(p["locale"]) else { return false }
        l10n.use(locale)
        return true
    }

    func setTheme(_ next: Theme, client: K7Client) async throws {
        let before = theme
        apply(next)
        do {
            try await client.patchPrefs(["theme": next.rawValue])
        } catch {
            apply(before)
            throw error
        }
    }

    func setLocale(_ next: String, client: K7Client, l10n: L10n = .shared) async throws {
        try await client.patchPrefs(["locale": next])
        l10n.use(next)
        onWritten()
    }

    private func apply(_ next: Theme) {
        guard next != theme else { return }
        theme = next
        onTheme(next)
    }
}
