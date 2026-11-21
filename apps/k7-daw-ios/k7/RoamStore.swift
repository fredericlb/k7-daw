// RoamStore.swift — the roaming list's store: NSUbiquitousKeyValueStore under the two C callbacks the Rust front reads and
// writes it through (apps/k7-daw-backend/docs/roam.md). What a reader gets wrong: only keys `ws.<id>` pass, and the callbacks
// capture nothing, since a C function pointer cannot.

import Foundation
import os

enum RoamStore {
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
            for (key, value) in NSUbiquitousKeyValueStore.default.dictionaryRepresentation where key.hasPrefix("ws.") {
                guard let text = value as? String else { continue }
                key.withCString { k in text.withCString { v in emit(ctx, k, v) } }
            }
        }, { key, value in
            guard let key else { return }
            let store = NSUbiquitousKeyValueStore.default
            let name = String(cString: key)
            guard name.hasPrefix("ws.") else { return }
            if let value { store.set(String(cString: value), forKey: name) } else { store.removeObject(forKey: name) }
            store.synchronize()
        })
    }
}
