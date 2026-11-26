# `npm run dev` — the whole studio in one command

```bash
npm run dev                        # every app + the engine, under Tilt
npm run dev -- k7-desktop k7-frontend # only the named Tilt resources
npm run dev:desktop                   # only the desktop app and its engine, no Tilt
```

## Where it answers

**Loopback.** The session is a DEVICE, as the desktop app and the e2e beds are: `env.yaml`'s `k7-front`
section sets `STUDIO_LOCAL_USER`, and k7 opens on **http://127.0.0.1:5223** with no login — `/auth/me`
hands the page its token, and it answers loopback peers only, so the page is visited on the
workstation itself. Nothing terminates TLS and no proxy is in front: there is no edge to start, no
`npm run caddy`, no certificate to trust.

| host | goes to | |
|---|---|---|
| `127.0.0.1:5223` | k7's dev server | the device's page, the one the session is developed on |

The documentation site is its own Vite entry: `npx vite --config vite.docs.config.ts` in `apps/k7-daw-frontend` serves it at
`http://127.0.0.1:5224/docs/` (`apps/k7-daw-frontend/docs/docsSite.md`). `/api`, `/auth` and the sockets go from Vite to the front on 8789 internally
(`apps/k7-daw-frontend/packages/vite-config/src/index.ts`'s `defineAppConfig` — `backendPort`, `proxyPaths`, `wsPaths`).

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

`env.yaml` gives the dev session a `K7_SESSION_TOKEN` and `STUDIO_LOCAL_USER: "1"` in the `k7-front`
section (`env.example.yaml`), the values the e2e bed writes (`apps/k7-daw-engine/scripts/ci/e2e-macos.sh`). There is
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
(`apps/k7-daw-backend/docs/front.md`) is a cargo build, not a package's `dev` script. It listens on
**8789**, where Vite proxies `/api`, `/auth` and the sockets.

| choice | why |
|---|---|
| `apps/k7-daw-backend/scripts/dev-front.mjs` hands the front env.yaml's `k7-front` section | the install's token above all, and env.yaml is the only place the dev session's settings are |
| a fixed secret, `k7-dev-front`, written in that script | the conformance specs behind the internal seam send it; a dev session is one machine's loopback |
| every host (the dev session, the desktop app, the phone) runs all fifteen native pieces answering: the list is the front's own default (`default_pieces.txt` beside `service/switch.rs`), and no variable switches one off | one behaviour on every host |
| `K7_FRONT_TRUST_HOPS=1` | Vite is the hop before the front, and its `X-Forwarded-Host`/`-Proto` name the page's origin |
| `cargo run --release` | the conformance suite and the pages run against it all day; the first build takes a minute, an edit a few seconds |
| rustc through sccache when the machine has it, in that script and in the desktop app task's cargo builds | a new worktree's first build reuses the dependencies another one compiled; the settings, and how to turn it off, are in [rust-builds.md](rust-builds.md) |
| `K7_ENGINE_CORE=front` in that script | the engine registry, the relay and the tunnel hub (`apps/k7-daw-backend/docs/engine.md`) exist in Rust alone |
| the front's own settings in that script | the front reads env.yaml's `STUDIO_MEDIATEK` (the instance's data folder) and keeps its compiled modules in `~/.k7/front-cache` |
| `K7_FILES=front` in that script | the file hub, `/api/ready` and AI organize exist in Rust alone (`apps/k7-daw-backend/docs/files.md`); the file services and the module libraries' service are the front's own, no node child |
| the page from `apps/k7-daw-frontend/dist` (`apps/k7-daw-backend/docs/last.md`) | Vite on 5223 still serves the page in a browser, so no `K7_DEV_FRONTEND` |
| restarting `k7-front` alone is enough for a Rust edit | the file service is the front's own |

The front's file hub makes an account's runtime on its first request (`apps/k7-daw-backend/docs/files/hub.md`).

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

## The desktop app task — `scripts/dev-desktop.mjs`

It is `apps/k7-daw-desktop`'s `dev` script, which is what makes it the `k7-desktop` resource beside the servers.
It builds the engine, the supervisor, the page and library bundle and the desktop app, then
runs the desktop app on those builds: k7 answers at `http://127.0.0.1:18789`, the desktop app's front runs the
supervisor on `~/.k7`, and the supervisor runs the engine. The machine's engine during a session is
therefore the desktop app's, as it is on a musician's machine (`apps/k7-daw-desktop/docs/app.md`).

The desktop app is pointed at the repo's builds through the overrides its `paths.rs` reads first —
`K7_BACKEND_BUNDLE` for the bundle (the library it seeds) in the desktop app workspace's gitignored `.dev` folder, `STUDIO_ENGINE_HOST_CMD` for the
supervisor build — so nothing is staged. The dev session on `http://127.0.0.1:5223` is a device like the desktop app, and
the machine's engine is the desktop app's: under the desktop app, the machine's loopback k7 is the desktop app's front, which the
supervisor reaches every loopback registry at, so the desktop app's front drops any other loopback entry
when it pins its own (`apps/k7-daw-backend/docs/localEngine.md`).

| choice | why |
|---|---|
| the engine registers at the desktop app's front on 18789, not at the dev session's front on 8789 | the supervisor rewrites a loopback address to k7's one port (`apps/k7-daw-engine/k7-supervisor/src/Url.cpp`) and the desktop app drops any other loopback entry when it pins its own (`apps/k7-daw-backend/packages/k7-local/src/settings.rs`); a dev-only exception would be a second rule in both |
| no device token handed to the engine by the dev setup | a `weburi.pair` writes a credential into the machine's real `engine-host.json`, a person's choice and not a script's |
| `K7_GIT_SECRETS=file` in this task and in `dev-front.mjs`: the git secrets in the account's private file, never the keychain | an unsigned dev build changes signature at each build, and the keychain would ask at every read (`apps/k7-daw-backend/docs/git.md`, "Credentials") |
| `let go of engine … its claim moved on` read as a suite at work | the web and conformance suites claim engines under their own accounts, and a newcomer displaces the holder (`apps/k7-daw-backend/docs/engine/claim.md`): the displaced account's binding lets go |

**The page on 18789 is `k7-frontend`'s Vite, with HMR.** The task hands the desktop app
`K7_DEV_FRONTEND=http://localhost:5223`, and the desktop app's front then streams every request it would have
answered from `apps/k7-daw-frontend/dist` to that Vite instead (`apps/k7-daw-backend/packages/k7-front/src/routes/last/dev_frontend.rs`); `/api` and
`/auth` stay its own. So the local PWA is developed on its real origin — its service worker, its cookie,
its storage, the desktop app's own "Open in Browser" — rather than on a stand-in port. HMR's websocket goes to Vite's own port,
`ws://127.0.0.1:5223`, which is why k7's `vite.config.ts` names it in `server.ws`: left
to the page, a page on 18789 would look for Vite on 18789. With `k7-frontend` down, 18789 answers
a 502 naming it. `K7_DEV_FRONTEND=` (empty) before `npm run dev` brings back the built page.

**Its front is linked into the desktop app**, so a save in the front's crates rebuilds and restarts the desktop app (the table below): the desktop app's own
binary holds the front, and a restart ends the engine with it, since the front holds the supervisor's job
(`apps/k7-daw-engine/k7-supervisor/docs/hostProtocol.md`), which is what keeps a killed desktop app from leaving a ghost engine.

The engine is C++, so it has no watch mode that means anything: there is nothing to hot-reload, only
a binary to relink and a process to restart — and the desktop app has to be stopped for that relink, since
its tree holds the engine running. That is what the task does — watch the sources, rebuild, restart.

| target | watched | rebuilt with |
|---|---|---|
| `k7-runtime` | `engine/`, `server/`, `CMakeLists.txt`, `CMakePresets.json` | `npm run build:win` / `build:mac --workspace k7-runtime` |
| `k7-supervisor` | `src/`, `CMakeLists.txt`, `CMakePresets.json` | `supervisorBuild.mjs` |
| `k7-desktop` | `src-tauri/src/`, `Cargo.toml`, `tauri.conf.json`, and the front's crates it links: `apps/k7-daw-backend/packages/**`, its `Cargo.toml` and `Cargo.lock` | `cargo build` in `apps/k7-daw-desktop/src-tauri`, line tables only for debug info: `CARGO_PROFILE_DEV_DEBUG=2` before `npm run dev` for a debugger ([rust-builds.md](rust-builds.md)) |

| choice | why |
|---|---|
| the bundle built at every start of the task, with no frontend in it | the page is Vite's (above); what is left in the bundle is the `k7-core` seed |
| every desktop app stopped first, an installed one included | it holds port 18789 and `~/.k7`, and its single-instance guard would turn the dev desktop app's launch into a no-op |
| a failed bundle ends the task | a desktop app without its library would only say so on its splash; a red resource says it where the session is watched |
| a save under `apps/k7-daw-backend` rebuilds and restarts the desktop app, the engine with it | the front's crates are in the desktop app's binary (`Cargo.toml` path deps); a front edit seen at once is worth the engine's restart, as K7 REC's Mac loop relaunches its app on every save |

A failed build of any target leaves the desktop app down, and says so — `✖ … the desktop app is stopped, fix and
save again`. The cycle stops the desktop app, the supervisor and the engine BEFORE building, since a running
image cannot be relinked, and starts the desktop app again only once every dirty target has built. The
target that failed stays dirty, so the next save retries it instead of looping on it.

`apps/k7-daw-engine/k7-runtime/tests/` is deliberately not watched: it links a binary of its own, and
rebuilding it would double the wait for something the session never runs.

**The binary run is the newest of THIS machine's presets.** A phone's build (`android-*`, `ios-*`)
lands in the same `build/` folder, the APK being built on the desk it is tested from
(`apps/k7-daw-engine/k7-runtime/docs/android.md`), and is newer as soon as it is built; it cannot run here, so every
lookup that picks a binary to run or to stage skips those presets (`CROSS` in
`apps/k7-daw-engine/scripts/stageEngine.mjs`).

### Why one process owns every watch

**A running image cannot be relinked.** On Windows the file is held open for write, so
a relink against a live binary fails with a confusing linker error, and the engine's
does the same while the supervisor has `k7-runtime.exe` running. The order is
therefore stop → build → start.

And the processes that must be stopped are *the same* for every target: the desktop app holds the backend,
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

A failed build puts its target back in the dirty set and stops there, leaving the desktop app
down. It is **not** retried on a timer: a broken build retried every 500 ms is a fan, not
a signal. Save again and it tries again.

At startup the task rebuilds a target only if there is no binary, or if its sources are newer than
the binary, then starts the desktop app whatever was rebuilt. A session should not pay a cmake run to be
told there is nothing to do, and should not silently run the engine you had before a `git pull`.

### A build that succeeds without replacing the image

MSBuild prints `k7-runtime.vcxproj -> …exe` and exits **0** when a running engine holds the
file, so the session would go on running the code you just changed away from. `buildEngine`
therefore checks the binary's mtime afterwards.

The test is deliberately *"is the binary at least as new as its sources"* and **not** *"did it
change"*. A watcher event can fire with nothing actually edited — Windows reports odd names for
changes under the build tree — and a no-op build is then the correct outcome. Demanding a change
turns that harmless case into a hard failure that leaves the desktop app stopped, which is worse than the
bug being guarded.

The watch roots filter by BASE name, because a recursive `fs.watch` reports a path relative to the
root. Anything unmatched is ignored, which is what keeps the engine's own `build/` output, Cargo's
`target/` — and every editor swap file — from triggering a rebuild. The rebuild order is engine first:
building the slow one first means a broken engine never costs the cheaper rebuilds.

### On exit

`dev-desktop.mjs` stops every desktop app, supervisor and engine on `SIGINT`/`SIGTERM` and again at startup
(`stopAll` in `apps/k7-daw-engine/scripts/devEngine.mjs`), and `dev.mjs` repeats it after Tilt is gone — a
resource may be ended too abruptly for its own handler to run, and an engine left holding its port and
its binary is the one leak worth being sure about. Stopping the desktop app is enough on Windows, where its
job object takes the rest along; the supervisor and the engine are stopped by name anyway, for the
cases where it is not. Every step **throws** rather than calling `process.exit`, so a watcher meant
to outlive a bad build survives one.
