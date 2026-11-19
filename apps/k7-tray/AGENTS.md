# AGENTS.md — the tray

**K7 DAW, k7 on a computer** (Windows, Linux, macOS on Apple silicon). A Tauri app with one window and a tray icon.
It runs k7's Rust front in its own process on `127.0.0.1:<standalone.json port>`, and starts nothing else. The front runs the machine's supervisor (`engine/k7-supervisor`) and so its engine, and
holds the engine registry. The window shows k7 at that address, on the system's webview; the tray's "Open in Browser"
shows it in the person's browser. The tray shows one status line and the window's way back.

The page has no IPC and no native power: every setting of the engine is a page of k7 (`/settings/engine`), which
reaches the supervisor through k7's front (`apps/k7/backend-rs/docs/localEngine.md`). The host only decides what the
window may navigate to, download and ask for (`src/handlers.rs`).

All Rust lives under `src-tauri/src/`. `docs/tray.md` says what was decided and why; `docs/desktop.md` is the window,
`docs/webviews.md` the three webviews, `docs/macos.md` signing and the DMG.

## Lifetime

```
K7 DAW (Tauri: window + tray)   ← the process the user kills; creates the Job Object
 └─ k7's front (a thread)       ← k7_front::embedded, on the table's port
     └─ k7-supervisor           ← run by the front over stdio (k7-local); its stdin closing is its stop
         └─ k7-runtime
```

- **Windows**: `job::init()` runs before anything spawns; membership is inherited, so a force-killed tray takes
  the whole tree with it.
- **Unix**: the pipes do it. The tray's death closes the supervisor's stdin, which stops the engine.
- **Front stops answering**: said on the status line after three missed checks; Restart starts it again.
- **Quit**: `backend::stop` stops the front, whose stop stops the supervisor (it deregisters first), then
  `pidkill::backstop`.

## Key modules

- **`backend.rs`** — the front, its one environment (standalone.json's, the switches, the supervisor, the bearer,
  the stamp), readiness on the front's own `/api/health`, the stop.
- **`loops.rs`** — the status poll (`/api/local/tray`, under the bearer) and the spinner.
- **`tray.rs`** — the menu. Draws what the front words; computes nothing.
- **`window.rs`**, **`handlers.rs`**, **`appmenu.rs`** (macOS), **`webview_linux.rs`**, **`autostart.rs`** — the window and
  what the webview may do, the Mac's menu bar, WebKitGTK's settings, the login item.
- **`paths.rs`** — the bundle, the supervisor: the staged resources, else the checkout's builds. A miss
  is said on the status line, never a silent empty tray.
- **`updates.rs`** — tauri-plugin-updater on the tray's channel feed (`docs/tray.md`).
- **`job.rs`**, **`pidkill.rs`**, **`logbus.rs`**, **`trayicon.rs`** — the safety nets, the merged log, the icon.

## Build / run

Builds belong to the machine of their OS (a Mac builds the `.dmg`; Windows and Linux builds are the person's), never WSL's (root `AGENTS.md`). `dev` is the dev session's `k7-tray` resource
(`scripts/dev-tray.mjs`, `docs/dev.md`); the installers are `tray:build`, never `build`, which the root `npm run build`
would run.

- `npm run stage --workspace k7-tray` — the frontend, the page and library bundle, the engine (the repo's builds, or
  `$STUDIO_PREBUILT_BIN`), into `src-tauri/resources/`.
- `npm run dev` (Tilt) or `npm run dev:tray` (alone) — builds the engine, the supervisor, the page and library bundle and
  the tray, runs the tray on them, and rebuilds and restarts on save. `cargo build` in `src-tauri` alone is enough
  to run `target/debug/k7-tray.exe` on staged resources.
- `npm run tray:build --workspace k7-tray` — the installers (signed for the updater only with `K7_UPDATER_PUBKEY` and `TAURI_SIGNING_PRIVATE_KEY` both set, `docs/tray.md`); `node engine/scripts/package-release.mjs --kind tray`
  renames them for a release.

## Verify

`cargo test` in `src-tauri`; the front's side is `cargo test -p k7-local` in `apps/k7/backend-rs`.
Behavioural check: start the app, let the window reach k7, open a project and play it; close the window and bring it back from the tray; then quit and confirm the
engine stops. Force-kill the tray and confirm nothing survives: no `k7-supervisor.exe` or
`k7-runtime.exe` of its tree, and the k7 port and the engine's free.
