// HomeStyle.swift — the native home's look, from the design canvas's iOS and iPadOS boards: black ground, grouped cards,
// a violet accent, and Liquid Glass on iOS 26 with a material in its place before. The tabs are the web home's five.
// See docs/native-ui-plan.md.

import SwiftUI

enum HomeStyle {
    static let ground = Color.black
    static let card = Color(red: 28 / 255, green: 28 / 255, blue: 30 / 255)
    static let accent = Color(red: 185 / 255, green: 166 / 255, blue: 1)
    static let accentLight = Color(red: 214 / 255, green: 200 / 255, blue: 1)
    static let primaryTop = Color(red: 132 / 255, green: 103 / 255, blue: 238 / 255)
    static let primary = Color(red: 111 / 255, green: 79 / 255, blue: 224 / 255)
    static let badgeText = Color(red: 75 / 255, green: 47 / 255, blue: 192 / 255)
    static let secondary = Color(red: 235 / 255, green: 235 / 255, blue: 245 / 255).opacity(0.6)
    static let tertiary = Color(red: 235 / 255, green: 235 / 255, blue: 245 / 255).opacity(0.3)
    static let separator = Color(red: 84 / 255, green: 84 / 255, blue: 88 / 255).opacity(0.65)
    static let videoTile = Color(red: 58 / 255, green: 111 / 255, blue: 216 / 255)
    static let warn = Color(red: 1, green: 0.8, blue: 0.4)
    static let error = Color(red: 1, green: 0.45, blue: 0.45)

    /// The SF Symbol an archetype's row and picker draw (the web draws lucide's equivalents).
    static func symbol(_ shape: String?) -> String {
        switch shape {
        case "mix": return "opticaldisc"
        case "video": return "film"
        case "live": return "slider.vertical.3"
        case "live-video": return "web.camera"
        case "arrangement": return "waveform"
        default: return "arrow.turn.down.right"
        }
    }

    static func tile(_ shape: String?) -> Color {
        shape == "video" || shape == "live-video" ? videoTile : primary
    }
}

/// The home's five tabs, as the web home's floating bar has them; only Projects is native so far.
enum HomeTab: String, CaseIterable, Identifiable {
    case projects, surfaces, tasks, library, settings
    var id: String { rawValue }

    var key: String { "home.\(rawValue)" }

    var symbol: String {
        switch self {
        case .projects: return "folder"
        case .surfaces: return "display"
        case .tasks: return "checklist"
        case .library: return "books.vertical"
        case .settings: return "gearshape"
        }
    }

    /// The web page a tab opens until it is native; nil for the native one.
    var webRoute: String? {
        switch self {
        case .projects: return nil
        case .surfaces: return HomeRules.surfacesRoute
        case .tasks: return HomeRules.tasksRoute
        case .library: return HomeRules.librariesRoute
        case .settings: return HomeRules.settingsRoute
        }
    }
}

extension View {
    /// Liquid Glass in `shape` where the system has it, a thin material before iOS 26.
    @ViewBuilder func glass<S: Shape>(_ shape: S) -> some View {
        if #available(iOS 26.0, *) {
            glassEffect(.regular, in: shape)
        } else {
            background(.ultraThinMaterial, in: shape)
                .overlay(shape.stroke(Color.white.opacity(0.18), lineWidth: 1))
        }
    }
}

/// A filled violet pill with a count badge: Pull and Save on a workspace's header.
struct CountPill: View {
    let symbol: String
    let label: String
    let count: Int?

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: symbol).font(.system(size: 14, weight: .semibold))
            Text(label).font(.system(size: 15, weight: .semibold))
            if let count {
                Text("\(count)")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(HomeStyle.badgeText)
                    .padding(.horizontal, 5)
                    .frame(minWidth: 18, minHeight: 18)
                    .background(Color.white, in: Capsule())
            }
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 14)
        .frame(height: 34)
        .background(LinearGradient(colors: [HomeStyle.primaryTop, HomeStyle.primary], startPoint: .top, endPoint: .bottom),
                    in: Capsule())
        .frame(minHeight: 44)
    }
}

/// The outlined pill: Setup sync.
struct QuietPill: View {
    let symbol: String
    let label: String

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: symbol).font(.system(size: 14, weight: .semibold))
            Text(label).font(.system(size: 15, weight: .semibold))
        }
        .foregroundStyle(HomeStyle.accentLight)
        .padding(.horizontal, 14)
        .frame(height: 34)
        .background(Color.white.opacity(0.10), in: Capsule())
        .overlay(Capsule().stroke(Color.white.opacity(0.18), lineWidth: 1))
        .frame(minHeight: 44)
    }
}
