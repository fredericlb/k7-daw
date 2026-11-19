# The tray

The tray is k7 on Windows and Linux. A Mac runs the iOS app, Designed for iPad (`engine/k7-runtime/docs/ios.md`):
no tray is released for macOS. The tray's code still builds and runs there, for the developer's session
(`docs/dev.md`), which is why its macOS branches (menu-bar activation, `open`, the microphone key) stay.

## Names, and the ones a rename may not touch

The binary and the displayed name are `k7-tray`; the engine it runs is `k7-runtime`. Those are build
artifacts, which the next build replaces.

**`org.parapluie.studio.engine-tray`, the bundle identifier, is frozen.** It is the app's identity on Windows and
Linux, and it names
the folder the front keeps its state in (the OS's per-app folder), so moving it reads as a first launch.

Neither may `<state>` (`~/.k7`) move, nor what is under it, nor the `STUDIO_*` variables
(`packages/engine-client/docs/paths.md`).

## k7 on a computer is the web app, opened in a browser

The tray opens no window. It starts k7's front on `127.0.0.1` and opens that address in the person's
browser, where k7 installs as an app (its manifest and service worker are the bundled build's).

| choice | why |
|---|---|
| the browser, not a window of ours | one rendering engine to test against, the one the app is developed in; no tabs, menus or window chrome to maintain |
| `127.0.0.1`, not `localhost` | loopback is a secure context, so the page installs and its service worker runs; `127.0.0.1` is the address every loopback check names |
| the port fixed by `standalone.json` (18789) | a page's storage and its installed app are keyed on the origin, port included |
| offline by construction | the page, the front, the file services and the engine are all on this machine; only a git push or pull needs the network |
| the engine is the machine's own | an HTTPS page cannot open `ws://` to the LAN, and no server relays; a remote engine returns with the app's second-device rework (`docs/inventaire.md`) |

The page tells it is served by a machine's own front from that front (`GET /api/local`), never from a
preload: the bundle is the same one every host serves (`apps/k7/frontend/docs/localApp.md`).

## One process, and the front runs the supervisor

The tray runs k7's front in its own process, on a thread, on the table's port. The front runs the
supervisor over stdio, holds the engine registry, the relay and the tunnel hub, and answers the
file hub and the page (`K7_LOCAL_ENGINE`, `K7_ENGINE_CORE` and `K7_FILES` set to `front`,
`apps/k7/backend-rs/docs/local.md`, `engine.md` and `files.md`); the engine settings page reaches it through same-origin routes behind the session.

| choice | why |
|---|---|
| the front hosts the supervisor, not the tray's own code | the supervisor listens on no port by design (a loopback port is one any page in the browser can `fetch()`, `engine/k7-supervisor/docs/hostProtocol.md`); routes of the front inherit its loopback guard, CORS policy and session |
| the front in the tray's process, not a second program | nothing more to stage, sign or kill; it ends with the tray whatever ends it |
| no node anywhere: the bundle holds the page and the library the front seeds | the front holds the file hub and every account's file service itself |
| the front's code root staged as `resources/code` (`apps/k7/backend-rs/scripts/stageCodeRoot.mjs`, some 700 KB) and named to it as `K7_CODE_ROOT` | the front's actor reads `@meta`, `commands.yaml`, the demos and the sample video at its root, and the tray runs from no checkout (`apps/k7/backend-rs/docs/service.md`) |
| the tray sets no file-service variable: every account's file routes and the libraries' runtime are the front's own (`apps/k7/backend-rs/docs/service.md`, `libraries.md`), the heap budget the embedded front's | a tray holds the local user and the account it is paired with |
| the page from the bundle's `apps/k7/frontend/dist` (`K7_FRONTEND_DIST`), or the dev tray's Vite when `K7_DEV_FRONTEND` is set | the last routes serve it (`apps/k7/backend-rs/docs/last.md`) |
| ready once the front's own `/api/health` answers on the page's port | the page and the registry are the same process; the library's first install gates only the paths that create workspaces (`apps/k7/backend-rs/docs/standalone.md`) |
| said down after three missed health checks, and watched still | a front that stops answering with the tray alive is shown, with Restart as the way out |
| the tray reads its status line from the front, under a bearer it hands over at start | the wording is tested (`k7-local`'s `tray_status`, held to `apps/k7/backend-rs/fixtures/local-vectors.json`); the tray draws and decides nothing |
| `~/.k7` and the supervisor's own port (8765) | the tray's engine is the machine's engine, the one the command-line engine would be; servers list it, and it is what `k7.rocks` plays on through the tunnel |
| the front pinned first in `engine-host.json` | `$STUDIO_REGISTRY_URL` is only a fallback once the person lists another server, and the page needs this engine in its own registry |
| a secret drawn per run for the front's own seams | the internal API answers only a caller that holds it |
| the front stopped first on a quit or a restart | a request in flight then fails at once instead of waiting on a front going away |

## Why Tauri

The tray needs an icon, a menu and a dialog, and no webview at all. Tauri gives those in a few megabytes of
Rust, where Electron would add about 300 MB and a Chromium for a window nobody opens. Rust also gives the
Windows job object directly (`job.rs`).

## The microphone on the developer's Mac

On macOS the microphone grant belongs to the responsible app: the engine records through the front and the
supervisor the dev tray launched, so the tray asks, and its Info.plist carries `NSMicrophoneUsageDescription`.
Without it macOS hands the engine silence: the take is written, all zeros.

## Updates

The tray keeps itself current from `k7.rocks/app_downloads/tray/<channel>/latest.json` with
tauri-plugin-updater (`docs/app-downloads.md` at the root).

| choice | why |
|---|---|
| tauri-plugin-updater, not our own download | it installs on Windows and the AppImage, and checks every update's signature |
| signed, with the key in CI secrets | the upload token alone would let whoever holds it ship code every tray runs; the public key is built in (`K7_UPDATER_PUBKEY`), and a build without it has no updater rather than one that trusts anything |
| the channel from the release stamp | a tag follows `release` and takes a higher tag; a snapshot follows `snapshot`, which only ever holds `main`'s latest build, and takes any other one; `dev` asks nothing |
| told once per build, the menu's check always answers, a switch to turn it off | as the phone's check does; kept in `updates.json` beside the front's state |
| the front stopped before the installer runs | the installer replaces binaries the front and the engine are running, and a graceful stop deregisters the engine |
| a `.deb` is told, not updated | it belongs to apt; Download opens the page |
| a debug build never updates, and its menu has no update entry | it is the dev session's tray (`scripts/dev-tray.mjs`): an update would replace the binary cargo just built, whatever stamp the environment leaked into it |

## Quitting

The front's stop stops the supervisor first, which deregisters while the engine is still up, then stops the
engine. `pidkill::backstop` then reaches an engine a hung
supervisor left, since an engine that survives the app holds the audio device and is adopted by nobody next
launch. SIGTERM and SIGINT quit the same way: launchd stops a login item with SIGTERM.

The backstop kills only what the pidfile **proves** is the engine: the pid on its first line, then the
`start=` and `exe=` the supervisor writes beside it, checked against what the OS says of that pid now — the
verdict of `engine/k7-supervisor/src/PidFile.cpp`, which the tray reads through `k7-local`'s copy
(`apps/k7/backend-rs/crates/k7-local/src/pidfile.rs`, `apps/k7/backend-rs/docs/supervisorHost.md`). A pid is a
number the OS reuses, and a kill of the wrong tree is worse than an orphan. A file it cannot prove (a bare pid,
an OS that would not say) is left in place with its process, for the supervisor's next start to judge; a file
whose process is gone, or is not the engine, is removed.

A tray that ends without its stop (killed, crashed) leaves nothing behind on Windows either:

| choice | why |
|---|---|
| the tray joins a kill-on-close job object of its own at start, before it starts anything (`job.rs`) | everything it starts inherits the job: the front's own children, which no host hook reaches (the hub's file and libraries' services, a render's chromium, ffmpeg, the jsdsp jail). The supervisor's own job, held through `STUDIO_HOST_PID`, covers it and the engine alone |
| the browser and Notepad are started out of the job (`CREATE_BREAKAWAY_FROM_JOB`, the job allowing it), and inside it when the tray's own parent forbids leaving | a browser the tray started is the person's: quitting the tray must not close their tabs |
| kill-on-close is turned off before the updater runs the installer and restarts the tray | both must outlive this process; the front was stopped gracefully just before |
| `K7_TRAY_SCRATCH` (a folder) and `K7_TRAY_PORT` run a debug tray on data and a port of its own, with no single-instance lock | a test of the tray's own processes, such as killing it and counting what survives, must not touch the person's tray, its data or its pairing |

## When the front will not come up

Every state the tray can end in has a way out short of quitting it: the menu's **Restart k7** stops the
front as Quit does and starts it again.

| choice | why |
|---|---|
| a port that answers is waited for, with a backoff, for 30 s | a tray relaunched at once finds the previous front still stopping; only an answer that outlasts that is another program's |
| a front silent for 90 s is shown down, and still watched | a spinner that never ends tells nothing; the status says to look at the logs or restart, and turns ready if the front answers late |

## Launch at login

The autostart plugin: a login item on Windows, an XDG autostart entry on Linux.

## The merged log

`<state>/.studio.log` is the supervisor's file, which the web Logs panel reads by byte offset. The tray
truncates it once per launch, the one process that knows a session began, and files the front's output there
under `[front]`; the tray writes its own notes under `[host]`. The format, `HH:MM:SS [tag] text`,
is that panel's contract.
