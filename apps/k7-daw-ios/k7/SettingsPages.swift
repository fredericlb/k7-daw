// SettingsPages.swift — Settings' Devices and About pages and the engine's instruments. Native: the Engine row, the
// instruments, the version and the source link. Still the web settings page's: the paired devices, the licence text and
// the notices (Services and Downloads are web tabs as a whole, SettingsRoot). See docs/native-ui-plan.md.

import SwiftUI

struct DevicesPage: View {
    @ObservedObject var model: SettingsModel
    @ObservedObject var engine: EngineModel

    var body: some View {
        SettingsList {
            Section {
                NavigationLink(value: SettingsPage.engine) {
                    SettingsLabel(title: tr("settings.engineRow"), symbol: "waveform", detail: tr("settings.engineRowDetail"))
                }
                .accessibilityIdentifier("settings.engine")
            } header: { Text(tr("home.engine")) }
            // The pairing speaks the engine's own protocol, which the web page's client holds: web until its phase.
            Section {
                WebRow(title: tr("pairing.settingsTitle"), symbol: "ipad.and.iphone",
                       route: SettingsTab.devices.webRoute, open: model.openWeb)
            } footer: { Text(tr("pairing.settingsHint")) }
        }
        .navigationTitle(tr("settings.tabDevices"))
        .navigationBarTitleDisplayMode(.inline)
    }
}

/// The sampled instruments the engine can play, each with its one action (InstrumentsSection.tsx).
struct InstrumentsSection: View {
    @ObservedObject var engine: EngineModel

    var body: some View {
        Section {
            let rows = engine.instruments
            if rows.isEmpty {
                NoteRow(text: tr("engine.noInstruments"))
            }
            ForEach(rows) { i in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(i.label).foregroundStyle(HomeStyle.ink)
                        if i.phase == "downloading" {
                            ProgressView(value: i.received, total: max(i.bytes, 1))
                        } else {
                            Text(SettingsRules.instrumentLine(i, l: .shared)).font(.footnote)
                                .foregroundStyle(i.phase == "failed" ? HomeStyle.error : HomeStyle.secondary)
                        }
                    }
                    Spacer()
                    if let action = SettingsRules.action(i) {
                        Button(tr(action.key), role: action.cmd == "instruments.remove" && i.installed ? .destructive : nil) {
                            engine.instrument(action.cmd, i.name)
                        }
                        .buttonStyle(.borderless)
                    }
                }
            }
        } header: { Text(tr("engine.downloadsTitle")) } footer: { Text(tr("engine.downloadsHint")) }
    }
}

struct AboutPage: View {
    @ObservedObject var model: SettingsModel
    @Environment(\.openURL) private var openURL

    /// Where the complete source is published: the address the AGPL's offer names (the web's `SOURCE_URL`).
    static let source = "https://github.com/fredericlb/k7-daw"

    private var version: String {
        model.local?.version ?? (Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "")
    }

    var body: some View {
        SettingsList {
            Section {
                Text(tr("about.version", ["version": version])).foregroundStyle(HomeStyle.ink)
                    .accessibilityIdentifier("settings.about.version")
            } header: { Text("K7 DAW") } footer: { Text(tr("about.tagline")) }
            Section {
                Text(tr("about.license.body")).foregroundStyle(HomeStyle.ink)
                WebRow(title: tr("about.license.show"), symbol: "doc.text", route: SettingsTab.about.webRoute, open: model.openWeb)
            } header: { Text(tr("about.license.title")) }
            Section {
                Button { if let u = URL(string: Self.source) { openURL(u) } } label: {
                    HStack {
                        Text(Self.source).foregroundStyle(HomeStyle.ink).lineLimit(1).truncationMode(.middle)
                        Spacer()
                        Label(tr("about.source.open"), systemImage: "arrow.up.right.square").labelStyle(.titleAndIcon)
                    }
                }
            } header: { Text(tr("about.source.title")) } footer: { Text(tr("about.source.hint")) }
            Section {
                WebRow(title: tr("about.notices.title"), symbol: "list.bullet.rectangle", route: SettingsTab.about.webRoute,
                       open: model.openWeb)
            } footer: { Text(tr("about.notices.hint")) }
        }
        .navigationTitle(tr("settings.tabAbout"))
        .navigationBarTitleDisplayMode(.inline)
        .task { await model.loadLocal() }
    }
}
