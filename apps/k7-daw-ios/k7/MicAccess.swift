// MicAccess.swift — the microphone, asked at the first launch before the engine starts, so a grant opens its inputs
// at once; later, whatever needs it (a page's getUserMedia, a track armed) asks again if still undecided. A grant
// that comes after the engine started reaches it at the NEXT start, since inputs are opened once with the device.
// A refusal iOS remembers is not asked again: the app's Settings are offered. See apps/k7-daw-engine/k7-runtime/docs/ios.md.

import AVFAudio
import UIKit

enum MicAccess {
    enum Answer { case granted, denied, blocked }

    /// At launch: asks if undecided, then `start` on the main thread whatever the answer; a test run never waits on a prompt.
    static func askAtLaunch(then start: @escaping () -> Void) {
        let undecided = AVAudioApplication.shared.recordPermission == .undetermined
        guard undecided, ProcessInfo.processInfo.environment["K7_TEST_HOOKS"] != "1" else { return start() }
        hostLog("microphone: asked at launch, the engine waits for the answer")
        AVAudioApplication.requestRecordPermission { granted in
            hostLog("microphone: \(granted ? "granted" : "refused") at launch")
            DispatchQueue.main.async(execute: start)
        }
    }

    /// Ask unless already decided; `answer` on the main thread.
    static func request(_ answer: @escaping (Answer) -> Void) {
        let state = AVAudioApplication.shared.recordPermission
        hostLog("microphone: iOS says \(state == .granted ? "granted" : state == .denied ? "denied" : "undetermined")")
        switch state {
        case .granted:
            DispatchQueue.main.async { answer(.granted) }
        case .denied:
            DispatchQueue.main.async {
                answer(.blocked)
                offerSettings()
            }
        default:
            AVAudioApplication.requestRecordPermission { granted in
                DispatchQueue.main.async {
                    answer(granted ? .granted : .denied)
                    if granted { afterGrant() }
                }
            }
        }
    }

    private static func offerSettings() {
        hostLog("microphone: refused by iOS, offering Settings")
        Dialogs.confirm(
            "Microphone",
            "k7 is not allowed to use the microphone. Allow it in Settings, then try again.",
            yes: "Settings", no: "Not now"
        ) { yes in
            guard yes, let url = URL(string: UIApplication.openSettingsURLString) else { return }
            UIApplication.shared.open(url)
        }
    }

    // The engine opened its device without inputs: they arrive with the next start, and quitting is the one restart there is.
    private static func afterGrant() {
        guard EngineBridge.startedWithMic == false else { return }
        if Busy.shared.busy {
            Dialogs.inform("Microphone", "The engine's inputs open at its next start: quit k7 once nothing is playing or recording.")
            return
        }
        Dialogs.confirm(
            "Microphone",
            "The engine's inputs open at its next start. Quit k7 now? Open it again from the home screen.",
            yes: "Quit", no: "Later"
        ) { yes in
            if yes { AppDelegate.quit() }
        }
    }
}
