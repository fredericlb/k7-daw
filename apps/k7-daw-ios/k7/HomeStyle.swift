// HomeStyle.swift — the native home's look, from the design canvas's iOS and iPadOS boards: black ground (a light one too), grouped cards,
// a violet accent, and Liquid Glass on iOS 26 with a material in its place before. The tabs are the web home's five.
// See docs/native-ui-plan.md.

import SwiftUI
import UIKit

enum HomeStyle {
    /// A colour for each look: the mockups' dark, and its light counterpart.
    static func pair(_ dark: UIColor, _ light: UIColor) -> Color {
        Color(UIColor { $0.userInterfaceStyle == .light ? light : dark })
    }

    private static func rgb(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1) -> UIColor {
        UIColor(red: r / 255, green: g / 255, blue: b / 255, alpha: a)
    }

    static let ground = pair(.black, rgb(242, 242, 247))
    static let card = pair(rgb(28, 28, 30), .white)
    /// The text drawn on the ground and on cards.
    static let ink = pair(.white, .black)
    static let accent = pair(rgb(185, 166, 255), rgb(111, 79, 224))
    static let accentLight = pair(rgb(214, 200, 255), rgb(75, 47, 192))
    static let primaryTop = Color(red: 132 / 255, green: 103 / 255, blue: 238 / 255)
    static let primary = Color(red: 111 / 255, green: 79 / 255, blue: 224 / 255)
    static let badgeText = Color(red: 75 / 255, green: 47 / 255, blue: 192 / 255)
    static let secondary = pair(rgb(235, 235, 245, 0.6), rgb(60, 60, 67, 0.6))
    static let tertiary = pair(rgb(235, 235, 245, 0.3), rgb(60, 60, 67, 0.3))
    static let separator = pair(rgb(84, 84, 88, 0.65), rgb(60, 60, 67, 0.29))
    /// A field's or a quiet pill's fill, and the rim drawn round glass.
    static let fill = pair(UIColor.white.withAlphaComponent(0.10), UIColor.black.withAlphaComponent(0.06))
    static let rim = pair(UIColor.white.withAlphaComponent(0.18), UIColor.black.withAlphaComponent(0.08))
    /// An unselected tab's label.
    static let tabInk = pair(UIColor.white.withAlphaComponent(0.86), UIColor.black.withAlphaComponent(0.78))
    static let videoTile = Color(red: 58 / 255, green: 111 / 255, blue: 216 / 255)
    static let warn = pair(rgb(255, 204, 102), rgb(178, 106, 0))
    static let error = pair(rgb(255, 115, 115), rgb(215, 0, 21))

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

/// The home's five tabs, as the web home's floating bar has them; Projects and Settings are native so far.
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
        case .settings: return nil
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
                .overlay(shape.stroke(HomeStyle.rim, lineWidth: 1))
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
        .background(HomeStyle.fill, in: Capsule())
        .overlay(Capsule().stroke(HomeStyle.rim, lineWidth: 1))
        .frame(minHeight: 44)
    }
}
