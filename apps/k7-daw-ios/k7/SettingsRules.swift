// SettingsRules.swift — what the native Settings decides, pure: its tabs, the web pages it still hands over to, the
// engine's status line, the instruments' rows and actions, a model download's line, a commit author's source.
// Each mirrors the web's named function (apps/k7-daw-frontend/src/settings/, src/git/), so both say the same.
// See docs/native-ui-plan.md.

import Foundation

/// The web settings page's five tabs, in its order.
enum SettingsTab: String, CaseIterable, Identifiable, Hashable {
    case general, devices, services, downloads, about
    var id: String { rawValue }

    var key: String {
        switch self {
        case .general: return "settings.tabGeneral"
        case .devices: return "settings.tabDevices"
        case .services: return "settings.tabServices"
        case .downloads: return "settings.tabDownloads"
        case .about: return "settings.tabAbout"
        }
    }

    var symbol: String {
        switch self {
        case .general: return "gearshape"
        case .devices: return "iphone.gen3.radiowaves.left.and.right"
        case .services: return "person.crop.circle.badge.checkmark"
        case .downloads: return "arrow.down.circle"
        case .about: return "info.circle"
        }
    }

    /// Whether the tab is a native page: Services (the sign-ins, phase 3) and Downloads (the web page's own) stay web.
    var native: Bool { self != .services && self != .downloads }

    /// The tab's web page, the `web` item keeping it the web's (HomeRules.nativePage).
    var webRoute: String { "/settings?tab=\(rawValue)&web" }
}

/// A page of Settings pushed from its list.
enum SettingsPage: Hashable {
    case tab(SettingsTab)
    case engine
}

enum SettingsRules {
    /// Each language named in its own, as the web's `LanguageSection` does.
    static func languageName(_ locale: String) -> String {
        ["en": "English", "fr": "Français"][locale] ?? locale
    }

    /// The engine's line (the web's `engineLine` in AndroidEngineSection.tsx).
    static func engineLine(state: [String: Any]?, exit: Int?, lan: String?, l: L10n) -> String {
        if let exit { return l.t("android.engineStopped", ["code": String(exit)]) }
        guard let state, let status = state["status"] as? String else { return l.t("android.starting") }
        switch status {
        case "running":
            let port = (state["enginePort"] as? NSNumber)?.intValue ?? 0
            return l.t("android.engineRunning", ["address": "ws://\(lan ?? "127.0.0.1"):\(port)"])
        case "error":
            return l.t("android.engineError", ["message": state["error"] as? String ?? ""])
        default:
            return l.t("android.engineStatus", ["status": status])
        }
    }

    /// One instrument as the Downloads section draws it (the web's `InstrumentRow`).
    struct Instrument: Equatable, Identifiable {
        let name: String
        let label: String
        let bytes: Double
        let installed: Bool
        let wanted: Bool
        let phase: String?
        let received: Double
        let error: String?
        var id: String { name }
    }

    /// Every instrument the engine can play, each with its download's phase (the web's `instrumentRows`).
    static func instruments(_ state: [String: Any]?) -> [Instrument] {
        let downloads = state?["downloads"] as? [[String: Any]] ?? []
        return (state?["instruments"] as? [[String: Any]] ?? []).compactMap { i in
            guard let name = i["name"] as? String else { return nil }
            let d = downloads.first { $0["name"] as? String == name }
            return Instrument(name: name, label: i["label"] as? String ?? name,
                              bytes: (i["bytes"] as? NSNumber)?.doubleValue ?? 0,
                              installed: i["installed"] as? Bool ?? false,
                              wanted: i["wanted"] as? Bool ?? false,
                              phase: d?["phase"] as? String,
                              received: (d?["received"] as? NSNumber)?.doubleValue ?? 0,
                              error: d?["error"] as? String)
        }
    }

    /// The row's one action and its command, or none while a download runs (the web's `instrumentAction`).
    static func action(_ i: Instrument) -> (key: String, cmd: String)? {
        switch i.phase {
        case "downloading", "unpacking": return nil
        case "queued": return ("engine.instrument.cancel", "instruments.remove")
        case "failed": return ("engine.instrument.retry", "instruments.download")
        default:
            return i.installed ? ("engine.instrument.remove", "instruments.remove") : ("engine.instrument.download", "instruments.download")
        }
    }

    /// The line under an instrument (InstrumentsSection.tsx's detail).
    static func instrumentLine(_ i: Instrument, l: L10n) -> String {
        let size = sizeOf(i.bytes)
        switch i.phase {
        case "queued": return l.t("engine.instrument.queued", ["size": size])
        case "unpacking": return l.t("engine.instrument.unpacking", ["size": size])
        case "failed": return l.t("engine.instrument.failed", ["error": i.error ?? ""])
        case "downloading": return "\(sizeOf(i.received)) / \(size)"
        default:
            if i.installed { return l.t("engine.instrument.installed", ["size": size]) }
            return i.wanted ? l.t("engine.instrument.missing", ["size": size]) : size
        }
    }

    /// "748 MB", "1.3 GB" (the web's `sizeOf`).
    static func sizeOf(_ bytes: Double) -> String {
        let mb = bytes / 1e6
        return mb >= 1000 ? String(format: "%.1f GB", mb / 1000) : "\(max(1, Int(mb.rounded()))) MB"
    }

    /// One line for a model download as it stands (the web's `describeDownload`).
    static func download(_ j: ModelJob, l: L10n) -> String {
        if j.done { return j.error ?? l.t("status.modelDownloaded", ["file": j.file]) }
        let mb = 1024.0 * 1024.0
        let got = String(format: "%.0f", j.received / mb)
        guard let total = j.total, total > 0 else { return l.t("status.modelDownloading", ["file": j.file, "got": got]) }
        return l.t("status.modelDownloadingPct", ["file": j.file, "pct": String(Int((j.received / total * 100).rounded(.down))),
                                                   "got": got, "total": String(format: "%.0f", total / mb)])
    }

    /// Where an untyped author field came from (the web's `sourceNote`); a phone is not a Mac.
    static func sourceKey(_ source: String?) -> String? {
        switch source {
        case "github": return "git.identityFromGithub"
        case "forgejo": return "git.identityFromForgejo"
        case "git-config": return "git.identityFromConfig"
        case "system": return "git.identityFromComputer"
        default: return nil
        }
    }

    /// What a field sends: empty while it still shows its suggestion, so the suggestion keeps following its source.
    static func typed(_ value: String, shown: String, source: String?) -> String {
        source != nil && value.trimmingCharacters(in: .whitespaces) == shown ? "" : value
    }

    /// The options a chain picker lists: the stored one first when the library no longer has it.
    static func options(_ chains: [ColdChains.Chain], held: String?) -> [ColdChains.Chain] {
        guard let held, !held.isEmpty, !chains.contains(where: { $0.ref == held }) else { return chains }
        return [ColdChains.Chain(ref: held, instrumental: false, unavailable: nil, downloads: [])] + chains
    }
}
