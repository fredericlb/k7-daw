// HomeViews.swift — the native Projects home: on an iPhone a scrolling page under a glass header with the floating tab
// bar; on an iPad a split view, the tabs and the workspaces in the sidebar. Both list WorkspaceSection.
// Settings is SettingsViews.swift; a tab that is not native yet opens its web page (HomeHost). See docs/native-ui-plan.md.

import SwiftUI

/// The sheet the page has open, if any.
enum HomeSheet: Identifiable {
    case newWorkspace
    case newProject(String)
    var id: String {
        switch self {
        case .newWorkspace: return "ws"
        case .newProject(let w): return "p:\(w)"
        }
    }
}

struct HomeRoot: View {
    @ObservedObject var model: HomeModel
    @ObservedObject var settings: SettingsModel
    @ObservedObject var engine: EngineModel
    @ObservedObject var l10n: L10n = .shared
    @ObservedObject var prefs: AppPrefs = .shared
    @Environment(\.horizontalSizeClass) private var size
    @State private var sheet: HomeSheet?

    var body: some View {
        Group {
            if size == .regular {
                PadHome(model: model, settings: settings, engine: engine, sheet: $sheet)
            } else {
                PhoneHome(model: model, settings: settings, engine: engine, sheet: $sheet)
            }
        }
        // Every word is read when drawn: a language change draws the page again.
        .id(l10n.locale)
        .preferredColorScheme(prefs.theme.scheme)
        .tint(HomeStyle.accent)
        .sheet(item: $sheet) { s in
            switch s {
            case .newWorkspace:
                NewWorkspaceSheet { try await model.createWorkspace($0) }
            case .newProject(let w):
                NewProjectSheet(workspace: w) { try await model.createProject(workspace: w, folder: $0, shape: $1) }
            }
        }
    }
}

/// The list itself, shared by both layouts: what stands in for it while it loads or fails, the notices, the sections.
struct ProjectsList: View {
    @ObservedObject var model: HomeModel
    let wide: Bool
    @Binding var sheet: HomeSheet?

    var body: some View {
        TimelineView(.periodic(from: .now, by: 60)) { clock in
            VStack(alignment: .leading, spacing: 0) {
                if let notice = model.notice {
                    banner(notice, HomeStyle.warn)
                }
                if model.loading && model.workspaces.isEmpty {
                    HStack(spacing: 8) { ProgressView(); Text(tr("repos.reading")) }
                        .foregroundStyle(HomeStyle.secondary).padding(20)
                } else if let error = model.error, model.workspaces.isEmpty {
                    banner(tr("repos.noFilesTitle") + "\n" + error, HomeStyle.error)
                } else if !model.hasOwn && model.query.isEmpty {
                    empty
                }
                ForEach(model.listed) { w in
                    WorkspaceSection(
                        workspace: w, git: model.git[w.name], wide: wide, now: clock.date,
                        onOpen: { model.open(workspace: w.name, project: $0.path) },
                        onOpenWorkspace: { model.open(workspace: w.name, project: ".") },
                        onNewProject: { sheet = .newProject(w.name) },
                        onWeb: model.openWeb
                    )
                    .id(w.name)
                    .padding(.horizontal, wide ? 20 : 16)
                    .padding(.top, wide ? 18 : 14)
                    .padding(.bottom, 6)
                }
                if !model.query.isEmpty && model.listed.isEmpty {
                    Text(tr("repos.filterNone", ["query": model.query])).foregroundStyle(HomeStyle.secondary).padding(20)
                }
            }
        }
    }

    private var empty: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(tr("repos.emptyTitle")).font(.system(size: 20, weight: .bold))
            Text(tr("repos.empty")).foregroundStyle(HomeStyle.secondary)
            HStack(spacing: 10) {
                Button { sheet = .newWorkspace } label: { CountPill(symbol: "externaldrive", label: tr("ws.new"), count: nil) }
                    .disabled(model.workspaceBlocked != nil)
                Button { model.openWeb(HomeRules.cloneRoute) } label: { QuietPill(symbol: "arrow.down.circle", label: tr("git.clone")) }
            }
            if let blocked = model.workspaceBlocked {
                Text(tr(blocked)).font(.footnote).foregroundStyle(HomeStyle.warn)
            }
        }
        .padding(20)
    }

    private func banner(_ text: String, _ color: Color) -> some View {
        Text(text)
            .font(.footnote)
            .foregroundStyle(color)
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(color.opacity(0.12), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .padding(.horizontal, 16)
            .padding(.top, 8)
    }
}

/// The header's ⋯: clone a repository, and whether Examples is listed.
struct PageMenu: View {
    @ObservedObject var model: HomeModel
    let symbol: String

    var body: some View {
        Menu {
            Button { model.openWeb(HomeRules.cloneRoute) } label: { Label(tr("git.clone"), systemImage: "arrow.down.circle") }
            Toggle(isOn: $model.chosenExamples) { Label(tr("repos.showExamples"), systemImage: "music.note.list") }
                .disabled(!model.hasOwn)
        } label: {
            Image(systemName: symbol).font(.system(size: 19, weight: .semibold)).frame(width: 44, height: 44)
        }
        .accessibilityLabel(tr("repos.pageMenu"))
    }
}

struct PhoneHome: View {
    @ObservedObject var model: HomeModel
    @ObservedObject var settings: SettingsModel
    @ObservedObject var engine: EngineModel
    @Binding var sheet: HomeSheet?
    @State private var searching = false
    @FocusState private var searchFocused: Bool

    var body: some View {
        ZStack(alignment: .bottom) {
            if model.tab == .settings {
                SettingsRoot(model: settings, engine: engine, path: $model.settingsPath, bottomInset: 80)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        header
                        if searching { search }
                        ProjectsList(model: model, wide: false, sheet: $sheet)
                    }
                    .padding(.bottom, 120)
                }
                .scrollDismissesKeyboard(.interactively)
                // On the list alone: Settings' buttons keep the accent the system draws them in.
                .foregroundStyle(HomeStyle.ink)
            }
            TabBar(model: model)
        }
        .background(HomeStyle.ground.ignoresSafeArea())
    }

    private var header: some View {
        HStack(spacing: 12) {
            Text(tr("home.projects")).font(.system(size: 30, weight: .bold)).accessibilityAddTraits(.isHeader)
            Spacer()
            HStack(spacing: 0) {
                Button {
                    searching.toggle()
                    searchFocused = searching
                    if !searching { model.query = "" }
                } label: {
                    Image(systemName: "magnifyingglass").font(.system(size: 18, weight: .semibold)).frame(width: 44, height: 44)
                }
                .accessibilityLabel(tr("repos.filter"))
                Button { sheet = .newWorkspace } label: {
                    Image(systemName: "plus").font(.system(size: 20, weight: .semibold)).frame(width: 44, height: 44)
                }
                .disabled(model.workspaceBlocked != nil)
                .accessibilityLabel(tr("ws.new"))
                PageMenu(model: model, symbol: "ellipsis")
            }
            .padding(.horizontal, 4)
            .foregroundStyle(HomeStyle.ink)
            .glass(Capsule())
        }
        .frame(height: 52)
        .padding(.leading, 20)
        .padding(.trailing, 16)
    }

    private var search: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(HomeStyle.secondary)
            TextField(tr("repos.filter"), text: $model.query)
                .focused($searchFocused)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
        }
        .padding(.horizontal, 14)
        .frame(height: 44)
        .background(HomeStyle.fill, in: Capsule())
        .padding(.horizontal, 16)
        .padding(.top, 4)
    }
}

/// The floating tab bar of the iPhone mockup: Projects and Settings here, the others their web pages for now.
struct TabBar: View {
    @ObservedObject var model: HomeModel

    var body: some View {
        HStack(spacing: 0) {
            ForEach(HomeTab.allCases) { tab in
                let current = tab == model.tab
                Button { model.choose(tab) } label: {
                    VStack(spacing: 2) {
                        Image(systemName: tab.symbol).font(.system(size: 20))
                        Text(tr(tab.key)).font(.system(size: 10, weight: .semibold)).lineLimit(1).minimumScaleFactor(0.8)
                    }
                    .foregroundStyle(current ? HomeStyle.accentLight : HomeStyle.tabInk)
                    .frame(maxWidth: .infinity, minHeight: 52)
                    .background(current ? HomeStyle.accent.opacity(0.24) : .clear, in: Capsule())
                }
                .accessibilityAddTraits(current ? .isSelected : [])
                .accessibilityIdentifier("tab.\(tab.rawValue)")
            }
        }
        .padding(4)
        .frame(height: 62)
        .glass(Capsule())
        .padding(.horizontal, 16)
        .padding(.bottom, 8)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(tr("home.nav"))
    }
}

struct PadHome: View {
    @ObservedObject var model: HomeModel
    @ObservedObject var settings: SettingsModel
    @ObservedObject var engine: EngineModel
    @Binding var sheet: HomeSheet?
    @State private var target: String?

    var body: some View {
        NavigationSplitView {
            sidebar
        } detail: {
            if model.tab == .settings {
                SettingsRoot(model: settings, engine: engine, path: $model.settingsPath, bottomInset: 0)
            } else {
                GeometryReader { geo in
                    ScrollViewReader { reader in
                        detail(reader, columns: geo.size.width >= Self.columnsFrom)
                    }
                }
                .background(HomeStyle.ground.ignoresSafeArea())
                .toolbar(.hidden, for: .navigationBar)
                .foregroundStyle(HomeStyle.ink)
            }
        }
    }

    /// The width from which a row's kind and date get columns of their own, as on the iPad board.
    static let columnsFrom: CGFloat = 700

    private func detail(_ reader: ScrollViewProxy, columns: Bool) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                header
                ProjectsList(model: model, wide: columns, sheet: $sheet)
            }
            .padding(.bottom, 40)
        }
        .onChange(of: target) { _, name in
            guard let name else { return }
            withAnimation { reader.scrollTo(name, anchor: .top) }
            target = nil
        }
    }

    private var sidebar: some View {
        List {
            Section {
                ForEach(HomeTab.allCases) { tab in
                    Button { model.choose(tab) } label: {
                        Label(tr(tab.key), systemImage: tab.symbol)
                    }
                    .listRowBackground(tab == model.tab ? HomeStyle.accent.opacity(0.24) : Color.clear)
                    .accessibilityAddTraits(tab == model.tab ? .isSelected : [])
                    .accessibilityIdentifier("tab.\(tab.rawValue)")
                }
            }
            Section(tr("home.workspaces")) {
                ForEach(model.listed) { w in
                    Button {
                        model.choose(.projects)
                        target = w.name
                    } label: {
                        HStack {
                            Label(w.name, systemImage: "folder")
                            Spacer()
                            Text("\(w.projects.count)").foregroundStyle(HomeStyle.secondary)
                        }
                    }
                }
            }
        }
        .navigationTitle("K7 DAW")
        .foregroundStyle(HomeStyle.ink)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button { sheet = .newWorkspace } label: { Image(systemName: "folder.badge.plus") }
                    .disabled(model.workspaceBlocked != nil)
                    .accessibilityLabel(tr("ws.new"))
            }
        }
    }

    private var header: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 0) {
                Text(tr("home.projects")).font(.system(size: 34, weight: .bold)).fixedSize().accessibilityAddTraits(.isHeader)
                if model.projectTotal > 0 {
                    Text(L10n.shared.plural("repos.projects", model.projectTotal) + " · "
                        + L10n.shared.plural("repos.workspaces", model.listed.count))
                        .font(.system(size: 13)).foregroundStyle(HomeStyle.secondary).lineLimit(1)
                }
            }
            .layoutPriority(1)
            Spacer(minLength: 0)
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass").foregroundStyle(HomeStyle.secondary)
                TextField(tr("repos.filter"), text: $model.query)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }
            .padding(.horizontal, 14)
            .frame(maxWidth: 280)
            .frame(height: 44)
            .background(HomeStyle.fill, in: Capsule())
            HStack(spacing: 0) {
                PageMenu(model: model, symbol: "ellipsis.circle")
                Button { sheet = .newWorkspace } label: {
                    Image(systemName: "plus").font(.system(size: 21, weight: .medium)).frame(width: 44, height: 44)
                }
                .disabled(model.workspaceBlocked != nil)
                .accessibilityLabel(tr("ws.new"))
            }
            .padding(.horizontal, 4)
            .glass(Capsule())
        }
        .padding(.horizontal, 20)
        .padding(.top, 8)
        .frame(minHeight: 60)
    }
}
