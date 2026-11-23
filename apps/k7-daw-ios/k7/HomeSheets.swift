// HomeSheets.swift — New workspace and New project as native sheets: the name, the folder it becomes shown before
// creating (and why a name will not do), and for a project its archetype. The web's dialogs, in the same words.
// See docs/native-ui-plan.md.

import SwiftUI

/// A name field with its derived folder, its refusal, and the wait: the part both sheets share.
private struct NameField: View {
    let placeholder: String
    @Binding var name: String
    let preview: String
    let refusal: String?

    var body: some View {
        Section {
            TextField(placeholder, text: $name)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            if !name.isEmpty {
                if let refusal {
                    Text(refusal).foregroundStyle(HomeStyle.error).font(.footnote)
                } else {
                    Text(preview).foregroundStyle(HomeStyle.secondary).font(.footnote.monospaced())
                }
            }
        }
    }
}

/// The toolbar and the step or failure under the form: Cancel, Create, and what the wait is about.
private struct SheetChrome: ViewModifier {
    let title: String
    let canSubmit: Bool
    let busy: Bool
    let failure: String?
    let onCancel: () -> Void
    let onSubmit: () -> Void

    func body(content: Content) -> some View {
        content
            .safeAreaInset(edge: .bottom) {
                if let failure {
                    Text(failure).font(.footnote).foregroundStyle(HomeStyle.error).padding()
                }
            }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(tr("common.cancel"), action: onCancel).disabled(busy)
                }
                ToolbarItem(placement: .confirmationAction) {
                    if busy { ProgressView() } else {
                        Button(tr("common.create"), action: onSubmit).disabled(!canSubmit)
                    }
                }
            }
            .interactiveDismissDisabled(busy)
    }
}

struct NewWorkspaceSheet: View {
    let create: (String) async throws -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var busy = false
    @State private var failure: String?

    var body: some View {
        let derived = HomeRules.workspaceFolder(name)
        NavigationStack {
            Form {
                Section { Text(tr("ws.newBody")).foregroundStyle(HomeStyle.secondary) }
                NameField(placeholder: tr("ws.namePlaceholder"), name: $name, preview: derived.folder,
                          refusal: derived.error.map { tr($0) })
            }
            .modifier(SheetChrome(title: tr("ws.newTitle"), canSubmit: derived.error == nil, busy: busy, failure: failure,
                                  onCancel: { dismiss() }, onSubmit: { submit(derived.folder) }))
        }
    }

    private func submit(_ folder: String) {
        busy = true
        failure = nil
        Task {
            do {
                try await create(folder)
                dismiss()
            } catch {
                failure = error.localizedDescription
            }
            busy = false
        }
    }
}

struct NewProjectSheet: View {
    let workspace: String
    let create: (_ folder: String, _ shape: String) async throws -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var shape = "arrangement"
    @State private var busy = false
    @State private var failure: String?

    var body: some View {
        let derived = HomeRules.projectFolder(name)
        NavigationStack {
            Form {
                Section {
                    ForEach(HomeRules.shapes, id: \.self) { s in
                        Button { shape = s } label: { shapeRow(s) }.disabled(busy)
                    }
                } header: {
                    Text(tr("shape.pick"))
                } footer: {
                    Text(tr("project.newBody"))
                }
                NameField(placeholder: tr("project.namePlaceholder"), name: $name, preview: "\(workspace)/\(derived.folder)",
                          refusal: derived.error.map { tr($0) })
            }
            .modifier(SheetChrome(title: tr("project.newTitle"), canSubmit: derived.error == nil, busy: busy,
                                  failure: failure, onCancel: { dismiss() }, onSubmit: { submit(derived.folder) }))
        }
    }

    private func shapeRow(_ s: String) -> some View {
        HStack(spacing: 12) {
            Image(systemName: HomeStyle.symbol(s))
                .foregroundStyle(.white)
                .frame(width: 30, height: 30)
                .background(HomeStyle.tile(s), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text(tr(HomeRules.shapeKey(s))).foregroundStyle(.primary)
                Text(tr(HomeRules.shapeKey(s) + "What")).font(.footnote).foregroundStyle(HomeStyle.secondary)
            }
            Spacer()
            if shape == s { Image(systemName: "checkmark").foregroundStyle(HomeStyle.accent) }
        }
    }

    private func submit(_ folder: String) {
        busy = true
        failure = nil
        Task {
            do {
                try await create(folder, shape)
                dismiss()
            } catch {
                failure = error.localizedDescription
            }
            busy = false
        }
    }
}
