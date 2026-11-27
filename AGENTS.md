# General rules on development

- Prioritize using existing libraries whenever possible. When you found one, show its strengths and weaknesses and ask if you should include it.
- **A source file over 500 lines is suspect.** Split it unless there is a reason not to, and say the reason.
- **Comments are in English**, everywhere, whatever the language of the surrounding docs.
- **A file header is at most 10 lines.** What the file does, and the one thing a reader would get wrong.
- **A comment inside the body is exactly one line.** No exceptions, no wrapped sentences, no JSDoc paragraphs.
- Long rationale goes to the zone's `docs/` page, and the code points at it.
- **New logic is written in Rust; TypeScript draws.** A calculation, a rule, a format or a protocol goes in a
  Rust crate (reached from the page through the WebAssembly module or a backend route), and a type that
  mirrors a Rust struct is generated from it (ts-rs), never written by hand. TypeScript holds the React
  components, the UI kit, the editor and the browser's own APIs — `docs/rust-first.md`.
- **Icons come from `@studio/ui/icons`**, never an emoji or a direct `lucide-react` import — `apps/k7-daw-frontend/packages/ui/docs/icons.md`.
- **Documentation is in English**, like the comments — including the pages in every `docs/`.
- **A page carries CHOICES, not mechanism.** What was decided and what forced it. Not a
  walkthrough of what happens when: the code is what says how a thing works, and prose that
  restates it is prose that goes stale without anyone noticing.
- **A page describes the project AS IT IS.** No history: no "it used to be", no account of what
  a change replaced, no migration narrative. Where a past failure is what justifies a decision,
  state the hazard in the present tense and drop the story.
- **Whenever you edit a js/ts file, make sure it passes biome's lint**
- **`apps/k7-daw-frontend/docs/INTERACTIONS.md` is generated from the interactions Sheet**: change the Sheet, never the md — `apps/k7-daw-frontend/docs/interactionsSheet.md`.

These rules hold in every language here — TypeScript and C++. The code keeps the **why** short;
the zone's `docs/` directory is where the justification belongs:

| zone | pages live in |
|---|---|
| a shared package (the UI kit, i18n, env, vite-config and tsconfig included) | `apps/k7-daw-frontend/packages/<pkg>/docs/` |
| the k7 frontend | `apps/k7-daw-frontend/docs/` |
| the engine reach | `apps/k7-daw-backend/docs/engine/` (the routes) |
| the DJ desk | `apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/` |
| the project shell | `apps/k7-daw-frontend/packages/project-shell/docs/` |
| the C++ engine | `apps/k7-daw-engine/k7-runtime/docs/` |
| the machine's supervisor | `apps/k7-daw-backend/packages/k7-supervisor/docs/` |
| the Android app | `apps/k7-daw-engine/k7-runtime/docs/android.md` |
| the iOS app | `apps/k7-daw-engine/k7-runtime/docs/ios.md` |
| K7 DAW, the desktop app (Windows, Linux, macOS) | `apps/k7-daw-desktop/docs/` |
| k7's backend in Rust (the front, the session cookie, the embed) | `apps/k7-daw-backend/docs/` |
| the machine's supervisor run by k7's front | `apps/k7-daw-backend/docs/local.md` |
| the engine's build scripts | `apps/k7-daw-engine/scripts/docs/` |
| the plugin contract | `apps/k7-daw-frontend/packages/k7-sdk/docs/` |
| k7's builtin plugins | `apps/k7-daw-frontend/packages/k7-plugins/docs/` |
| the functional suite (Playwright, against a running instance or the Android app) | `e2e/docs/` |
| CI and deployment, which belong to no single app | `docs/` |

(Those are notes for developers. `apps/k7-daw-frontend/user-docs/` is the exception: it is the
user-facing documentation the app ships and renders at `/docs`.)

**Never** turn a multi-line comment into a one-liner by joining its lines: that
produces an unreadable 500-character line and can swallow a `biome-ignore` directive,
which silently stops working unless it starts its own comment. Condense to a single
sentence and move the rest to a doc page.

`npm run lint:comments` enforces the three limits above — header length, body one-liners, and
no accented prose where English is expected. It is **non-blocking**, like `lint:dead`: it reports
a debt that predates the rule and shrinks as files are touched. New code has no excuse.

# This repo

`k7-daw` (the K7 DAW project) holds the music/stage apps. **Read `docs/architecture.md` before working in an area you
don't know, or touching packaging and deployment**: it is the full map of every app and package,
the production image, and the repo-wide pages in `docs/`. The short map:

| dir | what it is |
|---|---|
| `apps/k7-daw-engine/k7-runtime` | C++ audio engine: receives a compiled `ProjectDoc` over WebSocket, plays it; never parses song files (its own `AGENTS.md`) |
| `apps/k7-daw-backend/packages/k7-supervisor` | the supervisor of the engine machine, in Rust, a crate of the backend: starts and watches the engine, holds the tunnel, runs the jobs its backend asks (its `docs/`) |
| `apps/k7-daw-engine/scripts` | builds the engine and supervisor, packages the CLI archive and the desktop app's release files |
| `apps/k7-daw-android`, `apps/k7-daw-ios` | the engine and supervisor on a phone, with k7's backend bundled (`apps/k7-daw-engine/k7-runtime/docs/android.md`, `ios.md`) |
| `apps/k7-daw-frontend` | the text-first DAW's page — React/Vite, the one place k7's TypeScript lives: its own source, its `packages/` (`@studio/*`, consumed as raw TypeScript through `exports` subpaths: a bare deep import fails on purpose; most serve the page or the verbs' Worker, and `engine-client`, `env`, `song-strudel`, `tsconfig` and `vite-config` serve only scripts, tests and builds) and `user-docs/`, the documentation rendered at `/docs`; k7 runs on devices only, no hosted app (its own `AGENTS.md`) |
| `apps/k7-daw-backend` | k7's backend in Rust: one process answering the session, the preferences, the file hub, AI organize, the engine registry and the built page; its crates are its `packages/`, `waveform` (the measure, decoders and encoders, built into the backend, the supervisor and, as WebAssembly, the page's `waveform.wasm`) among them (`docs/rust-backend-plan.md`) |
| `apps/k7-daw-wasm` | the Rust crates that build for the browser as WebAssembly, `k7-wasm`'s one module, and are linked by the backend too: `song`, `song-schema`, `song-edit`, `k7x`, `k7-wasm` (the TypeScript halves are the frontend's `packages/`) |
| `apps/k7-daw-desktop` | K7 DAW, k7 on Windows, Linux and macOS (Apple silicon): a Tauri app with one window that runs k7's backend on `127.0.0.1:18789`, which runs the machine's supervisor and engine; k7 shows in its window, or in the browser from the desktop app's menu. Built locally on each OS (`apps/k7-daw-desktop/docs/`) |
| `k7-core/` | the `@k7-core` modules library, the seed every library folder is copied from |
| `e2e/` | the Playwright functional suite (`e2e/docs/`) |
| `docs/` | repo-wide notes: CI, deployment, the dev session, plans, audits |

**`k7-core/` is a published version**: a change to its content raises the `version` in its
`library.k7x` in the same commit, since an installed version is never rewritten
(`docs/library-versions-plan.md`). Keep its paths short (Windows' 260-character limit).

Before deleting code that merely *looks* unused, read `docs/inventaire.md`; before touching the
engine registry, the relay or a gate, `docs/audit-moteur.md`. The repo's own checks (`docs/lints.md`):

- `npm run lint:docs` — blocking: refuses a `.md` or source comment naming a file that is not there.
- `npm run lint:bytes` — blocking: refuses a NUL byte in a tracked source file (ripgrep skips it whole).
- `npm run lint:dead` — knip, non-blocking: dead and duplicated code; build-only entries go in `knip.json`.
- `npm run lint:panels` — a panel's `bind:` against its plugin's real parameters; needs the plugins installed, not in `npm run lint` (`apps/k7-daw-frontend/packages/ui-module-builder/docs/panelBinds.md`).

# Merging a branch

A branch works in its own worktree. To merge it: merge `main` into the branch (a generated file is never
merged by hand: git keeps one side, `npm run rebuild:generated` writes it from the merged sources), run
`bash scripts/agent/gate.sh` until it is green, then fast-forward local `main` to the branch (`docs/agent-playbook.md`,
"Merging"). `main` is pushed to GitHub only when the user says so or by a scheduled task (the nightly run), never after
each change and never by force; never use PowerShell for text in a tracked file.

## Rust builds and the CI's checks

How a build stays fast and where a branch is checked — `docs/rust-builds.md`, `docs/release.md`:

- **A target folder is named with `--target-dir`, never `CARGO_TARGET_DIR`**: sccache hashes every `CARGO_*`
  variable, so the environment variable makes every compile miss. Our scripts set `RUSTC_WRAPPER` themselves
  (`apps/k7-daw-engine/scripts/sccache.mjs`); an ad-hoc cargo script adds `eval "$(node apps/k7-daw-engine/scripts/sccache.mjs dev --export)"`, an agent's
  `eval "$(node apps/k7-daw-engine/scripts/sccache.mjs agent --export)"` (four jobs, line tables only).
  Never `sccache --stop-server`: the servers are shared.
- **A branch is checked on the Mac**: no CI job releases, and Windows is set aside until the user brings it back
  (`docs/agent-playbook.md`). A milestone ends with `bash scripts/agent/gate.sh --milestone` (the quick
  checks, `check:changed`, `mac-checks.sh`); before a merge, `gate.sh` adds `win-clippy.sh` and `android-check.sh`, and
  `npm run ci:local` runs on `main` after merges. `main` is pushed only when the user asks or by a scheduled task, never by force.
- **The web and conformance suites run on the Mac's own e2e beds, never in CI**, two at once (`apps/k7-daw-engine/scripts/ci/e2e-macos.sh`, `e2e/docs/e2e.md`).
- **A test server listens on `127.0.0.1`**, never on every interface: on macOS another process's loopback bind
  shadows it.

## Android

`bash scripts/agent/android-check.sh` builds the Rust front for the phone, the bundled backend and the app's Kotlin (JDK 21
as `JAVA_HOME`, the SDK with NDK `29.0.14206865` as `ANDROID_HOME`; Gradle refuses a newer JDK). It never compiles
the C++ engine: that is `./gradlew :app:assembleE2e` from `apps/k7-daw-android`, as `apps/k7-daw-engine/k7-runtime/docs/android.md` says.
A new test must hold on Windows too: no file symlink without Developer Mode (`EPERM`), no POSIX mode (a file reads
`0o666`) — guard those with `process.platform === "win32"` (TypeScript) or `#![cfg(unix)]` (Rust, as `apps/k7-daw-backend/packages/k7-front/tests/tunnel/hub_confinement.rs` does).

## iOS

No CI job builds the iPhone app: it is built in Xcode on the Mac, and `bash scripts/agent/ios-check.sh` checks its Rust and Swift sides.
