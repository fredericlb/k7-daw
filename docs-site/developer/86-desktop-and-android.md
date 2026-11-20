# K7 DAW and the phone apps

K7 DAW (on a computer) and the Android app are the same idea on two platforms: k7's backend, the supervisor and the engine on the user's own device. Both start the **same backend bundle** with the **same settings table**, `apps/k7-daw-backend/standalone.json`:

| Key | Value | Meaning |
|---|---|---|
| `port` | 18789 | The backend's port on every device (not the dev session's 8789, so both can run). Fixed, because a page's storage and its installed app are both keyed on the origin, port included |
| `env` | `STUDIO_LOCAL_USER=1` | The front's environment; a device answers its own machine only |
| `dirs` | `STUDIO_FILES_ROOT=workspaces`, `STUDIO_MEDIATEK=mediatek`, `STUDIO_PREFS_DIR=prefs/k7`, `STUDIO_HOME=server-home` | Folders relative to the app's state directory |
| `library` | `k7-core` | The library seeded on first use from the bundled copy (`STUDIO_LIBRARY_SEED`) |

The bundle is built by `apps/k7-daw-backend/scripts/stageBundle.mjs`, the page and library included. The page it serves is k7's only page; what a device adds is asked of the backend (`GET /api/local`, `apps/k7-daw-frontend/docs/localApp.md`).

## The supervisor host (`k7-local`)

`apps/k7-daw-backend/packages/k7-local` runs `apps/k7-daw-engine/k7-supervisor` as a child process of k7's Rust front under the desktop app (`apps/k7-daw-backend/docs/supervisorHost.md`). `apps/k7-daw-engine/scripts/stageEngine.mjs` stages the binaries the desktop app and the command-line engine carry.

| File | What |
|---|---|
| `apps/k7-daw-backend/packages/k7-local/src/host.rs` | Spawns the supervisor with no arguments, keeps stdin open for the host's whole life (its closing is the supervisor's quit), parses NDJSON frames, and relaunches a dead supervisor on a 1 s → 30 s backoff, reset after 20 s of uptime. Its stop sends `shutdown`, closes stdin, waits, and kills an engine that outlived the teardown only on the pidfile's proof (`pidfile.rs`). |
| `apps/k7-daw-backend/packages/k7-local/src/protocol.rs` | The frames of `apps/k7-daw-engine/k7-supervisor/schema/hostProtocol.schema.json`, partial on purpose: a field is added when a host first reads it. A snapshot has every list, since a frame may omit an empty one. |
| `apps/k7-daw-engine/scripts/stageEngine.mjs` | `stageEngine({repo, bin})`: copy the newest engine and supervisor builds (Debug last) into a staging `bin/`, with the ONNX Runtime libraries beside the supervisor. `$STUDIO_PREBUILT_BIN` copies an already staged `bin/` instead, which is how CI packages binaries built by another job. |

`k7-local`'s `tests/all/host.rs` drives a fake supervisor (`k7-fake-supervisor`) speaking the same NDJSON.

## K7 DAW (`apps/k7-daw-desktop`, Tauri)

A Tauri app (Rust) with one window, on Windows, Linux and macOS. It runs k7's Rust front in its own process on `http://127.0.0.1:18789`, starts nothing else, and shows that address in its window, on the system's webview (`apps/k7-daw-desktop/docs/desktop.md`, `webviews.md`); the menu's "Open in Browser" opens it in the person's browser. The front runs the supervisor over stdio on `~/.k7` and the supervisor's own port (8765): the desktop app's engine is the machine's engine, the one the command-line engine would be. Everything is on the machine, so it works offline. The choices and their reasons are in `apps/k7-daw-desktop/docs/app.md`; the front's side in `apps/k7-daw-backend/docs/localEngine.md`.

| Where | What |
|---|---|
| `apps/k7-daw-desktop/src-tauri/src/backend.rs` | The one spawn, its environment (standalone.json's, plus `K7_SUPERVISOR_BIN`), readiness, the backoff, the stop (closing stdin, `K7_STDIN_STOP`) |
| `apps/k7-daw-desktop/src-tauri/src/window.rs`, `handlers.rs`, `appmenu.rs` | The window (splash, navigation, close), what the webview may do (links, downloads, permissions) and the window's menu (`apps/k7-daw-desktop/docs/desktop.md`) |
| `apps/k7-daw-desktop/src-tauri/src/job.rs`, `pidkill.rs` | The safety nets: a Windows job object, so a force-killed app takes its tree with it, and a backstop for an engine a hung supervisor left |
| `apps/k7-daw-backend/packages/k7-front/src/routes/local/` | The routes a page reaches it through: `/api/local/engine` (the snapshot as Server-Sent Events, an allow-list of commands), and `/api/local`, the probe that tells a page it is on a machine's own backend |
| `apps/k7-daw-frontend/src/settings/desktopEngine.ts` | The engine settings page (`/settings/engine`) over those routes |

There is no sign-in: a page asks `/auth/me` for its token and is the local user.

### Running and packaging

```sh
npm run stage --workspace k7-desktop    # frontend, backend bundle, engine binaries, pinned node into src-tauri/resources/
npm run dev --workspace k7-desktop      # tauri dev
npm run build --workspace k7-desktop    # the installers
```

`apps/k7-daw-desktop/scripts/stage.mjs` builds the resources: the backend bundle, `bin/` from `stageEngine`, and node. The bundle identifier `org.parapluie.studio.engine-tray` must not change: Windows and Linux key the install on it, and the OS's data folder is named after it. The engine and supervisor must have been built first ([87](./87-engine-build-and-shipping.md)). `cargo test` in `apps/k7-daw-desktop/src-tauri` and `npx vitest run src/localEngine` in `apps/k7/backend` are the unit tests; `apps/k7-daw-desktop/AGENTS.md` gives the behavioural check.

## `apps/k7-daw-android`

A Kotlin app on Capacitor, `arm64-v8a` only, Android 10 (API 29) and up.

### Native parts

Android only lets an app execute files from its native library directory, so both native programs ship as `lib*.so`:

| Shipped as | Built from | How it runs |
|---|---|---|
| `libk7runtime.so` | `apps/k7-daw-engine/k7-runtime`, preset `android-arm64`: a **shared library** with `apps/k7-daw-engine/k7-runtime/server/android/EngineJni.cpp` and no `main()` | Loaded in the app's process. `EngineBridge.kt` sets the environment, initialises JUCE, and calls `nativeServe(port)` on a thread that prepared a `Looper`, which becomes JUCE's message thread. Once per process: a stopped engine needs a new process. |
| `libk7supervisor.so` | `apps/k7-daw-engine/k7-supervisor`, preset `android-arm64`, an executable renamed | Spawned by `SupervisorHost.kt` with `STUDIO_HOME`, `STUDIO_MACHINE_NAME` (the device name), `STUDIO_INSTRUMENTS=all`, `STUDIO_PORTS` and `STUDIO_EXTRA_CA_CERTS`. Its `engine` frames start and stop the in-process engine. |

The engine's Android build has no VST3/LADSPA hosting and no Surge XT, and grows its buffer on underruns (up to 40 ms).

### Kotlin

`apps/k7-daw-android/app/src/main/kotlin/se/frediver/k7/`:

| File | Role |
|---|---|
| `MainActivity.kt` | A Capacitor `BridgeActivity`: registers the two plugins, keeps the screen on, starts the backend, asks for `RECORD_AUDIO` then starts the supervisor. Finishing the activity stops supervisor, engine and backend and exits the process. |
| `BackendHost.kt` | Unpacks `backend.zip`, then starts k7's front (`RustFront.kt`) on `127.0.0.1:18789` |
| `SupervisorHost.kt` | The NDJSON channel (what `k7-local` is to the desktop app's front); pins the local backend first in `registryUrls` |
| `EngineBridge.kt`, `EngineOps.kt` | The in-process engine; LAN address, CA import and forget (which restart the supervisor, which then adopts the running engine) |
| `EnginePlugin.kt` | Capacitor plugin `K7Runtime` for the desktop app page: `state` and `engineExit` events, commands, quit |
| `AppViewsPlugin.kt` | Each studio app in a full-screen WebView of its own, loaded from the connected studio, created once and then only hidden so it keeps its state |
| `StudioBridge.kt` | `window.k7Android` for those views: status bar colour, account, media session for everyone; the engine (k7's settings panel, `apps/k7-daw-frontend/src/settings/androidEngine.ts`) for k7's origin only, checked per message |
| `MediaPlayback.kt`, `MediaPlaybackService.kt` | A media notification, and the one foreground service, held only while a page plays |
| `TrustedCas.kt` | User-picked CA certificates |

### The desktop app page

`apps/k7-daw-android/web/` is a small React app built with Vite and staged as Capacitor's `public/`. `App.tsx` chooses the studio (`studios.ts`: the phone's own first, then the supervisor's list, remembering the last pick) and asks `AppViews` to show k7 over it; `Tray.tsx` and `Pairing.tsx` draw the engine status, studios, pairing and CA controls when no studio is shown.

### Building

```sh
cd apps/k7-daw-android
./gradlew assembleDebug
```

`apps/k7-daw-android/app/build.gradle.kts` wires `preBuild` to: `buildEngine` (`node scripts/build.mjs android-arm64` in `apps/k7-daw-engine/k7-runtime`, with the SDK's NDK), `buildSupervisor` (`apps/k7-daw-engine/scripts/supervisorBuild.mjs` with `STUDIO_SUPERVISOR_PRESET=android-arm64`), `stageEngine`, `stageShell` (the desktop app page), and `stageBackend` (`buildFrontend` then `stageBundle.mjs`, zipped with `standalone.json`, stored uncompressed). This compiles the whole C++ engine for Android; it is a long build.
