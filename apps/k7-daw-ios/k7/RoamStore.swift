// RoamStore.swift — the roaming list's store: NSUbiquitousKeyValueStore under the two C callbacks the Rust front reads and
// writes it through (apps/k7-daw-backend/docs/roam.md). What a reader gets wrong: only keys `ws.<id>` and `forge.<id>`
// pass, and the callbacks capture nothing, since a C function pointer cannot.

import Foundation
import os

enum RoamStore {
    /// The front's keys, the workspace list's and the Forgejo servers' (`k7_roam::store::KEY_PREFIXES`).
    static let prefixes = ["ws.", "forge."]
    static func ours(_ key: String) -> Bool { prefixes.contains { key.hasPrefix($0) } }

    private static let log = Logger(subsystem: "rocks.k7.daw", category: "k7-roam")

    /// Before the front starts: pulls the store, notes what other devices change, and hands the front its callbacks.
    static func install() {
        let store = NSUbiquitousKeyValueStore.default
        NotificationCenter.default.addObserver(
            forName: NSUbiquitousKeyValueStore.didChangeExternallyNotification, object: store, queue: nil
        ) { note in
            let keys = note.userInfo?[NSUbiquitousKeyValueStoreChangedKeysKey] as? [String] ?? []
            log.info("the list changed on another device: \(keys.count) key(s)")
        }
        store.synchronize()
        k7_roam_install({ emit, ctx in
            guard let emit else { return }
            for (key, value) in NSUbiquitousKeyValueStore.default.dictionaryRepresentation where RoamStore.ours(key) {
                guard let text = value as? String else { continue }
                key.withCString { k in text.withCString { v in emit(ctx, k, v) } }
            }
        }, { key, value in
            guard let key else { return }
            let store = NSUbiquitousKeyValueStore.default
            let name = String(cString: key)
            guard RoamStore.ours(name) else { return }
            if let value { store.set(String(cString: value), forKey: name) } else { store.removeObject(forKey: name) }
            store.synchronize()
        })
    }
}
