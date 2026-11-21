// LanBridge.swift — discovery of other k7 devices, done with Network.framework because raw multicast on iOS needs an
// entitlement Apple grants apart (apps/k7-daw-backend/docs/lan.md). The front hands `advertise` a record, which a listener
// publishes as `_k7._tcp`; what the browser resolves is pushed back to it. The service's own port is not the door's:
// the TXT record carries the door and the engine, so the listener only needs to exist.

import Foundation
import Network
import os

enum LanBridge {
    private static let log = Logger(subsystem: "rocks.k7.daw", category: "k7-lan")
    private static let queue = DispatchQueue(label: "k7-lan")
    private static var browser: NWBrowser?
    private static var listener: NWListener?
    private static var heard: [String: [String: Any]] = [:]

    /// Before the front starts: the advertiser as a C callback, then the browse.
    static func install() {
        // A C function pointer captures nothing: the type is named, never the implicit `self` of a static member.
        k7_lan_install({ record in
            let text = record.map { String(cString: $0) }
            LanBridge.queue.async { LanBridge.advertise(text) }
        })
        queue.async { browse() }
    }

    private static func advertise(_ record: String?) {
        listener?.cancel()
        listener = nil
        guard let record, let data = record.data(using: .utf8),
              let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return }
        var txt: [String: String] = [:]
        for (key, value) in json { txt[key] = "\(value)" }
        do {
            let l = try NWListener(using: .tcp)
            l.service = NWListener.Service(
                name: "k7-\(txt["id"] ?? "device")", type: "_k7._tcp", txtRecord: NWTXTRecord(txt))
            // A peer connects only to resolve the address: nothing is said on it.
            l.newConnectionHandler = { $0.cancel() }
            l.start(queue: queue)
            listener = l
        } catch {
            log.error("not advertising: \(error.localizedDescription)")
        }
    }

    private static func browse() {
        let b = NWBrowser(for: .bonjourWithTXTRecord(type: "_k7._tcp", domain: nil), using: .tcp)
        b.browseResultsChangedHandler = { results, _ in
            queue.async { follow(results) }
        }
        b.start(queue: queue)
        browser = b
    }

    private static func follow(_ results: Set<NWBrowser.Result>) {
        let names = Set(results.map { "\($0.endpoint)" })
        heard = heard.filter { names.contains($0.key) }
        push()
        for r in results where heard["\(r.endpoint)"] == nil {
            resolve(r)
        }
    }

    /// The address a service answers on is read off a connection to it, which is dropped as soon as it is ready.
    private static func resolve(_ r: NWBrowser.Result) {
        guard case let .bonjour(txt) = r.metadata else { return }
        let params = NWParameters.tcp
        (params.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options)?.version = .v4
        let conn = NWConnection(to: r.endpoint, using: params)
        let key = "\(r.endpoint)"
        conn.stateUpdateHandler = { state in
            switch state {
            case .ready:
                if case let .hostPort(host, _)? = conn.currentPath?.remoteEndpoint {
                    var address = "\(host)"
                    if let i = address.firstIndex(of: "%") { address = String(address[..<i]) }
                    let peer: [String: Any] = [
                        "engineId": txt["id"] ?? "", "name": txt["name"] ?? "", "host": address,
                        "door": Int(txt["door"] ?? "") ?? 0, "engine": Int(txt["engine"] ?? "") ?? 0,
                        "api": Int(txt["api"] ?? "") ?? 0, "proto": Int(txt["proto"] ?? "") ?? 0,
                    ]
                    queue.async {
                        heard[key] = peer
                        push()
                    }
                }
                conn.cancel()
            case .failed:
                conn.cancel()
            default:
                break
            }
        }
        conn.start(queue: queue)
    }

    private static func push() {
        guard let data = try? JSONSerialization.data(withJSONObject: Array(heard.values)),
              let text = String(data: data, encoding: .utf8) else { return }
        _ = text.withCString { k7_lan_peers($0) }
    }
}
