# General rules on development

- Prioritize using existing libraries whenever possible. When you found one, show its strengths and weaknesses and ask if you should include it.
- **A source file over 500 lines is suspect.** Split it unless there is a reason not to, and say the reason.
- **Comments are in English**, everywhere, whatever the language of the surrounding docs.
- **A file header is at most 10 lines.** What the file does, and the one thing a reader would get wrong.
- **A comment inside the body is exactly one line.** No exceptions, no wrapped sentences, no JSDoc paragraphs.
- Long rationale goes to the zone's `docs/` page, and the code points at it.
- **Icons come from `@studio/ui/icons`**, never an emoji or a direct `lucide-react` import — `packages/ui/docs/icons.md`.
- **Documentation is in English**, like the comments — including the pages in every `docs/`.
- **A page carries CHOICES, not mechanism.** What was decided and what forced it. Not a
  walkthrough of what happens when: the code is what says how a thing works, and prose that
  restates it is prose that goes stale without anyone noticing.
- **A page describes the project AS IT IS.** No history: no "it used to be", no account of what
  a change replaced, no migration narrative. Where a past failure is what justifies a decision,
  state the hazard in the present tense and drop the story.
- **Whenever you edit a js/ts file, make sure it passes biome's lint**

These rules hold in every language here — TypeScript and C++. The code keeps the **why** short;
the zone's `docs/` directory is where the justification belongs:

| zone | pages live in |
|---|---|
| a shared package (the UI kit, i18n, http, oidc, env, vite-config and tsconfig included) | `packages/<pkg>/docs/` |
| the k7 frontend | `apps/k7/frontend/docs/` |
| the engine reach | `apps/k7/backend-rs/docs/engine/` (the routes) |
| the DJ desk | `packages/k7-plugins/docs/turntables/` |
| the project shell | `packages/project-shell/docs/` |
| the C++ engine | `engine/k7-runtime/docs/` |
| the native supervisor | `engine/k7-supervisor/docs/` |
| the Android app | `engine/k7-runtime/docs/android.md` |
| the iOS app | `engine/k7-runtime/docs/ios.md` |
| the tray (k7 on Windows and Linux) | `apps/k7-tray/docs/` |
| k7's backend in Rust (the front, the session cookie, the embed) | `apps/k7/backend-rs/docs/` |
| the machine's supervisor run by k7's front | `apps/k7/backend-rs/docs/local.md` |
| the engine's build scripts | `engine/scripts/docs/` |
| the plugin contract | `packages/k7-sdk/docs/` |
| k7's builtin plugins | `packages/k7-plugins/docs/` |
| the functional suite (Playwright, against a running instance or the Android app) | `e2e/docs/` |
| CI and deployment, which belong to no single app | `docs/` |

(Those are notes for developers. `apps/k7/docs/` is the exception: it is the
user-facing documentation the app ships and renders at `/docs`.)

**Never** turn a multi-line comment into a one-liner by joining its lines: that
produces an unreadable 500-character line and can swallow a `biome-ignore` directive,
which silently stops working unless it starts its own comment. Condense to a single
sentence and move the rest to a doc page.

`npm run lint:comments` enforces the three limits above — header length, body one-liners, and
no accented prose where English is expected. It is **non-blocking**, like `lint:dead`: it reports
a debt that predates the rule and shrinks as files are touched. New code has no excuse.

# This repo

`studio` holds the music/stage apps. **Read `docs/architecture.md` before working in an area you
don't know, or touching packaging and deployment**: it is the full map of every app and package,
the production image, and the repo-wide pages in `docs/`. The short map:

| dir | what it is |
|---|---|
| `engine/k7-runtime` | C++ audio engine: receives a compiled `ProjectDoc` over WebSocket, plays it; never parses song files (its own `AGENTS.md`) |
| `engine/k7-supervisor` | C++ supervisor of the engine machine: starts and watches the engine, holds the tunnel, runs the jobs its backend asks (its own `AGENTS.md`) |
| `engine/k7-android`, `engine/k7-ios` | the engine and supervisor on a phone, with k7's backend bundled (`engine/k7-runtime/docs/android.md`, `ios.md`) |
| `engine/scripts` | builds the engine and supervisor, packages the CLI archive and the tray's release files |
| `apps/k7` | the text-first DAW — frontend (React/Vite) + backend (Express: prefs, the file hub, AI organize; the session and the engine registry are `backend-rs`'s Rust front; k7 runs on devices only, no hosted app) (its own `AGENTS.md`) |
| `apps/k7/backend-rs` | k7's backend in Rust, phase by phase: the front that proxies what is not ported yet (`docs/rust-backend-plan.md`) |
| `apps/k7-tray` | k7 on a Windows or Linux computer (a Mac runs the iOS app, Designed for iPad): a windowless Tauri app in the system tray that runs k7's backend on `127.0.0.1:18789`, which runs the machine's supervisor and engine; k7 opens in the browser. Built locally (`apps/k7-tray/docs/`) |
| `packages/` | `@studio/*`, consumed as raw TypeScript through `exports` subpaths (a bare deep import fails on purpose); four are published (`docs/publishing.md`); the UI kit (`ui`), `i18n`, `http`, `oidc`, `env`, `vite-config` and `tsconfig` are k7's own, with no other consumer |
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
- `npm run lint:panels` — a panel's `bind:` against its plugin's real parameters; needs the plugins installed, not in `npm run lint` (`packages/song/docs/panelBinds.md`).

# Developing on the Windows machine

**Windows only** — on macOS and Linux, edit and build in the same world and run `npm run dev`.

**Two checkouts, and only one of them runs anything.** Agents edit in the WSL clone `~/Src/k7`
(ext4, its own Linux `node_modules`) — git, biome, vitest, tsc and the lints are 3–50× faster
there. **Every build and every dev server belongs to Tilt, in the Windows checkout**
(`npm run dev` = `tilt up`, UI at http://localhost:10350): never build by hand, and never run Tilt,
a dev server or the engine build from WSL. Branch from `win/main`, then `git push win <branch>`;
the branch lands in the Windows checkout, where it is merged and where Tilt runs it. Read the
session with `tilt.exe get uiresources` / `tilt.exe logs <resource>` (`docs/dev.md`, `docs/remote.md`).

**One exception: a Rust crate that touches no audio device builds and tests in WSL** (`rustup` stable with
`clippy`, `rustfmt` and `wasm32-unknown-unknown` in `~/.cargo`; `export PATH=$HOME/.cargo/bin:$PATH`; the
target folder named with `--target-dir ~/.cache/<name>`, never inside the tree). WSL has no sound card, so
the engine, the supervisor and any test that opens an audio device stay on Windows.

**Never use PowerShell for text in a tracked file**: it silently corrupts UTF-8 and drops accented
arguments. Write the script with the file tool and run `bash script.sh`; edit files with the edit
tool. PowerShell is for short ASCII commands only (`git status`, `npm run test`) — `docs/dev.md`.

Don't create Pull Request, only use branches and woktress

**A session on the Mac follows `docs/agent-playbook.md`**: the agents, the merges into local `main`, the
local builds, the Rust that must hold on Windows and Linux, and the CI rounds.

**A worktree needs its own `node_modules` before any other step**, even for a docs-only change: an empty
tree breaks biome, the `@studio/*` imports and the build. In a WSL worktree `node scripts/agent/worktree-init.mjs`
builds it (a hook runs it once `scripts/agent/claude-settings.json` is installed as `.claude/settings.json`);
on Windows `npm ci` does. **Never run `npm install` INSIDE a workspace folder** (`packages/ui`…): it writes a
nested `node_modules` whose React shadows the root's, and the dev server then dies on an Invalid hook call (the
frontend's `vite.config.ts` dedupes React as a second line of defence). The tooling and the guard's limits: `docs/agents.md`.

**Before committing, run `npm run check:changed`**: biome on the changed files, the blocking lints, and
typecheck + test of the workspaces the branch touches (`--dependents` for the ones importing them).

# Merging a branch

A finished branch is merged by the agent, always the same way, without looking for another one. One agent
merges at a time, so nobody rebases onto a `main` about to move:

1. **Check first, without the lock**: rebase on a fresh `win/main` and run `npm run ci:local` (plus the gates
   your branch's area needs). A generated file (the generated files, `.gitattributes`' `merge=rebuilt`) is never
   merged by hand: git keeps one side, `npm run rebuild:generated` writes it from the merged sources.
2. **Then take the merge lock** (`node scripts/agent/merge-lock.mjs take <branch>`; it waits while another
   agent merges; one held past 45 minutes counts as abandoned) and rebase again on what landed meanwhile. If
   those new commits touch none of your files nor their neighbours, `npm run check:changed` is the whole
   re-check under the lock; otherwise re-run `ci:local` there. Hold the lock minutes, not a test run.
3. **Commit, `git push win <branch>`, then leave the worktree** (ExitWorktree, `keep`): an isolated session
   cannot run the Windows git below.
4. **Merge in the Windows checkout**, with its own git, never WSL's git on `/mnt/c` (it rewrites the
   index over 9p): `git.exe -C 'C:\Users\flb\Src\k7' merge --no-ff <branch> -m "Merge branch '<branch>': <summary>"`,
   the summary being the branch's commit subject without its `k7: ` prefix. Leave the checkout's own
   uncommitted files alone; on a conflict, abort (`merge --abort`), rebase the branch on `win/main` in WSL, push, retry.
5. **Fast-forward WSL**: `git fetch win && git merge --ff-only win/main` in `~/Src/k7`, then **give the lock
   back**: `node scripts/agent/merge-lock.mjs give <branch>`.
6. **Restart the front, the backend and the frontend in Tilt together**: `tilt.exe trigger k7-front`,
   `k7-backend` and `k7-frontend` (each with `--port 10350`; plus any other resource the change touches, and
   `"(Tiltfile)"` when a `dev` script was added or removed) — never one alone, which leaves processes of two
   revisions talking — and not while a suite targets the dev session; then check `tilt.exe logs <resource> --port 10350`.

# Building and testing on Windows from the WSL clone

AGENTS.md's rule stands: nothing is built IN WSL but the pure Rust crates it names, and Tilt's checkout `C:\Users\flb\Src\k7` is never
built by hand nor switched to another branch (Tilt watches it and would rebuild the running session).
What an agent may do is run a Windows build ON Windows, through WSL interop, in a worktree of its own.
`scripts/agent/win-verify.sh <branch> <command...>` does the steps below in one call, from the main session:

- **The verify worktree is `C:\Users\flb\Src\k7-verify`**, outside Tilt's tree. A branch reaches it by
  `git push win <branch>`; create it once with `git worktree add --detach C:/Users/flb/Src/k7-verify <branch>`
  run from `C:\Users\flb\Src\k7`, then `npm ci` (25 s) inside it.
  Its build folders are its own, so it never fights Tilt's Ninja state; `buildLock.mjs` caps the compilers.
- **Every Windows step is a script FILE run by Git Bash**: write it under
  `C:\Users\flb\AppData\Local\Temp\k7-verify\` (`/mnt/c/...` from WSL), then run
  `/mnt/c/Users/flb/AppData/Local/Programs/Git/bin/bash.exe -l script.sh > step.log 2>&1` and read the log.
  Never PowerShell, never `cmd.exe` with a command line (UNC cwd warnings, and the encoding rules above).
- **A session isolated in a WSL worktree cannot run Windows commands** (the guard cannot prove they stay in
  the worktree): commit, push to `win`, leave the worktree, and drive Windows from the main session.
- **Restart the Tilt session's resources yourself, from WSL**, once a change has landed in the Windows
  checkout — never leave it to the user. `tilt.exe` is on the PATH and talks to the running session:
  `tilt.exe trigger <resource> --port 10350` restarts one (`k7-tray`, `k7-backend`, `k7-frontend`…);
  `tilt.exe trigger "(Tiltfile)" --port 10350` re-reads the task list, which a `dev` script added or removed
  needs (a resource is only created or dropped then); `tilt.exe get uiresources --port 10350 -o name` lists
  them and `tilt.exe logs <resource> --port 10350` reads one's output, to confirm the restart came up.
- **Git Bash rewrites `/flag` arguments into paths**: `/m:6` reaches MSBuild as `m:6`. Use CMake's portable
  `cmake --build <dir> --config RelWithDebInfo --target <t> --parallel 6`, or `MSYS_NO_PATHCONV=1`.
- **What fails on this machine whatever the branch** — check `main` before blaming a change:
  `k7_supervisor_tests`, which dies in `HostLock_test.cpp` (exit 127) so the files after it never run.
- **A C++ test binary that crashes loses its `ok`/`FAIL` lines** (stdout is buffered into a file). To see
  how far it got, add `std::setvbuf(stdout, nullptr, _IONBF, 0);` at the top of `main()` in the verify
  worktree only, rebuild, run, then `git checkout --` the file. `winpty` does not help without a tty.
- **A new test must hold on Windows too**: no file symlink without Developer Mode (`EPERM`), no POSIX mode
  (a file reads `0o666`) — guard those with `process.platform === "win32"`, as `atomicWrite.test.ts` does.

## Rust builds and the CI's checks

How a build stays fast and where a branch is checked — `docs/rust-builds.md`, `docs/release.md`:

- **A target folder is named with `--target-dir`, never `CARGO_TARGET_DIR`**: sccache hashes every `CARGO_*`
  variable, so the environment variable makes every compile miss. Our scripts set `RUSTC_WRAPPER` themselves
  (`engine/scripts/sccache.mjs`); an ad-hoc cargo script adds `eval "$(node engine/scripts/sccache.mjs dev --export)"`, an agent's
  `eval "$(node engine/scripts/sccache.mjs agent --export)"` (four jobs, line tables only).
  Never `sccache --stop-server`: the servers are shared.
- **A branch is checked on the Mac**: no CI job releases, and Windows is set aside until the user brings it back
  (`docs/agent-playbook.md`). A milestone ends with `bash scripts/agent/gate.sh --milestone` (the quick
  checks, `check:changed`, `mac-checks.sh`); before a merge, `gate.sh` adds `win-clippy.sh` and `android-check.sh`, and
  `npm run ci:local` runs on `main` after merges. Never push `main`.
- **The web and conformance suites run on the Mac's own e2e beds, never in CI**, two at once (`engine/scripts/ci/e2e-macos.sh`, `e2e/docs/e2e.md`).
- **A test server listens on `127.0.0.1`**, never on every interface: on macOS another process's loopback bind
  shadows it.

## Android

From the verify worktree's `engine/k7-android`, under Git Bash, with
`JAVA_HOME="/c/Program Files/Android/Android Studio/jbr"` (JDK 21; Gradle refuses newer) and
`ANDROID_HOME=/c/Users/flb/AppData/Local/Android/Sdk` (NDK `29.0.14206865`):

- **Kotlin and the bundled backend only** — what a change outside the C++ needs, about a minute:
  `./gradlew.bat --no-daemon :app:stageBackend :app:compileE2eKotlin -x buildEngine -x buildSupervisor -x stageEngine -x stageFaustLibs -x stageTermuxTools`.
  `preBuild` depends on `stageEngine`, so without the `-x` it first compiles the whole engine for the phone.
- **`stageTermuxTools` fails under Git Bash** (`tar: Cannot connect to C:`): its GNU tar reads `C:` as a
  host. The full APK (`gradlew :app:assembleE2e`) runs from a Visual Studio developer prompt (`vcvars64`,
  Windows' own tar first on the PATH), as `engine/k7-runtime/docs/android.md` says.

## iOS

No Mac here, and no CI job builds the iPhone app: it is built in Xcode on the Mac.
