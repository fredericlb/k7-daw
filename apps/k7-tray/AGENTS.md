# AGENTS.md — the tray

**k7 on a computer.** A windowless Tauri app in the system tray (Windows, Linux; a Mac runs the iOS app, and builds the tray only for the dev session). It runs k7's Rust front in its own
process on `127.0.0.1:<standalone.json port>` (`apps/k7/backend-rs/standalone.json`); it starts nothing else.
The front runs the machine's supervisor (`engine/k7-supervisor`) and so its engine, and holds the engine registry. The person uses k7 in their browser, at that address, as an
installable web app. The tray shows one status line and opens k7.

It has no webview and no settings of its own: every setting of the engine is a page of k7
(`/settings/engine`), which reaches the supervisor through k7's front (`apps/k7/backend-rs/docs/localEngine.md`).

All Rust lives under `src-tauri/src/`. `docs/tray.md` says what was decided and why.

## Lifetime

```
tray (Tauri, windowless)        ← the process the user kills; creates the Job Object
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
- **`tray.rs`** — the menu. Draws what the backend words; computes nothing.
- **`paths.rs`** — the bundle, the supervisor: the staged resources, else the checkout's builds. A miss
  is said on the status line, never a silent empty tray.
- **`updates.rs`** — tauri-plugin-updater on the tray's channel feed (`docs/tray.md`).
- **`job.rs`**, **`pidkill.rs`**, **`logbus.rs`**, **`trayicon.rs`** — the safety nets, the merged log, the icon.

## Build / run

Builds belong to Windows, never to WSL (root `AGENTS.md`). `dev` is the dev session's `k7-tray` resource
(`scripts/dev-tray.mjs`, `docs/dev.md`); the installers are `tray:build`, never `build`, which the root `npm run build`
would run.

- `npm run stage --workspace k7-tray` — the frontend, the page and library bundle, the engine (the repo's builds, or
  `$STUDIO_PREBUILT_BIN`), into `src-tauri/resources/`.
- `npm run dev` (Tilt) or `npm run dev:tray` (alone) — builds the engine, the supervisor, the page and library bundle and
  the tray, runs the tray on them, and rebuilds and restarts on save. `cargo build` in `src-tauri` alone is enough
  to run `target/debug/k7-tray.exe` on staged resources.
- `npm run tray:build --workspace k7-tray` — the installers; `node engine/scripts/package-release.mjs --kind tray`
  renames them for a release.

## Verify

`cargo test` in `src-tauri`; the front's side is `cargo test -p k7-local` in `apps/k7/backend-rs`.
Behavioural check: start the tray, open k7 from its menu, open a project and play it; then quit and confirm the
engine stops. Force-kill the tray and confirm nothing survives: no `k7-supervisor.exe` or
`k7-runtime.exe` of its tree, and the k7 port and the engine's free.
