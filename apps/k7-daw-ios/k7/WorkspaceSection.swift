// WorkspaceSection.swift — one workspace on the native home: its name over its git line, the pill its git state offers,
// its ⋯ menu, then a card of project rows ending in New project. A row opens the project in the web editor.
// What a reader gets wrong: the pills only open the web page that acts (Sync settings, the web home's cards) for now.
// See docs/native-ui-plan.md.

import SwiftUI

struct WorkspaceSection: View {
    let workspace: Workspace
    let git: GitState?
    /// iPad's wider rows: kind and date in columns rather than under the name.
    let wide: Bool
    let now: Date
    let onOpen: (Project) -> Void
    let onOpenWorkspace: () -> Void
    let onNewProject: () -> Void
    let onWeb: (String) -> Void

    private var examples: Bool { HomeRules.isExamples(workspace) }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            header.padding(.horizontal, 4)
            VStack(spacing: 0) {
                if workspace.projects.isEmpty && !examples {
                    // Nothing in it yet: the workspace opens and the editor asks for the first project.
                    Button(action: onOpenWorkspace) {
                        footerRow(symbol: "folder.badge.plus", label: tr("repos.createFirst"))
                    }
                    .accessibilityLabel(tr("repos.openAndCreate", ["name": workspace.name]))
                } else {
                    ForEach(workspace.projects) { p in
                        Button { onOpen(p) } label: { row(p) }
                            .buttonStyle(.plain)
                            .accessibilityLabel(tr("repos.openProjectNamed", ["name": HomeRules.label(workspace, p)]))
                    }
                    if !examples {
                        Button(action: onNewProject) { footerRow(symbol: "plus", label: tr("project.new")) }
                            .accessibilityLabel(tr("project.newHint"))
                    }
                }
            }
            .background(HomeStyle.card, in: RoundedRectangle(cornerRadius: 26, style: .continuous))
            .clipShape(RoundedRectangle(cornerRadius: 26, style: .continuous))
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(tr("repos.card", ["name": workspace.name]))
    }

    private var header: some View {
        HStack(spacing: 8) {
            VStack(alignment: .leading, spacing: 0) {
                Text(workspace.name).font(.system(size: 20, weight: .bold)).lineLimit(1)
                Text(HomeRules.gitLine(git, l: L10n.shared) ?? L10n.shared.plural("repos.projects", workspace.projects.count))
                    .font(.system(size: 13))
                    .foregroundStyle(HomeStyle.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            pill
            menu
        }
    }

    @ViewBuilder private var pill: some View {
        switch HomeRules.pill(git, examples: examples) {
        case .none:
            EmptyView()
        case .connect:
            Button { onWeb(HomeRules.syncRoute(workspace.name)) } label: {
                QuietPill(symbol: "arrow.triangle.2.circlepath", label: tr("git.card.connect"))
            }
            .accessibilityLabel(tr("git.card.connectName", ["workspace": workspace.name]))
        case .merge(let n):
            Button { onWeb(HomeRules.webHomeRoute) } label: {
                CountPill(symbol: "arrow.triangle.merge", label: L10n.shared.plural("git.mergeOpen", n), count: nil)
            }
        case .pull(let n):
            Button { onWeb(HomeRules.webHomeRoute) } label: {
                CountPill(symbol: "arrow.down", label: tr("git.pull"), count: n)
            }
            .accessibilityLabel(tr("git.card.pullN", ["n": String(n)]))
        case .save(let n):
            Button { onWeb(HomeRules.webHomeRoute) } label: {
                CountPill(symbol: "square.and.arrow.down", label: tr("git.save.label"), count: n)
            }
            .accessibilityLabel(tr("git.card.saveN", ["n": String(n)]))
        }
    }

    private var menu: some View {
        Menu {
            if !examples {
                Button { onWeb(HomeRules.syncRoute(workspace.name)) } label: {
                    Label(tr("git.menuSync"), systemImage: "arrow.triangle.2.circlepath")
                }
            }
            Button { onWeb(HomeRules.browseRoute(workspace.name)) } label: {
                Label(tr("ws.browse"), systemImage: "folder")
            }
        } label: {
            Image(systemName: "ellipsis")
                .font(.system(size: 18, weight: .bold))
                .foregroundStyle(HomeStyle.accent)
                .frame(width: 44, height: 44)
        }
        .accessibilityLabel(tr("ws.menu", ["workspace": workspace.name]))
        .padding(.trailing, -10)
    }

    private func row(_ p: Project) -> some View {
        let shape = HomeRules.shape(p.surfaces)
        let kind = shape.map { tr(HomeRules.shapeKey($0)) }
        let when = p.editedAt.map { ago($0) }
        return HStack(spacing: 12) {
            Image(systemName: HomeStyle.symbol(shape))
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 30, height: 30)
                .background(HomeStyle.tile(shape), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: 6) {
                        Text(HomeRules.label(workspace, p)).font(.system(size: 17)).lineLimit(1).layoutPriority(1)
                        if p.active == true {
                            Circle().fill(HomeStyle.accent).frame(width: 6, height: 6)
                                .accessibilityLabel(tr("repos.active"))
                        }
                        if p.demo == true {
                            Text(tr("demo.badge"))
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(HomeStyle.warn)
                                .lineLimit(1)
                                .fixedSize()
                                .accessibilityHint(tr("demo.badgeHint"))
                        }
                    }
                    if !wide, let status = status(kind: kind, when: when, parent: HomeRules.parent(p)) {
                        Text(status).font(.system(size: 15)).foregroundStyle(HomeStyle.secondary).lineLimit(1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                if wide {
                    Text(kind ?? "").font(.system(size: 15)).foregroundStyle(HomeStyle.secondary).frame(width: 110, alignment: .leading)
                    Text(when ?? "").font(.system(size: 15)).foregroundStyle(HomeStyle.secondary).frame(width: 130, alignment: .leading)
                }
                Image(systemName: "chevron.right").font(.system(size: 13, weight: .bold)).foregroundStyle(HomeStyle.tertiary)
            }
            .padding(.trailing, 14)
            .frame(minHeight: wide ? 52 : 60)
            .overlay(alignment: .bottom) { Rectangle().fill(HomeStyle.separator).frame(height: 0.5) }
        }
        .padding(.leading, 16)
        .contentShape(Rectangle())
    }

    private func footerRow(symbol: String, label: String) -> some View {
        HStack(spacing: 12) {
            Image(systemName: symbol).font(.system(size: 18, weight: .medium)).frame(width: 30)
            Text(label).font(.system(size: 17))
            Spacer()
        }
        .foregroundStyle(HomeStyle.accent)
        .padding(.horizontal, 16)
        .frame(minHeight: 44)
        .contentShape(Rectangle())
    }

    private func status(kind: String?, when: String?, parent: String?) -> String? {
        let parts = [kind, when, parent].compactMap { $0 }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    /// "2 hours ago" in k7's language, as the web's `editedAgo`.
    private func ago(_ ms: Double) -> String {
        let f = RelativeDateTimeFormatter()
        f.locale = L10n.shared.formatLocale
        f.dateTimeStyle = .named
        let at = Date(timeIntervalSince1970: ms / 1000)
        return f.localizedString(for: min(at, now), relativeTo: now)
    }
}
