// EngineView.swift — the native Engine page, Settings › Devices › Engine: this phone's engine as the web page's phone
// section draws it (AndroidEngineSection.tsx): its state, whether the network may use it unpaired, the instruments it
// downloads and the private CA. No Quit and no full screen: an iOS app never ends itself. See docs/native-ui-plan.md.

import SwiftUI
import UIKit
import UniformTypeIdentifiers

struct EnginePage: View {
    @ObservedObject var engine: EngineModel
    @State private var picking: ((URL?) -> Void)?

    var body: some View {
        SettingsList {
            Section {
                Text(engine.line(.shared)).foregroundStyle(HomeStyle.ink).accessibilityIdentifier("engine.line")
                if let id = engine.engineId {
                    Text(tr("engine.id", ["id": id])).font(.footnote).foregroundStyle(HomeStyle.secondary)
                }
            } header: { Text(tr("android.title")) } footer: { Text(tr("android.hint")) }
            Section {
                Toggle(isOn: Binding(get: { engine.lanOpen }, set: { engine.setLanOpen($0) })) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(tr("engine.lanOpen")).foregroundStyle(HomeStyle.ink)
                        Text(tr(engine.lanOpen ? "engine.lanOpenOn" : "engine.lanOpenOff"))
                            .font(.footnote).foregroundStyle(HomeStyle.secondary)
                    }
                }
                .disabled(engine.state == nil)
            } header: { Text(tr("engine.lanTitle")) } footer: { Text(tr("engine.lanHint")) }
            InstrumentsSection(engine: engine)
            Section {
                let count = engine.caCount ?? 0
                Text(count == 0 ? tr("android.caSystemIos") : tr("android.caYoursIos", ["count": String(count)]))
                    .foregroundStyle(HomeStyle.ink)
                if let note = engine.caNote { NoteRow(text: note) }
                HStack {
                    Button(tr("android.caTrust")) {
                        engine.cas("importCa", doing: tr("android.caAdding")) { then in picking = then }
                    }
                    .buttonStyle(.borderless)
                    Spacer()
                    Button(tr("android.caForget"), role: .destructive) {
                        engine.cas("forgetCas", doing: tr("android.caForgetting")) { _ in }
                    }
                    .buttonStyle(.borderless)
                    .disabled(count == 0)
                }
                .disabled(engine.caBusy)
            } header: { Text(tr("android.caTitle")) } footer: { Text(tr("android.caHintIos")) }
        }
        .navigationTitle(tr("settings.engineRow"))
        .navigationBarTitleDisplayMode(.inline)
        .onAppear { engine.start() }
        .fileImporter(isPresented: Binding(get: { picking != nil }, set: { if !$0 { finish(nil) } }),
                      allowedContentTypes: CertPicker.types) { result in
            finish(try? result.get())
        }
    }

    private func finish(_ url: URL?) {
        let then = picking
        picking = nil
        then?(url)
    }
}

enum CertPicker {
    /// What a certificate file is called, as the web page's picker in StudioBridge.swift asks for.
    static let types: [UTType] = [UTType.x509Certificate, UTType(filenameExtension: "pem"), UTType(filenameExtension: "crt"),
                                  UTType.data].compactMap { $0 }
}
