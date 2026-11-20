// AudioSession.swift — what the app's audio session is, said in the machine log at each open, and what interrupts it.
// JUCE sets the category and the options (EXCLUSIVE here: no MixWithOthers) and restarts its device after an
// interruption; this only watches and logs, so it never fights JUCE. See engine/k7-runtime/docs/ios.md.

import AVFAudio
import Foundation

enum AudioSession {
    private static var started = false

    /// Once per process, on the main thread.
    static func start() {
        if started { return }
        started = true
        let center = NotificationCenter.default
        // Off the main thread: describe() asks the audio daemon.
        center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: logQueue) { note in
            let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt ?? 0
            routeChanged(reason(raw))
        }
        logQueue.maxConcurrentOperationCount = 1
        logQueue.qualityOfService = .utility
        center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { note in
            let kind = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt ?? 0
            if AVAudioSession.InterruptionType(rawValue: kind) == .began {
                hostLog("audio interruption began")
            } else {
                let opts = note.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0
                let resume = AVAudioSession.InterruptionOptions(rawValue: opts).contains(.shouldResume)
                hostLog("audio interruption ended: shouldResume=\(resume ? 1 : 0)")
                report("interruption ended")
            }
        }
    }

    private static let logQueue = OperationQueue()
    private static var lastRouteLog = Date.distantPast
    private static var skippedRoutes = 0

    /// One line per second at most, counting the changes it skipped; runs on the serial `logQueue`.
    private static func routeChanged(_ why: String) {
        let now = Date()
        if now.timeIntervalSince(lastRouteLog) < 1 {
            skippedRoutes += 1
            return
        }
        let skipped = skippedRoutes
        lastRouteLog = now
        skippedRoutes = 0
        report("route change: \(why)" + (skipped > 0 ? ", \(skipped) more since the last line" : ""))
    }

    /// The engine's own lines say when its device opens or comes back; the session is read at that moment.
    static func engineSaid(_ text: String) {
        if text.hasPrefix("[device]") && (text.contains("' open:") || text.contains("reopened")) {
            logQueue.addOperation { report("device open") }
        }
    }

    /// One `audio session` line: category, options, whether others may mix in, and the route.
    static func report(_ why: String) {
        hostLog("audio session (\(why)): \(describe())")
    }

    static func describe() -> String {
        let session = AVAudioSession.sharedInstance()
        let options = session.categoryOptions
        let names = optionNames.filter { options.contains($0.0) }.map { $0.1 }.joined(separator: ",")
        let outs = session.currentRoute.outputs.map { $0.portName }.joined(separator: ",")
        let ins = session.currentRoute.inputs.map { $0.portName }.joined(separator: ",")
        let mix = options.contains(.mixWithOthers) ? 1 : 0
        return "category=\(session.category.rawValue) options=[\(names)] mixWithOthers=\(mix) route=out:[\(outs)] in:[\(ins)]"
    }

    private static let optionNames: [(AVAudioSession.CategoryOptions, String)] = [
        (.mixWithOthers, "mixWithOthers"), (.duckOthers, "duckOthers"), (.allowBluetooth, "allowBluetooth"),
        (.allowBluetoothA2DP, "allowBluetoothA2DP"), (.allowAirPlay, "allowAirPlay"),
        (.defaultToSpeaker, "defaultToSpeaker"),
        (.interruptSpokenAudioAndMixWithOthers, "interruptSpokenAudioAndMixWithOthers"),
    ]

    private static func reason(_ raw: UInt) -> String {
        switch AVAudioSession.RouteChangeReason(rawValue: raw) {
        case .newDeviceAvailable: return "new device"
        case .oldDeviceUnavailable: return "device gone"
        case .categoryChange: return "category change"
        case .override: return "override"
        case .wakeFromSleep: return "wake"
        case .noSuitableRouteForCategory: return "no route for category"
        case .routeConfigurationChange: return "configuration"
        default: return "unknown"
        }
    }
}
