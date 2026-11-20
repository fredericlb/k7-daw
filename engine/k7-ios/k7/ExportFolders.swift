// ExportFolders.swift — where the page's exports go on this phone: a folder the person picks with the system's
// folder picker, kept as a security-scoped bookmark (the page holds it as its `id`), then each file written into it
// from the chunks the page sends over StudioBridge. The folder's scope stays open for the life of the app, which is
// also what lets the front write a background film to its `path`. The thing a reader would get wrong: the bookmark
// is the folder's identity, not the path, which a moved folder changes. See engine/k7-runtime/docs/ios.md.

import UIKit
import UniformTypeIdentifiers

final class ExportFolders: NSObject, UIDocumentPickerDelegate {
    static let shared = ExportFolders()

    private static let lastKey = "k7.export.lastFolder"
    // One queue, so the chunks of a file are written in the order they came.
    private let io = DispatchQueue(label: "k7-export")
    private var picked: ((URL?) -> Void)?
    private var scoped = Set<URL>()
    private var open: [String: (handle: FileHandle, url: URL)] = [:]

    /// The picker, opened on the last folder; `{id, name, path}` or `[:]` when cancelled.
    func pick(_ answer: @escaping (Result<[String: Any], Error>) -> Void) {
        DispatchQueue.main.async { [self] in
            picked = { url in
                guard let url, let bookmark = try? url.bookmarkData() else { return answer(.success([:])) }
                self.hold(url)
                UserDefaults.standard.set(bookmark, forKey: Self.lastKey)
                answer(.success(["id": bookmark.base64EncodedString(), "name": url.lastPathComponent, "path": url.path]))
            }
            let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.folder])
            picker.delegate = self
            if let last = UserDefaults.standard.data(forKey: Self.lastKey), let url = resolve(last) { picker.directoryURL = url }
            Dialogs.top()?.present(picker, animated: true)
        }
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        picked?(urls.first)
        picked = nil
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        picked?(nil)
        picked = nil
    }

    /// A file created in the folder, never over one there; the answer carries the token the chunks name.
    func begin(folder: String, name: String, _ answer: @escaping (Result<[String: Any], Error>) -> Void) {
        io.async { [self] in
            answer(Result {
                guard let data = Data(base64Encoded: folder), let dir = resolve(data) else { throw Self.fail("the folder is gone: choose it again") }
                let url = Self.free(dir, Self.plain(name))
                guard FileManager.default.createFile(atPath: url.path, contents: nil) else { throw Self.fail("cannot create \(url.lastPathComponent)") }
                let token = UUID().uuidString
                open[token] = (try FileHandle(forWritingTo: url), url)
                return ["token": token]
            })
        }
    }

    func chunk(token: String, data: String, _ answer: @escaping (Result<[String: Any], Error>) -> Void) {
        io.async { [self] in
            answer(Result {
                guard let file = open[token], let bytes = Data(base64Encoded: data) else { throw Self.fail("no export \(token)") }
                try file.handle.write(contentsOf: bytes)
                return [:]
            })
        }
    }

    /// Closes the file, or removes it when `cancel`; answers the name it has and its URL.
    func end(token: String, cancel: Bool, _ answer: @escaping (Result<[String: Any], Error>) -> Void) {
        io.async { [self] in
            answer(Result {
                guard let file = open.removeValue(forKey: token) else { throw Self.fail("no export \(token)") }
                try? file.handle.close()
                if cancel {
                    try? FileManager.default.removeItem(at: file.url)
                    return [:]
                }
                return ["name": file.url.lastPathComponent, "where": file.url.absoluteString]
            })
        }
    }

    /// The system share sheet, offered the saved file.
    func share(where location: String, _ answer: @escaping (Result<[String: Any], Error>) -> Void) {
        DispatchQueue.main.async {
            guard let url = URL(string: location), let top = Dialogs.top() else { return answer(.failure(Self.fail("nothing to share"))) }
            let sheet = UIActivityViewController(activityItems: [url], applicationActivities: nil)
            // An iPad anchors the sheet; a phone ignores it.
            sheet.popoverPresentationController?.sourceView = top.view
            sheet.popoverPresentationController?.sourceRect = CGRect(x: top.view.bounds.midX, y: top.view.bounds.midY, width: 0, height: 0)
            top.present(sheet, animated: true)
            answer(.success([:]))
        }
    }

    /// The folder a bookmark names, its scope open; nil when it no longer resolves.
    private func resolve(_ bookmark: Data) -> URL? {
        var stale = false
        guard let url = try? URL(resolvingBookmarkData: bookmark, bookmarkDataIsStale: &stale) else { return nil }
        hold(url)
        return url
    }

    private func hold(_ url: URL) {
        if scoped.insert(url).inserted { _ = url.startAccessingSecurityScopedResource() }
    }

    private static func plain(_ given: String) -> String {
        let name = given.split(whereSeparator: { $0 == "/" || $0 == "\\" }).last.map(String.init) ?? ""
        let clean = String(name.filter { !$0.isNewline && !"<>:\"|?*".contains($0) && $0.asciiValue.map { $0 >= 32 } ?? true })
        let trimmed = clean.trimmingCharacters(in: .whitespaces).drop(while: { $0 == "." })
        return trimmed.isEmpty ? "export" : String(trimmed.prefix(200))
    }

    /// `name`, or `stem (2).ext`, `stem (3).ext`… the first a file of the folder does not have.
    private static func free(_ dir: URL, _ name: String) -> URL {
        let stem = (name as NSString).deletingPathExtension
        let ext = (name as NSString).pathExtension
        var at = dir.appendingPathComponent(name)
        var n = 2
        while FileManager.default.fileExists(atPath: at.path) {
            at = dir.appendingPathComponent(ext.isEmpty ? "\(stem) (\(n))" : "\(stem) (\(n)).\(ext)")
            n += 1
        }
        return at
    }

    private static func fail(_ why: String) -> Error {
        NSError(domain: "k7", code: 5, userInfo: [NSLocalizedDescriptionKey: why])
    }
}
