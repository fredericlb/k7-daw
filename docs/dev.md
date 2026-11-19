# `npm run dev` — the whole studio in one command

```bash
npm run dev                        # every app + the engine, under Tilt
npm run dev -- k7-tray k7-frontend # only the named Tilt resources
npm run dev:tray                   # only the tray and its engine, no Tilt
npm run vscode                     # the browser editor at /dev, on its own
```

## Where it answers

**Loopback.** The session is a DEVICE, as the tray and the e2e beds are: `env.yaml`'s `k7-backend`
section sets `STUDIO_LOCAL_USER`, and k7 opens on **http://127.0.0.1:5223** with no login — `/auth/local`
signs the machine's own user in, and it answers loopback peers only, so the page is visited on the
workstation itself. Nothing terminates TLS and no proxy is in front: there is no edge to start, no
`npm run caddy`, no certificate to trust.

| host | goes to | |
|---|---|---|
| `127.0.0.1:5223` | k7's dev server | the device's page, the one the session is developed on |
| `docs.priv.k7.rocks` | k7's dev server | the documentation alone, as `docs.k7.rocks` is hosted; a name for a reverse proxy to forward, `scripts/dev-hosts.mjs` (`DOCS_DEV_HOST`) |

A dev server answers **403** to a Host it was not told about, so that file is what lets the docs
host through. `/api`, `/auth` and the sockets go from Vite to the front on 8789 internally
(`packages/vite-config/src/index.ts`'s `defineAppConfig` — `backendPort`, `proxyPaths`, `wsPaths`).

The **Tilt UI** is local only: `http://localhost:10350` on the workstation.

The session runs under **[Tilt](https://tilt.dev)**: `npm run dev` is `tilt up --port
10350`, so each dev server is its own restartable resource — status, filterable logs
and a restart button in the web UI at **http://localhost:10350**; from a terminal,
`tilt trigger <resource>` does what the button does. A crash paints one row red instead
of taking the session down, which one TUI process could never do. `dev.mjs` itself adds
only two things: it refuses to start when port 10350 is already held (one session at a
time — a second would fight the first for every dev port), and it stops the engine on
the way out. Extra args are handed to `tilt up`, so a resource-name list runs a
subset. The native side is a resource in that list like any other — see below for what
it takes to make it one.

## Signing in is the device's, not a provider's

`env.yaml` gives the dev session a `SESSION_SECRET` and `STUDIO_LOCAL_USER: "1"` in the `k7-backend`
section (`env.example.yaml`), the values the e2e bed writes (`engine/scripts/ci/e2e-macos.sh`). There is
no OIDC provider in the session, no cookie domain and no admin group; a device has no admin. Neither the `Tiltfile` nor any script sets those flags.

## The task list

`scripts/tilt-tasks.mjs` exists for one reason: to derive the resource list (as JSON,
read by the `Tiltfile`) instead of spelling out seventeen entries, so a new app joins by
existing. It runs over every workspace package that has a `dev` script and drops two
kinds, each for a reason:

- **the `apps/<x>` aggregators**, whose `dev` is itself a `concurrently` command over
  their own children. Running the parent *and* the children would start every server
  twice.
- **`tsc --noEmit --watch`**, the packages' typecheck watchers. Six more rows for
  feedback each app's own `npm run dev` still gives you.

The root `build` and `test` scripts stay outside this list — Tilt only owns the dev session.

**`k7-front` is the one row the `Tiltfile` names itself**: the Rust front
(`apps/k7/backend-rs/docs/front.md`) is a cargo build, not a package's `dev` script. It listens on
**8789**, where Vite proxies `/api`, `/auth` and the sockets, and `k7-backend`'s TypeScript moves behind
it on **8790**, loopback only.

| choice | why |
|---|---|
| `apps/k7/backend/scripts/dev-front.mjs` hands the front env.yaml's `k7-backend` section | the front reads the values that backend reads — the session secret above all — and env.yaml is the only place they are in dev |
| a fixed secret, `k7-dev-front`, written in that script and in `k7-backend`'s dev script | a dev session is one machine's loopback; a random one would need both processes started by one parent |
| every host (the dev session, the tray, the phone, the image) runs all fifteen native pieces answering: the list is the front's own default (`default_pieces.txt` beside `service/switch.rs`), and no variable switches one off; a host that still sets a removed one gets one log line | one behaviour on every host |
| `K7_FRONT_TRUST_HOPS=1` | Vite is the hop before the front, and its `X-Forwarded-Host`/`-Proto` name the page's origin |
| `cargo run --release` | the conformance suite and the pages run against it all day; the first build takes a minute, an edit a few seconds |
| rustc through sccache when the machine has it, in that script and in the tray task's cargo builds | a new worktree's first build reuses the dependencies another one compiled; the settings, and how to turn it off, are in [rust-builds.md](rust-builds.md) |
| `K7_ENGINE_CORE=front` in that script, and in no `k7-backend` setting | the engine registry, the relay and the tunnel hub (`apps/k7/backend-rs/docs/engine.md`) exist in Rust alone; the backend reaches the engines over the seam at `K7_FRONT_PORT` |
| the front's own settings in that script, and in no `k7-backend` setting | the front reads env.yaml's `STUDIO_MEDIATEK` (the instance's data folder) and keeps its compiled modules in `~/.k7/front-cache` |
| `K7_FILES=front` in that script, and in no `k7-backend` setting | the file hub, `/api/ready` and AI organize exist in Rust alone (`apps/k7/backend-rs/docs/files.md`); the backend answers the front's handshake that it runs none; the file services and the module libraries' service are the front's own, no node child |
| `K7_LAST_ROUTES=front` in that script, unless the environment sets it (empty turns it off) | node's last routes are the front's (`apps/k7/backend-rs/docs/last.md`): the seam, the `/api` guard, the preferences, the built page from `apps/k7/frontend/dist` as node found it; Vite on 5223 still serves the page in a browser, so no `K7_DEV_FRONTEND`. `k7-backend` keeps running behind the front, asked nothing |
| restarting `k7-front` alone is enough for a Rust edit | the file service is the front's own |
| restarting `k7-backend` alone is enough for a TypeScript edit | the front holds no state of the backend's; a request during the restart is a 502, as the page saw a refused connection before |

The front's file hub makes an account's runtime on its first request (`apps/k7/backend-rs/docs/files/hub.md`).

## The browser editor — outside the session, on purpose

VS Code in the browser at **https://code.priv.frediver.se** (its own origin, opening on
the user's home, terminal defaulting to Git Bash)
is *not* a resource in the dev session: an editor inside the session dies with it,
which remotely means losing the only door at the exact moment you need it. On the dev
box it is the **`studio-vscode` logon task** (see [remote.md](remote.md)); anywhere
else, `npm run vscode` starts it alone. `storybook` (`npm run storybook`, :6006) is
likewise its own command — a gallery you open on purpose, not something to have
running.

### How the editor task works

`scripts/vscode.mjs` runs the `code serve-web` that ships with VS Code 1.82+, so there is
nothing to install and no `code-server` (which on Windows means WSL). Two flags carry the
whole integration:

- `--default-folder`, pointed at the **user's home** — and the integrated terminal
  defaults to **Git Bash**, which the script pins in the server's Machine settings
  (`~/.vscode-server/data/Machine/settings.json`, merged, never overwritten). No
  `--server-base-path`: `code.priv.frediver.se` is a whole origin, so the server serves
  at `/` and the proxy strips nothing.
- `--without-connection-token`, so `https://code.priv.frediver.se` opens with no handshake.

**No auth is a decision, and worth stating plainly** so nobody "fixes" it by accident or
copies the pattern somewhere it does not belong. What is served is a browsable filesystem
*with a terminal*: anyone who can reach `code.priv.frediver.se` gets a shell on this machine. That is
accepted here — a dev-only config on an open-source repo — and nowhere else.
`--host 0.0.0.0`, because the LAN proxy (traefik) is another machine and reaches this
port over the network; the LAN is the trust boundary.

To put auth back, swap `--without-connection-token` for `--connection-token-file <path>`.
`serve-web` then answers `403` to a bare `code.priv.frediver.se` and expects the `?tkn=…` URL it prints
once; it trades that for a `vscode-tkn` cookie good for seven days. The stronger option, if
this ever leaves a dev machine, is a forward-auth rule on the LAN proxy pointed at k7's
backend (which owns `/auth`) so the studio session gates it.

The first run downloads a server build (~100 MB) and answers `202` with a self-reloading
"please wait" page until it is done.

A busy port is reported and treated as **success**. `serve-web` exits 0 when it cannot
bind, so the script checks the port itself — and an editor already serving that URL is
the goal met, which is what lets the **`studio-vscode` logon task** and a by-hand
`npm run vscode` coexist: whichever starts first serves, and the other says so and
exits clean. The case worth knowing: a killed `serve-web` leaves a child holding the
socket, so the port can be busy with nothing useful behind it. The message says as
much, and `Get-NetTCPConnection -LocalPort 8798` finds the process tree to stop.

## Working remotely — `code.priv.frediver.se`, `console.priv.frediver.se` and the logon tasks

The dev session is something you start and stop; the *door* must not be. `npm run
remote:install` registers two Windows logon tasks that survive the session: the
**browser editor** at **https://code.priv.frediver.se** and the **shared shell** at
**https://console.priv.frediver.se** — where `npm run dev` is typically launched when working
remotely. Both are their own origins, routed by hostname on the LAN proxy, and both
start in the **user's home** with Git Bash as the default shell. The whole design —
and why ttyd is not the answer — is in [remote.md](remote.md).

## A NEW `exports` subpath needs the dev server restarted

Every `@studio/*` package is consumed as raw source through `package.json` `exports` subpaths, so
adding a module usually means adding a line there too. Vite resolves those once and caches it:
until the dev server is restarted, the new subpath fails with

```
"./takeClock" is not exported under the conditions ["module", "browser", "development", "import"]
```

which names `node_modules/@studio/<pkg>/package.json` and so reads as a stale install or a broken
symlink. It is neither — the symlink points at the workspace and the field is already there. Save
does not fix it and neither does an HMR round; `tilt trigger <resource>` does. Editing an existing
subpath's file needs none of this: it is the exports MAP being new that the cache is holding.

## The tray task — `scripts/dev-tray.mjs`

It is `apps/k7-tray`'s `dev` script, which is what makes it the `k7-tray` resource beside the servers.
It builds the engine, the supervisor, k7's backend bundle and the tray, then
runs the tray on those builds: k7 answers at `http://127.0.0.1:18789`, the tray's backend runs the
supervisor on `~/.k7`, and the supervisor runs the engine. The machine's engine during a session is
therefore the tray's, as it is on a musician's machine (`apps/k7-tray/docs/tray.md`).

The tray is pointed at the repo's builds through the overrides its `paths.rs` reads first —
`K7_BACKEND_BUNDLE` for the bundle in the tray workspace's gitignored `.dev` folder, `STUDIO_ENGINE_HOST_CMD` for the
supervisor build — so nothing is staged. The dev session on `http://127.0.0.1:5223` is a device like the tray, and
the machine's engine is the tray's: under the tray, the machine's loopback k7 is the tray's backend, which the
supervisor reaches every loopback registry at, so the tray's front drops any other loopback entry
when it pins its own (`apps/k7/backend-rs/docs/localEngine.md`).

| choice | why |
|---|---|
| the engine registers at the tray's front on 18789, not at the dev session's front on 8789 | the supervisor rewrites a loopback address to k7's one port (`engine/k7-supervisor/src/Url.cpp`) and the tray drops any other loopback entry when it pins its own (`apps/k7/backend-rs/crates/k7-local/src/settings.rs`); a dev-only exception would be a second rule in both |
| no device token handed to the engine by the dev setup | a `weburi.pair` writes a credential into the machine's real `engine-host.json`, a person's choice and not a script's |
| `let go of engine … its claim moved on` read as a suite at work | the web and conformance suites claim engines under their own accounts, and a newcomer displaces the holder (`apps/k7/backend-rs/docs/engine/claim.md`): the displaced account's binding lets go |

**The page on 18789 is `k7-frontend`'s Vite, with HMR.** The task hands the tray
`K7_DEV_FRONTEND=http://localhost:5223`, and the tray's backend then streams every request it would have
answered from `frontend/dist` to that Vite instead (`apps/k7/backend/src/devFrontend.ts`); `/api` and
`/auth` stay its own. So the local PWA is developed on its real origin — its service worker, its cookie,
its storage, the tray's own "Open in Browser" — rather than on a stand-in port. HMR's websocket goes to Vite's own port,
`ws://127.0.0.1:5223`, which is why k7's `vite.config.ts` names it in `server.ws`: left
to the page, a page on 18789 would look for Vite on 18789. With `k7-frontend` down, 18789 answers
a 502 naming it. `K7_DEV_FRONTEND=` (empty) before `npm run dev` brings back the built page.

**Its backend runs from source and restarts on save.** The bundle's `bin/backend.mjs` is replaced by an
import of `scripts/devTrayBackend.mjs`, which runs `apps/k7/backend/src/server.ts` under `tsx watch` — a
save in any file it imports, `packages/` included, restarts it. A restart ENDS THE ENGINE: the backend holds the
supervisor's job (`engine/k7-supervisor/docs/hostProtocol.md`), which is what keeps a killed tray from
leaving a ghost engine, so the engine is down a few seconds and the project reloads — the cost a manual
`tilt trigger k7-tray` already had, paid on every save instead of when chosen. A save in `@studio/song`
or `@studio/commands` for the page bounces it too. `K7_DEV_BACKEND=bundle` before `npm run dev` runs the
bundled backend, for a session spent listening. The tray's stdin stays the SHIM's: `tsx watch` reads its
own stdin, for the key that reruns its child, and a backend reading the same pipe for `K7_STDIN_STOP`
never got past its start on Windows. So `tsx watch` gets none, the backend no stop-on-end, and when the
tray closes the shim's stdin the shim ends the whole tree — `taskkill /T` on Windows, where a killed
backend takes its supervisor and engine with it through the job it holds; a SIGTERM first elsewhere. A
backend left behind would hold 18789 for the next tray.

The engine is C++, so it has no watch mode that means anything: there is nothing to hot-reload, only
a binary to relink and a process to restart — and the tray has to be stopped for that relink, since
its tree holds the engine running. That is what the task does — watch the sources, rebuild, restart.

| target | watched | rebuilt with |
|---|---|---|
| `k7-runtime` | `engine/`, `server/`, `CMakeLists.txt`, `CMakePresets.json` | `npm run build:win` / `build:mac --workspace k7-runtime` |
| `k7-supervisor` | `src/`, `CMakeLists.txt`, `CMakePresets.json` | `supervisorBuild.mjs` |
| `k7-tray` | `src-tauri/src/`, `Cargo.toml`, `tauri.conf.json` — and, at the task's START, the front's crates it links: `backend-rs/crates/**`, its `Cargo.toml` and `Cargo.lock` | `cargo build` in `apps/k7-tray/src-tauri`, line tables only for debug info: `CARGO_PROFILE_DEV_DEBUG=2` before `npm run dev` for a debugger ([rust-builds.md](rust-builds.md)) |

| choice | why |
|---|---|
| the backend bundle built at every start of the task, with no frontend in it | the page is Vite's and the backend tsx's (above); what is left in the bundle is the `k7-core` seed and the files the tray reads, and `K7_DEV_BACKEND=bundle` runs it whole |
| every tray stopped first, an installed one included | it holds port 18789 and `~/.k7`, and its single-instance guard would turn the dev tray's launch into a no-op |
| a failed bundle ends the task | a tray without a backend would only say so on its status line; a red resource says it where the session is watched |
| an edit under `apps/k7/backend-rs` does not rebuild the tray mid-session, only at its start | the front's crates are in the tray's binary (`Cargo.toml` path deps), so they decide whether it is stale when the task starts — but watching them would stop and restart the tray, and the engine with it, on every front save. A route added to the front 404s on 18789 until `k7-tray` is restarted |

A failed build of any target leaves the tray down, and says so — `✖ … the tray is stopped, fix and
save again`. The cycle stops the tray, the supervisor and the engine BEFORE building, since a running
image cannot be relinked, and starts the tray again only once every dirty target has built. The
target that failed stays dirty, so the next save retries it instead of looping on it.

`engine/k7-runtime/tests/` is deliberately not watched: it links a binary of its own, and
rebuilding it would double the wait for something the session never runs.

**The binary run is the newest of THIS machine's presets.** A phone's build (`android-*`, `ios-*`)
lands in the same `build/` folder, the APK being built on the desk it is tested from
(`engine/k7-runtime/docs/android.md`), and is newer as soon as it is built; it cannot run here, so every
lookup that picks a binary to run or to stage skips those presets (`CROSS` in
`engine/scripts/stageEngine.mjs`).

### Why one process owns every watch

**A running image cannot be relinked.** On Windows the file is held open for write, so
a relink against a live binary fails with a confusing linker error, and the engine's
does the same while the supervisor has `k7-runtime.exe` running. The order is
therefore stop → build → start.

And the processes that must be stopped are *the same* for every target: the tray holds the backend,
which holds the supervisor, which holds the engine, so stopping one part leaves a binary held. Several
independent watchers would race to stop and restart one tree. So there is one, and a cycle that
needs several rebuilds does one stop, every build, one start.

This is also why the task owns the *build* and not just the restart. A watcher that
only reacted to a changed binary could never fire: while the session is up, nothing can
produce that binary.

### The cycle

Saves are debounced 500 ms and coalesced per target. A target is removed from the dirty
set **before** its build, not after, so a save landing mid-build is not swallowed — it
just queues another cycle. A save landing during a build re-arms the timer, which keeps
retrying until the cycle in flight frees the tree.

A failed build puts its target back in the dirty set and stops there, leaving the tray
down. It is **not** retried on a timer: a broken build retried every 500 ms is a fan, not
a signal. Save again and it tries again.

At startup the task rebuilds a target only if there is no binary, or if its sources are newer than
the binary, then starts the tray whatever was rebuilt. A session should not pay a cmake run to be
told there is nothing to do, and should not silently run the engine you had before a `git pull`.

### A build that succeeds without replacing the image

MSBuild prints `k7-runtime.vcxproj -> …exe` and exits **0** when a running engine holds the
file, so the session would go on running the code you just changed away from. `buildEngine`
therefore checks the binary's mtime afterwards.

The test is deliberately *"is the binary at least as new as its sources"* and **not** *"did it
change"*. A watcher event can fire with nothing actually edited — Windows reports odd names for
changes under the build tree — and a no-op build is then the correct outcome. Demanding a change
turns that harmless case into a hard failure that leaves the tray stopped, which is worse than the
bug being guarded.

The watch roots filter by BASE name, because a recursive `fs.watch` reports a path relative to the
root. Anything unmatched is ignored, which is what keeps the engine's own `build/` output, Cargo's
`target/` — and every editor swap file — from triggering a rebuild. The rebuild order is engine first:
building the slow one first means a broken engine never costs the cheaper rebuilds.

### On exit

`dev-tray.mjs` stops every tray, supervisor and engine on `SIGINT`/`SIGTERM` and again at startup
(`stopAll` in `engine/scripts/devEngine.mjs`), and `dev.mjs` repeats it after Tilt is gone — a
resource may be ended too abruptly for its own handler to run, and an engine left holding its port and
its binary is the one leak worth being sure about. Stopping the tray is enough on Windows, where its
job object takes the rest along; the supervisor and the engine are stopped by name anyway, for the
cases where it is not. Every step **throws** rather than calling `process.exit`, so a watcher meant
to outlive a bad build survives one.

## Two checkouts on the Windows machine — what RUNS stays on NTFS

The Windows checkout is where everything that runs lives: Tilt, every dev server, the MSVC build
and the binaries all run from NTFS, natively. Nothing that runs crosses into WSL, because what
crosses breaks: MSBuild tracks a dependency lowercased, so over the case-sensitive 9p share no
build is ever up to date, and a server reading NTFS from the distro pays 9p on every `readdir`
(a workspace listing goes from about 1 ms to several seconds, and the service starves).

The agent's EDITING clone is `~/Src/k7` in the WSL distro, on ext4, with its own Linux
`node_modules`. It exists for one measured reason: process creation and small-file I/O. In Git Bash
on NTFS, 50 process spawns take 2.8 s, `lint:comments` 3.7 s and one package's vitest 8 s; on
ext4 the same take 47 ms, 0.2 s and 2.5 s. An agent spends its time in exactly those calls.

The two trees meet through git and nothing else, with no mirror:

| | WSL clone `~/Src/k7` | Windows checkout |
|---|---|---|
| edit, git, biome, vitest, tsc, the lints | yes | yes |
| Tilt, dev servers, engine, MSVC | **never** | yes |
| remote | `win` = `/mnt/c/Users/<you>/Src/k7` | — |

The agent branches from `win/main`, commits, and runs `git push win <branch>`: the branch lands in the
Windows checkout, which is where it is merged and where Tilt runs it. `tilt.exe` answers from
the distro through interop, so `tilt.exe get uiresources` reads the session without crossing.

## Shelling out on Windows — from a FILE, never through PowerShell

On the Windows side, editing (claude, nvim…) happens in the `console.priv.frediver.se` tabs,
Git Bash by default ([remote.md](remote.md)).

PowerShell is the wrong tool for anything touching this repo's text, and it fails silently:

- **It corrupts UTF-8.** `Get-Content -Raw` reads as ANSI, so a read-modify-write round trip turns
  `é` into `Ã©` and `—` into `â€"`. The file still parses and the diff still looks plausible, so
  nobody notices — in a source file or in a commit message.
- **Accented arguments do not survive the call**, even in single quotes:
  `bash -lc 'echo "é à ç"'` prints nothing of them — the text is lost between PowerShell and the exe.
- `--%` does not help: it swallows the rest of the line, epilogue included.
- Its parser rejects `&&`, `??`, `?.` and bare `|` in places bash accepts them.

So: **write the script with the file tool, then run `bash script.sh`.** The file tool writes UTF-8
without a BOM, and nothing accented ever passes through PowerShell's argument encoding.

```bash
# .tmp-work.sh — written as a file, then: bash .tmp-work.sh
set -euo pipefail
grep -rn 'médiathèque' packages/song/src | head
```

PowerShell is for short ASCII commands where none of the above bites — `git status`,
`npm run test`. **Never for text substitution in a tracked file**: that is the edit tool's job.
