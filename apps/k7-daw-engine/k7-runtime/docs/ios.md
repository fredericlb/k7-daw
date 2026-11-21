# The engine on iOS

The same engine, the same supervisor and the same Rust front as on Android (`docs/android.md`), run by the app in
`apps/k7-daw-ios`. The architecture is Android's, part for part. The phone runs its own studio and needs no
server, the supervisor lists the phone at the studios it is connected to, the engine serves the same WebSocket
protocol on port 8765, and the screen is k7, full screen, in a view of its own. What differs is what iOS forbids.

| Android | iOS | what forces it |
|---|---|---|
| the supervisor, a program the app runs (`libk7supervisor.so`) | `k7supervisor.framework`, run on a thread of the app (`apps/k7-daw-engine/k7-supervisor/src/Embedded.cpp`) | no iOS app may run a program of its own |
| the Rust front, `libk7_embed.so`, in front of node | the Rust front alone, `libk7_embed.a` linked into the app, no node (`apps/k7-daw-backend/docs/embed.md`) | no iOS app may run a program of its own, and a JIT-less node has no WebAssembly |
| the file hub starts each account's file service on node | every account's files and the libraries are answered by the front itself (`K7_FILES` on `front`) | no iOS app may start a process |
| the engine, `libk7runtime.so`, on a thread whose Looper is JUCE's | `k7runtime.framework`, on a thread whose run loop is JUCE's | the engine is a library on both phones |
| the waveform measures as WebAssembly, run by Wasmtime's JIT | the same module, run by Wasmtime's Pulley interpreter (`k7-mediatek`'s `waveform.rs`) | Apple allows no executable memory to an app |
| Android's CAs, read from the system's folders | a CA bundle the app ships (`cacert.pem`, Mozilla's set as curl publishes it) | iOS lets no app list its trusted roots |
| foreground services while busy | background audio, released while idle | iOS keeps a running audio app alive, and suspends one that stops |

## The process

Everything is ONE process: the engine, the supervisor, the front and the views.
iOS gives an app no child, so every part that is a program elsewhere is a library here, and each keeps the
interface it has as a program:

- **The supervisor** speaks its NDJSON over two pipes the app holds, not over stdio (`k7_supervisor_run`,
  `SupervisorHost.swift`). Its frames have a descriptor of their own (`sendFramesTo`), because the app's stdout
  is shared with the front and the engine. It installs no signal or crash handler and never exits. Closing the command
  pipe is its quit, as closing stdin is on a computer. Its `engine` requests are the app's to carry out, as on Android
  (`K7_ENGINE_IN_APP`, `src/Platform.h`). It is started again IN the process after a CA change. Each run's loop is
  left allocated, since a `work()` thread may still post to it, and the roots are read again (`forgetRoots`).
- **The front** answers the studio's port by itself, started once by `BackendHost.swift` with the table's
  environment and every switch on `front`: it serves the built page from the bundle, so nothing is behind it. A start
  that fails is the one thing a retry redoes, since one front runs per process. The engine's `.k7` is not the
  front's `standalone` folder, whose files it is sent by their own paths, so `EngineBridge.start` sets
  `$STUDIO_ENGINE_ROOTS` to the workspaces and the instance's data folder (`BackendHost.engineRoots`, from the bundled table):
  outside its roots the engine drops every file (`pathRoots.md`).
- **The engine** opens its device without inputs until the microphone is granted (`STUDIO_ENGINE_INPUTS`). JUCE's
  iOS device asks for the microphone the moment inputs open, and JUCE's `RuntimePermissions` answers "granted"
  on iOS whatever the truth.
- **Spawning anything** — ffmpeg, the engine's `--decode` — fails at once with a reason, in the
  simulator too (`Processes.cpp`). The simulator would let it through, and it must not promise what a phone refuses.

**stdout and stderr go to the system log** (`StdioLog.swift`), subsystem `se.frediver.k7.daw`, with each line keeping
its own tag (`[host]`, `[engine]`, `files |`). An app's own streams reach no Console a device shows. The engine's
iostreams are tagged `[engine]` at their source (`EngineEmbed.cpp`), since the process's stderr is shared with the front
and carries no other mark. `StdioLog.swift` also appends each `[engine]` line to the machine log through
`k7_supervisor_log`, as a child's line is appended on a computer, so the Logs panel and the specs that read the
engine's warnings see them. Android's engine lines still go to logcat only. `STUDIO_HOME` is set before anything
else runs: the supervisor opens the machine log at its first line and keeps that file, while the `log` job reads the one
under `STUDIO_HOME`, so a `[host]` line written earlier (the test hooks') would leave the Logs panel reading an empty file.

| choice | why |
|---|---|
| the front a static library, `libk7_embed.a`, the engine and the supervisor frameworks | a Rust `cdylib` for iOS needs Apple's own linker, and the front has no CMake dependency graph to fold; the C++ parts are the `.so` files Android loads |
| frameworks, not static libraries, for the engine and the supervisor | the linker folds every archive each one links into it, with no list of dependency targets to keep in step; and they are the `.so` files Android loads |
| the CA bundle pinned the same way | the supervisor's mbedTLS needs roots as PEM, and iOS gives none; a studio behind a private CA is the user's to add, as on Android |
| the bundle's files read where they lie, in the app bundle | an app bundle is a folder of real files, so nothing is unpacked: no `backend.zip`, no first-run percentage |
| cleartext to loopback only (`NSAllowsLocalNetworking`) | the phone's own studio is plain http, as on desktop; every other host stays https |
| iOS 17 | `AVAudioApplication`, which says the microphone's state in one call; every iPhone from 2018 on runs it |
| one window scene (`SceneDelegate.swift`); the backend, the supervisor and the engine started by the app delegate | iOS 27 stops at launch an app that has not adopted the scene lifecycle (a `SIGTRAP` in `_UIApplicationEvaluateRuntimeIssueForNoSceneLifecycleAdoption`); the parts are the process's, not a window's, so a scene that reconnects gets a new window over them. `Busy.swift` keeps the app's background and foreground notifications, which UIKit posts as the one scene goes behind and back |

## The views

The shell page is Android's own (`apps/k7-daw-android/web`), built by the same workspace. Its two plugins,
`K7Runtime` and `AppViews`, are registered by the Swift half with the same methods and events
(`EnginePlugin.swift`, `AppViews.swift`). k7 gets `window.k7Ios` (`StudioBridge.swift`), the protocol of
Android's `window.k7Android` under the platform's name. `apps/k7-daw-frontend/src/settings/androidEngine.ts` takes
whichever is there, and the three page scripts both apps inject accept either (`apps/k7-daw-android/app/src/main/assets`).
The page calls the engine, checked by origin on every message, with Android's short list of commands.

Where iOS does a thing itself, the app lets it:

| Android | iOS |
|---|---|
| an export goes to a folder picked with the system folder picker, kept as a security-scoped bookmark (`ExportFolders.swift`, `apps/k7-daw-frontend/docs/export.md`) | the person says where it lands; Files shows any folder they grant, and the grant outlives the launch |
| `download.js` fetching blobs in 512 KiB chunks | `WKDownload`, which takes a `blob:`, `data:` or same-origin URL with the session's cookies; the file lands in Documents/Downloads, which the Files app shows under k7 |
| `back.js` and the Back button | the edge swipe, over the view's own history |
| `CertTrust.kt` checking the leaf's name and dates | iOS's own trust evaluation with the imported CAs as anchors (`TrustedCas.swift`): a wrong name, an expired date or a certificate iOS's TLS rules refuse is refused here too. Android's rule stands: an imported non-CA is trusted as that exact leaf and never as an issuer, read from its DER since iOS has no public API for extensions (`CertTrust.swift`) |
| the window's insets | the safe area; the strip under the status bar is painted the app's top colour, and the status bar's icons follow its luminance |
| a renderer lost: the view destroyed, the shell's Retry makes a new one (`RendererLoss.kt`) | WebKit keeps the view usable, so a view whose content process died reloads, twice in a minute at most; past that the shell page is told with a `loadError` and its Retry reloads, since a page that runs out of memory as it loads would otherwise reload forever |

The microphone goes to k7's own origin only. It is asked at the first launch, and the engine waits for the
answer (`MicAccess.swift`): inputs open once with the device, so a grant given before the start is the one that
needs no restart. A recording app without its microphone is not worth the wait it saves, and the wait happens
once. A run with `K7_TEST_HOOKS` skips that prompt, since nobody answers it. A refusal iOS remembers offers the
app's Settings when something needs the microphone; iOS ends an app whose privacy setting changes, so a grant
given there opens the inputs at the launch that follows. A grant that comes after the engine started reaches it at
the next start, which is the person's to make: the iOS app has no Quit, since App Store review refuses an app that
ends its own process, and a stopped engine cannot start again in the same one.

**A media player to the system**, as on Android: `media-session.js` relays the page's Media Session, and
`MediaPlayback.swift` holds Now Playing and the remote commands. The bar may play through the engine, which
WebKit knows nothing of, so this is not left to WebKit's own media. Every command the page's Media Session can
register has its system command (play, pause, next, seek, skip back and forward, stop), and each one that reaches the
page is one `[host] remote command` line.

**The audio session is exclusive**: the iOS presets define `JUCE_DISABLE_AUDIO_MIXING_WITH_OTHER_APPS`, so JUCE
sets no `MixWithOthers`. Another app's music stops when k7 opens its device, and in exchange k7 is the Now Playing app
with the lock screen's controls; `Busy.swift` lets the device go when the app is idle behind another. JUCE restarts
its device after an interruption (a call, Siri), so `AudioSession.swift` only logs it. Each device open and each route
change is one `[host] audio session` line with the category, the options, `mixWithOthers` and the route.
`TestHooks.swift` (debug builds with `K7_TEST_HOOKS=1`) sends the system's media buttons through the real handler on
the simulator, which has no remote to press them: `xcrun simctl spawn booted notifyutil -p se.frediver.k7.daw.test.media-arm`,
then `...test.remote.<action>`.

## Busy: the hold is the default, and its release is the decision

iOS keeps an app with background audio running for as long as its audio device runs. The engine's device always
runs, so the app would never be suspended: a phone kept awake by silence. So `Busy.swift` decides when to LET GO.
Behind another app, with nothing playing, armed, recording, live or holding a microphone, the engine closes its
device (`k7_engine_suspend_audio`, `EngineHost::suspendAudio`) and iOS may suspend the app. It reopens it from the
setup JUCE holds when the app comes back. The reasons are Android's, from the same two sources: the supervisor's
`sessions[].playing`, and the pages' `busy` messages. The screen is kept on while a page reports anything
(`isIdleTimerDisabled`).

An app suspended behind another drops off every studio until it is back in front: its supervisor sends no
heartbeat while suspended.

## Building

```
node apps/k7-daw-ios/scripts/stage.mjs           # a device; --sim for the simulator on an Apple Silicon Mac
open apps/k7-daw-ios/k7.xcodeproj                 # pick a team under Signing, then Run
```

`stage.mjs` builds the engine's `ios-arm64` preset and the supervisor's, each as an xcframework, builds the front
(`k7-embed`, a static library) and fetches the CA bundle as pinned. It builds the
shell page and lays it out as `cap copy` would, and stages what the front reads: the built page, `k7-core`, its code
root (`stageCodeRoot.mjs`). Every run writes `build/staged/version.xcconfig`, the build number the target reads.
`--only=<part,…>` restages some parts.

| choice | why |
|---|---|
| staging is a step BEFORE Xcode, not a build phase | Xcode plans an xcframework's slices when the build starts, so one staged mid-build is not seen; Android's Gradle has no such phase to fight |
| Capacitor from `capacitor-swift-pm`, exactly 8.5.2 | Capacitor's npm package ships no Swift package; this one ships the same release as checksummed xcframeworks. Kept at the version the page's `@capacitor/core` resolves to |
| the project file written by hand, its sources a folder Xcode keeps in sync | no generator to install, and a new Swift file joins the target by being in `k7/` |
| the app links `-lz` and `-liconv` beside `-lresolv` | libgit2 inside `libk7_embed.a` takes zlib and iconv from the SDK, which the cargo build never links |
| signing left to the developer's own team | no team is committed; `CODE_SIGN_STYLE` is automatic |
| the build number staged as the commit count, never a number in the project file (`docs/versioning.md`) | App Store Connect refuses an upload whose number it has already seen; a fixed `1` would ship once |
| JUCE's `juceaide` built at the Mac's own deployment target (`engine/patches/tracktion.cmake`) | JUCE builds that tool for the build machine in a nested CMake and hands it the preset's deployment target, which under `ios-arm64` is an iPhone floor no macOS has |

## What the App Store asks of the app

| choice | why |
|---|---|
| `k7/PrivacyInfo.xcprivacy`: no tracking, no data collected, a reason for each required-reason API (`UserDefaults`, file timestamps, boot time, disk space) | App Store Connect refuses an upload whose code calls those APIs without a manifest; Swift, the Rust front and the JUCE core call all four, and nothing leaves the device for the developer |
| no Quit on iOS, in k7's settings or the tray | review refuses an app that ends its own process; an engine starts again when the person closes k7 and opens it |
| `ITSAppUsesNonExemptEncryption` false | the app uses only standard protocols (TLS, SSH, signatures) through the system and its libraries, which the export rules exempt; without the key every upload asks |

## TestFlight

`apps/k7-daw-ios/scripts/testflight.sh` stages a device build, archives it in Release and uploads it to App Store Connect
for the team's internal testers.

Set up once:

- a paid Apple Developer Program team;
- an App Store Connect app record for the bundle id `se.frediver.k7.daw`;
- the App ID's iCloud capability for key-value storage, which automatic signing with `-allowProvisioningUpdates` adds;
- an internal tester group on that record;
- an App Store Connect API key with the App Manager role, its `.p8` kept in `~/.appstoreconnect/private_keys/`.

```
K7_IOS_TEAM=<team id> K7_ASC_KEY_ID=<key id> K7_ASC_ISSUER=<issuer id> \
  apps/k7-daw-ios/scripts/testflight.sh [--no-stage] [--export-only] [--allow-dirty] [--dry-run]
```

`K7_ASC_KEY_PATH` names the `.p8` when it is not at the default path, and `K7_GITHUB_CLIENT_ID` is baked into the build
for Sign in with GitHub. `--no-stage` reuses the last staging, `--export-only` writes the `.ipa` and uploads nothing
(no API key needed), `--allow-dirty` builds an uncommitted tree, `--dry-run` prints the `xcodebuild` commands and runs none.

| choice | why |
|---|---|
| internal testing only (`testFlightInternalTestingOnly`) | no Beta App Review, and a build cannot slip to external testers |
| the API key rather than an Xcode sign-in | one command and no Xcode window; the script passes only the key's path, never its content |
| a dirty tree refused | the build number names a commit, and a build from other files would carry a number that is not its own |
| archives kept per build number under `~/.cache/k7-ios-testflight` | the archive holds the symbols a crash report is read with |

## On a phone

The kit is `apps/k7-daw-ios/scripts/device.sh`: a build signed by the developer's own team and reinstalled from the Mac (a free
Personal Team signs for 7 days but cannot carry iCloud's entitlements, which the project now holds, so the team is a paid one). The
increased-memory-limit entitlement is one no team here carries, so a cut runs under the default limit. The test ports are the engine's 18765 and the studio's 18799, never 8765.
The shell page is handed the app's own studio port at document start (`MainViewController.swift`), so the k7 view opens the studio the app
runs, whatever `K7_STUDIO_PORT` set, and never the computer's tray on 18789.

```
node apps/k7-daw-ios/scripts/stage.mjs                       # a device build, no --sim
export K7_IOS_TEAM=<team id>                               # Xcode > Settings > Accounts, or security find-identity
apps/k7-daw-ios/scripts/device.sh all                        # build, install, launch (phone attached, unlocked, trusted)
apps/k7-daw-ios/scripts/device.sh forward                    # iproxy: needs `brew install libimobiledevice`
apps/k7-daw-ios/scripts/device.sh logs                       # the system log to ~/.cache/k7-ios-device/
E2E_IOS_ATTACH=1 npm run test:e2e:ios                      # the suite against the phone, from the repo's e2e folder
```

Specs that drive `simctl` itself (stems, `flows.spec.ts`) do not run attached: what they prove is on the simulator, the checklist's lines are what is left.
`--dry-run` prints each command without running it.

| check | do | look for |
|---|---|---|
| sound | play a song, plug and unplug headphones | audio on the speaker and the headphones, no dropouts; `[host] audio session` in the log |
| lock screen controls | play, lock the phone | Now Playing shows the title; play, pause, next and seek act on the page |
| call or Siri | play, call the phone or say "Hey Siri", then end it | playback pauses at the interruption and resumes or waits as logged, no stuck engine |
| a hardware keyboard's undo and redo | type in the editor, then ⌘Z, ⌘⇧Z and ⌘Y | the typing goes and comes back; the suite presses them in the host's WebKit, never in the app's view |
| pairing | pair the phone with a server | the phone's engine is listed on the server and plays from it |
| local-network prompt | first reach a studio on the LAN | the prompt shows once; after Allow the studio answers, after Don't Allow the app says why |
| downloads into Files | export a WAV | the file is in Files > k7 > Downloads and opens |
| microphone's first prompt | launch fresh, press Allow, then reinstall and press Don't Allow | the prompt shows at launch; allowed: the engine opens 2 in without a restart; refused: 0 in |
| private CA through the picker | import a CA from Files, open an https studio under it | the picker hands the app the file; the studio loads |
| a stem cut on a 4-minute song | cut it in the foreground, then with the app behind | the time, the peak memory (Xcode > Debug Navigator, or the log) and whether the app survives the default limit |
| the 7-day reinstall | after 7 days open the app | it will not start; `device.sh all` again, and the library is kept |

## Proven on the simulator

`e2e/specs/app/flows.spec.ts` drives what the host's WebKit never reaches through `TestFlows.swift`'s hooks
(`notifyutil -p se.frediver.k7.daw.test.flow.<name>`, debug builds with `K7_TEST_HOOKS=1`). Each hook runs the app's own
code, so the spec reads a result from the `[host]` log line that code writes, never from a page.

| flow | the app's code | the line that proves it |
|---|---|---|
| a download | `Downloads.swift`, from a click on `<a download>` of a blob | `download: saved <name>`, and the file in `Documents/Downloads` holds the bytes |
| a private CA | `TrustedCas.importFile`, `vouch` and the views' challenge | `trust: 127.0.0.1 refused` before the import; `vouched for by an imported CA` after, the page shown; `trust: localhost refused` for a name the leaf does not carry |
| the microphone refused | `MicAccess` through the view's `getUserMedia` | `microphone: iOS says denied` (the simulator revoked before launch), `refused by iOS, offering Settings`, the dialog with `Not now` and `Settings` |
| a killed content process | `webViewWebContentProcessDidTerminate` | `lost: reload 1 of 2`, `reload 2 of 2`, then `lost a third time in a minute: no reload` |

The CA is made by `openssl` for each run, and the https server listens on `127.0.0.1` on a free port. The content process
is killed by its pid (`_webProcessIdentifier`, a private selector, in the debug build only). The microphone's first
prompt cannot be pressed from outside, so only the refusal iOS remembers is driven; the checklist holds the prompt.

## What starts a program, and what it answers here

A member that needs ffmpeg, a browser or another program answers as a machine without it does (Android's rule,
`docs/android.md`), and says why. `k7_mediatek::programs::NONE` is true on iOS, so the simulator answers as a phone does
and never runs a program the host happens to have. `e2e/specs/app/noPrograms.spec.ts` asserts each answer on the simulator.

| member | answer on iOS | why |
|---|---|---|
| picture render (`picture render`) | the verb fails: "a picture is not rendered here: it needs a browser started as a program, and an iOS app cannot start a program" | Chromium is a program |
| video probe | 200 with the facts the file's own headers give (mp4, mov, mkv); 415 `unsupported` for what they do not say | ffprobe is a program, the headers are read in Rust |
| video extract | the sound decoded by the waveform module when it opens the container; else the job fails with the decoder's reason | no ffmpeg to fall back on |
| video prepare (optimized copy, proxy) | the background job fails with "needs ffmpeg, and an iOS app cannot start a program"; the clip's field never lands | ffmpeg is a program |
| transcribe | the job fails: "no whisper on this device: an iOS app cannot start a program" | whisper is a program |
| the MP3 share and export | `GET /export?format=mp3` answers 400 "mp3 needs ffmpeg, and an iOS app cannot start a program: use Export WAV"; a share's release carries FLAC | the encoder is ffmpeg |
| the engine's `--decode` | never started: the waveform module decodes, and a call that names the engine fails with the same sentence | the engine is a library here, with no binary to run |
| stem cut | runs in the supervisor (`apps/k7-daw-engine/k7-supervisor/docs/supervisor.md`) | ONNX Runtime is linked in |

## Git on a phone

libgit2 is linked into `libk7_embed.a` with its own transports (`apps/k7-daw-backend/docs/git.md`), device and simulator slices alike. The secrets are in iOS's protected keychain. The checklist, on a phone with a repository host and a second machine:

| check | do | look for |
|---|---|---|
| clone over HTTPS | Share a project, paste an `https://` repository URL, sign in with a token | the workspace appears with its files; the token is kept and a second fetch asks nothing |
| clone over SSH | the same with an `ssh://` URL and a key made or imported in the app | an unknown host's fingerprint is shown and must be accepted once; a changed key is refused |
| commit | edit a song, open the project's history, commit | one entry named for the device; a commit with no change is refused |
| push | commit, then push | the remote has the commit; a remote ahead answers behind and nothing is lost |
| pull with a conflict | change the same lines here and on another machine, push there, pull here | the conflict is listed, both sides are shown, choosing one finishes the merge as one history entry |
| a large file | add a 20 MB WAV, commit, push, then clone it on another device | the repository holds a pointer, the host's LFS holds the file, the clone plays offline after its pull |
| no second TLS or SSH stack | `strings` on the staged library | no OpenSSL and no libssh2 in it |

## What it does not have yet

- **The engine's audio is driven on the simulator only, and with no sound heard.** The session's category and
  Now Playing are proven by log lines there; the lock screen's controls and the sound are heard on a phone only.
  `K7_NO_ENGINE=1` in the launch environment runs the app without the engine
  (`SIMCTL_CHILD_K7_NO_ENGINE=1 xcrun simctl launch`), for a simulator whose audio server does not answer.
- **An offline render (export, freeze, stems) is the engine's own**: the supervisor's `render` job asks the running
  engine (`render` command, `protocol.md`) instead of starting `k7-runtime --render`, and `Busy.swift` counts it as busy,
  so a render continues with the app behind.
- **Not run on a phone**, only on the simulator (below); the kit and its checklist are "On a phone". The
  local-network prompt is compiled, not driven; the flows under "Proven on the simulator" are.
- **TestFlight builds come from the script above, run by hand on the Mac.** No CI job builds the iPhone app, and the
  App Store itself is not submitted to.
- **No in-app updates**: a tester installs a new build from TestFlight, and a build expires after 90 days.
- **Stem separation is on the CPU**, ONNX Runtime linked statically (`apps/k7-daw-engine/k7-supervisor/docs/supervisor.md`, "A cut on a phone"); a cut's time and memory on a phone are not measured.
- **No picture render, `ffmpeg`, whisper or `--decode`**: no app may start a process; each member that needs one says so (above).

## What was measured

On the iOS 26.5 simulator (iPhone 17) on an Apple Silicon Mac, with the real front, supervisor, engine and bundle:

| | measured |
|---|---|
| the front | up on the studio's port, its file routes answered by the front's native pieces with no node, the library seeded, the waveform module compiled to Pulley |
| the engine | its audio device open at 44.1 kHz, 256 frames, 0 in, 2 out in 0.73 s; its tunnel up, its heartbeat at the phone's own registry as "iPhone 17" |
| playback | `measure-broadcast.mjs` through the host's loopback: 20 s of a 4OSC note at a realtime ratio of 1.0002, signal in 3428 of 3428 frames, none dropped, jitter spread 4.3 ms |
| behind another app, idle | the device closed at once and reopened on the return, the stream measuring the same after it |
| the microphone | granted before launch, the engine opens 2 in, 2 out; refused, 0 in, 2 out |
| the view | k7's Projects page from the phone's own studio, as the local user: 5 projects in 1 workspace, the engine indicator live |

## File watching

| choice | why |
|---|---|
| iOS polls its folders (`notify`'s `PollWatcher`) | the `notify` crate has no FSEvents on iOS, and its kqueue holds a descriptor per watched file: a library of 26k files held about 10,000 |
| a library's published versions are not watched, only its `draft/` | a version never changes (`docs/library-versions-plan.md`), so watching it only costs descriptors and scans |
| a missed event is a delay, never a wrong answer | callers walk anyway past a ceiling (`modwatch.rs`), so polling's lateness degrades speed alone |

`apps/k7-daw-backend/packages/k7-file-service/src/watcher.rs` is the one place a watcher is made. The app holds about 130 descriptors
after the suite, against about 13,000 with kqueue.

## iCloud holds the key and the list

Workspaces live in Application Support, as on every platform, and travel by git alone. iCloud carries two small things:
the device's SSH key (iCloud Keychain, `k7-git`'s `secret_ios.rs`) and the list of the workspaces the person added
(iCloud's key-value store, `docs/project-sync-plan.md`). `RoamStore.swift` installs two C callbacks before the front starts
(`k7_roam_install` in `K7Embed.h`), over `NSUbiquitousKeyValueStore`; `k7/k7.entitlements` holds the key-value
identifier `$(TeamIdentifierPrefix)se.frediver.k7.daw`, which the provisioning profile grants.

| choice | why |
|---|---|
| the callbacks pass only keys `ws.<id>`, and capture nothing | a C function pointer cannot capture, and nothing else of the store is the list's |
| the change notification is logged, and the page reads the store through the front every 20 s | iCloud applies a change to the store itself; a read is always current, and no event has to cross to Rust |
| a signed-out device or an unsigned build lists nothing from other devices and keeps its own list | the app works the same with no iCloud, and CI runs unsigned |
