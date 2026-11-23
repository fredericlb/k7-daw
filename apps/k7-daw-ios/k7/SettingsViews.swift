// SettingsViews.swift — the native Settings: the list of the web page's five tabs, a native one a page pushed in a
// navigation stack as the system's Settings does, a web one opening its web tab; and General's sections (language, appearance, commit author, AI key,
// separation chains). The other pages are SettingsPages.swift and EngineView.swift. See docs/native-ui-plan.md.

import SwiftUI

/// Settings' root: its own navigation stack, opened on the page an address asked for.
struct SettingsRoot: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var engine: EngineModel
    @Binding var path: [SettingsPage]
    /// Room left at the bottom for the iPhone's floating tab bar.
    let bottomInset: CGFloat

    var body: some View {
        NavigationStack(path: $path) {
            SettingsList {
                Section {
                    ForEach(SettingsTab.allCases) { tab in
                        if tab.native {
                            NavigationLink(value: SettingsPage.tab(tab)) {
                                SettingsLabel(title: tr(tab.key), symbol: tab.symbol)
                            }
                            .accessibilityIdentifier("settings.\(tab.rawValue)")
                        } else {
                            WebRow(title: tr(tab.key), symbol: tab.symbol, route: tab.webRoute, open: model.openWeb)
                                .accessibilityIdentifier("settings.\(tab.rawValue)")
                        }
                    }
                } footer: {
                    Text(tr("settings.scope"))
                }
            }
            .navigationTitle(tr("settings.title"))
            .navigationDestination(for: SettingsPage.self) { page in
                Group {
                    switch page {
                    case .tab(.general): GeneralPage(model: model, engine: engine)
                    case .tab(.devices): DevicesPage(model: model, engine: engine)
                    case .tab(.about): AboutPage(model: model)
                    case .tab(.services), .tab(.downloads): EmptyView()
                    case .engine: EnginePage(engine: engine)
                    }
                }
                .safeAreaInset(edge: .bottom) { Color.clear.frame(height: bottomInset) }
            }
            .safeAreaInset(edge: .bottom) { Color.clear.frame(height: bottomInset) }
        }
    }
}

/// The grouped list every Settings page draws, on the home's ground with its cards.
struct SettingsList<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        List { content() }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(HomeStyle.ground.ignoresSafeArea())
    }
}

/// A row's label with its symbol in a tinted tile, as the system's Settings draws one.
struct SettingsLabel: View {
    let title: String
    let symbol: String
    var detail: String?

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: symbol)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 30, height: 30)
                .background(HomeStyle.primary, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            VStack(alignment: .leading, spacing: 1) {
                Text(title).foregroundStyle(HomeStyle.ink)
                if let detail { Text(detail).font(.footnote).foregroundStyle(HomeStyle.secondary) }
            }
        }
    }
}

/// A row that opens a page not native yet in k7's web view.
struct WebRow: View {
    let title: String
    let symbol: String
    var detail: String?
    let route: String
    let open: (String) -> Void

    var body: some View {
        Button { open(route) } label: {
            HStack {
                SettingsLabel(title: title, symbol: symbol, detail: detail)
                Spacer()
                Image(systemName: "chevron.right").font(.system(size: 13, weight: .bold)).foregroundStyle(HomeStyle.tertiary)
            }
        }
    }
}

/// A row that is a choice among several, checked when it is the current one.
struct ChoiceRow: View {
    let title: String
    let chosen: Bool
    let choose: () -> Void

    var body: some View {
        Button(action: choose) {
            HStack {
                Text(title).foregroundStyle(HomeStyle.ink)
                Spacer()
                if chosen { Image(systemName: "checkmark").font(.body.weight(.semibold)).foregroundStyle(HomeStyle.accent) }
            }
        }
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

/// A sentence said under a section's rows: a refusal, a warning or a quiet note.
struct NoteRow: View {
    let text: String
    var color: Color = HomeStyle.secondary

    var body: some View {
        Text(text).font(.footnote).foregroundStyle(color)
    }
}

struct GeneralPage: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var engine: EngineModel
    @ObservedObject var l10n: L10n = .shared
    @ObservedObject var prefs: AppPrefs = .shared

    var body: some View {
        SettingsList {
            if let error = model.error { NoteRow(text: error, color: HomeStyle.error) }
            Section {
                ForEach(L10n.locales, id: \.self) { l in
                    ChoiceRow(title: SettingsRules.languageName(l), chosen: l == l10n.locale) {
                        Task { await model.setLocale(l) }
                    }
                    .accessibilityIdentifier("settings.language.\(l)")
                }
            } header: { Text(tr("lang.title")) } footer: { Text(tr("lang.detail")) }
            Section {
                ForEach(Theme.allCases) { t in
                    ChoiceRow(title: tr(t.key), chosen: t == prefs.theme) { Task { await model.setTheme(t) } }
                        .accessibilityIdentifier("settings.theme.\(t.rawValue)")
                }
            } header: { Text(tr("settings.themeTitle")) } footer: { Text(tr("settings.themeHint")) }
            IdentitySection(model: model)
            AiSection(model: model)
            ChainsSection(model: model)
        }
        .navigationTitle(tr("settings.tabGeneral"))
        .navigationBarTitleDisplayMode(.inline)
        .task {
            engine.start()
            await model.loadGeneral()
        }
        // The engine's id arrives with its first state: the chains it holds are read then.
        .onChange(of: engine.engineId) { _, _ in Task { await model.loadChains() } }
    }
}

/// The commit author: two fields that save themselves after a pause and when one is left, as the web's do.
struct IdentitySection: View {
    @ObservedObject var model: SettingsModel
    @State private var name = ""
    @State private var email = ""
    @FocusState private var focus: String?

    var body: some View {
        Section {
            field("name", tr("git.identityName"), $name, source: model.identity?.nameSource, shown: model.identity?.name)
                .textContentType(.name)
            field("email", tr("git.identityEmail"), $email, source: model.identity?.emailSource, shown: model.identity?.email)
                .textContentType(.emailAddress)
                .keyboardType(.emailAddress)
                .textInputAutocapitalization(.never)
        } header: { Text(tr("git.identityTitle")) } footer: { Text(tr("git.identityHint")) }
        .disabled(model.identity == nil)
        .onChange(of: model.identity) { _, known in
            guard let known else { return }
            // A draft that only differs by spaces stays: the save trims it, and the field must not jump under the caret.
            if name.trimmingCharacters(in: .whitespaces) != known.name { name = known.name }
            if email.trimmingCharacters(in: .whitespaces) != known.email { email = known.email }
        }
        .onChange(of: focus) { _, now in if now == nil { save() } }
        .task(id: name + "\u{1F}" + email) {
            try? await Task.sleep(nanoseconds: 800_000_000)
            if !Task.isCancelled { save() }
        }
    }

    private func save() {
        Task { await model.saveIdentity(name: name, email: email) }
    }

    private func field(_ id: String, _ label: String, _ text: Binding<String>, source: String?, shown: String?) -> some View {
        let note = model.identityNote?.field == id ? model.identityNote : nil
        let suggested = text.wrappedValue.trimmingCharacters(in: .whitespaces) == shown
            ? SettingsRules.sourceKey(source).map { tr($0) } : nil
        return VStack(alignment: .leading, spacing: 2) {
            LabeledContent(label) {
                TextField(label, text: text)
                    .multilineTextAlignment(.trailing)
                    .autocorrectionDisabled()
                    .focused($focus, equals: id)
                    .onSubmit(save)
                    .accessibilityIdentifier("settings.identity.\(id)")
            }
            if let line = note?.text ?? suggested {
                Text(line).font(.footnote).foregroundStyle(note?.error == true ? HomeStyle.error : HomeStyle.secondary)
            }
        }
    }
}

/// The Claude key and model: write-only, a hint of the key is all that comes back.
struct AiSection: View {
    @ObservedObject var model: SettingsModel
    @State private var key = ""
    @State private var chosenModel = ""
    @State private var askingRemove = false

    var body: some View {
        let ai = model.ai
        Section {
            HStack {
                VStack(alignment: .leading, spacing: 1) {
                    Text(ai?.keyHint.map { tr("ai.keyHint", ["hint": $0]) } ?? tr("ai.noKey")).foregroundStyle(HomeStyle.ink)
                    Text(tr(ai?.keyHint != nil ? "ai.keyOn" : "ai.keyOff")).font(.footnote).foregroundStyle(HomeStyle.secondary)
                }
                Spacer()
                if ai?.keyHint != nil {
                    Button(tr("ai.remove"), role: .destructive) { askingRemove = true }.disabled(model.aiBusy)
                }
            }
            HStack {
                SecureField(ai?.keyHint != nil ? tr("ai.replaceKey") : "sk-ant-…", text: $key)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityLabel(tr("ai.newKey"))
                Button(tr("common.save")) { saveKey() }
                    .disabled(model.aiBusy || key.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            HStack {
                TextField(ai?.defaultModel ?? tr("ai.modelPlaceholder"), text: $chosenModel)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityLabel(tr("ai.model"))
                Button(tr("common.save")) { saveModel() }.disabled(model.aiBusy || !modelChanged)
            }
            if let error = model.aiError { NoteRow(text: error, color: HomeStyle.error) }
        } header: { Text(tr("ai.title")) } footer: { Text(tr("ai.hint")) }
        .onAppear { chosenModel = model.ai?.model ?? "" }
        .onChange(of: model.ai) { _, v in chosenModel = v?.model ?? "" }
        .confirmationDialog(tr("ai.removeTitle"), isPresented: $askingRemove, titleVisibility: .visible) {
            Button(tr("ai.remove"), role: .destructive) { Task { await model.saveAi(["apiKey": NSNull()]) } }
        } message: { Text(tr("ai.removeBody")) }
    }

    private var modelChanged: Bool { chosenModel.trimmingCharacters(in: .whitespaces) != (model.ai?.model ?? "") }

    private func saveKey() {
        let typed = key
        Task {
            await model.saveAi(["apiKey": typed])
            // The typed key is gone once stored: the hint is all that stays on screen.
            if model.aiError == nil { key = "" }
        }
    }

    private func saveModel() {
        let m = chosenModel.trimmingCharacters(in: .whitespaces)
        let value: Any = m.isEmpty ? NSNull() : m
        Task { await model.saveAi(["model": value]) }
    }
}

/// Which cold chain this device's engine separates with and makes an instrumental with: the engine's own setting.
struct ChainsSection: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Section {
            if model.engineId() == nil {
                NoteRow(text: tr("settings.chains.noEngine"))
            } else {
                picker("stems", title: tr("stems.title"), none: tr("stems.none"), body: tr("stems.body"),
                       value: model.held?.stems, options: model.cold?.chains ?? [])
                // Only those that WRITE an instrumental: four stems is not one, and the far side would refuse it.
                picker("instrumental", title: tr("instrumental.title"), none: tr("instrumental.none"),
                       body: tr("instrumental.body"), value: model.held?.instrumental,
                       options: (model.cold?.chains ?? []).filter(\.instrumental))
                if let cold = model.cold, cold.chains.isEmpty {
                    NoteRow(text: tr("settings.chains.noCold", ["dir": "chains/"]))
                }
            }
            if let error = model.chainsError { NoteRow(text: error, color: HomeStyle.error) }
        } header: { Text(tr("settings.chains.title")) } footer: { Text(tr("settings.chains.hint")) }
    }

    private func picker(_ which: String, title: String, none: String, body: String, value: String?,
                        options: [ColdChains.Chain]) -> some View {
        let shown = SettingsRules.options(options, held: value)
        let chosen = options.first { $0.ref == value }
        let mine = model.fetching.flatMap { $0.ref == value ? $0 : nil }
        return VStack(alignment: .leading, spacing: 6) {
            Picker(selection: Binding(get: { value ?? "" }, set: { ref in Task { await model.choose(which, ref) } })) {
                Text(none).tag("")
                ForEach(shown) { c in
                    Text(c.unavailable == nil ? c.ref
                         : "\(c.ref) — \(tr(c.downloads.isEmpty ? "settings.chains.unavailableTag" : "settings.chains.downloadTag"))")
                        .tag(c.ref)
                }
            } label: {
                Text(title.prefix(1).uppercased() + title.dropFirst()).foregroundStyle(HomeStyle.ink)
            }
            .pickerStyle(.menu)
            .disabled(model.held == nil || model.chainsBusy)
            if let why = chosen?.unavailable {
                NoteRow(text: tr("settings.chains.unavailable", ["reason": why]), color: HomeStyle.warn)
            }
            if let c = chosen, !c.downloads.isEmpty, mine?.running != true {
                Button(tr("settings.chains.download", ["file": c.downloads.map(\.file).joined(separator: ", ")])) {
                    Task { await model.download(c.ref) }
                }
                .font(.footnote)
            }
            if let mine { NoteRow(text: mine.text, color: mine.failed ? HomeStyle.error : HomeStyle.secondary) }
            NoteRow(text: body)
        }
    }
}
