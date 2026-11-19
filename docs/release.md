# CI: what runs, and what releases nothing

No CI job builds or publishes a release. An artefact — the Mac app, the iPhone app, the APK, K7 DAW (the
desktop app), an image — is built locally by a developer when one is needed. CI only checks the code, in two
places: OneDev, where the jobs run on the project's own machines, and GitHub's `k7.yml`.

## On OneDev

`.onedev-buildspec.yml` holds the check jobs, named for what they check:

| job | runs on |
|---|---|
| `lint and test` (in a container: `lint:locks`, the engine testbed, the repo's lints and every workspace's tests) | every push to `main` |
| `lint and test macos` (`lint:locks`, the engine testbed and `npm run ci:local` — the container's checks plus `lint:docs` — natively on the macOS agent: `engine/scripts/ci/lint-test-macos.sh`) | by hand |
| `rust checks macos` and `rust checks windows` (clippy and the tests of `apps/k7/backend-rs` under cargo-nextest, natively on each agent: `engine/scripts/ci/rust-checks-*`, [rust-builds.md](rust-builds.md)) | by hand |
| `rust checks` (the same on Linux, in a container whose cache uploads with the `cache-upload-token` job secret) | by hand only |

| choice | why |
|---|---|
| no branch triggers a job | a branch is checked on the Mac (`docs/agent-playbook.md`), and Windows is set aside; the Rust checks keep each branch's target folder on their agent (a week on macOS, two days on Windows) |
| the container's `lint and test` is `main`'s alone | every container job shares the docker executor, and Linux, the image's platform, stays the gate there |
| the two macOS jobs run by hand, on the Mac that also runs the same checks itself | its agent shares that machine's cores |

`npm run ci:local` runs the same checks before a merge, so OneDev confirms rather than discovers. It
cannot run what a developer's machine may lack: the engine core's suites skip without a testbed
(`ENGINE_CORE_BIN`), the Cargo locks need cargo, and the macOS agent's newer clippy has its own lints;
`scripts/agent/mac-checks.sh` covers those on the Mac.

`lint and test` builds `k7-engine-testbed` first, in a `rust:1.97.1-alpine` step with its registry and
target cached, and runs the suites with `ENGINE_CORE_BIN` naming it and `ENGINE_CORE_REQUIRED=1`: the
engine core's suites have no TypeScript to test any more, and a suite that skipped there would pass for
having run nothing (`apps/k7/backend-rs/docs/engine.md`, the testbed). The same step first runs
`cargo metadata --locked` in `apps/k7/backend-rs` and `apps/k7-tray/src-tauri`, so a lock the
manifests outgrew fails there (`lint:locks`, `docs/lints.md`).

**The macOS jobs run on a macOS agent in trees that outlive the run**, `~/k7-trees/...`, driven by
`engine/scripts/ciTree.mjs`: a OneDev run gets a new workspace folder every time, and a build there
would start from nothing. Xcode's clang, Homebrew's CMake, Ninja, sccache and cargo-nextest, Node and
rustup's `cargo` are the agent's.

| choice | why |
|---|---|
| `~/k7-trees`, not `~/k7-ci` | the agent's Rust checks delete the week-old folders under `~/k7-ci` |
| 8 compilers (`CMAKE_BUILD_PARALLEL_LEVEL`, `CARGO_BUILD_JOBS`), not every core | the Rust checks run on the same machine |
| `lint and test macos` on the agent's own Node (26, newer than `.nvmrc`'s 24) and Homebrew's ffmpeg, the testbed in `~/k7-trees/testbed-target` copied into the run's workspace, `ci:local`'s worker counts | the suite passes the same on both Nodes, and the application firewall lets only the agent's own take LAN connections, which the WebRTC tests' ICE uses; another branch's run may rebuild the shared testbed while these suites use it |
| `K7_TEST_NO_LAN_PEER=1` there: the two tests dialling the testbed from the machine's LAN address skip | the agent's application firewall, in stealth mode, drops an incoming LAN connection to an unsigned binary; Linux, `main`'s gate, runs them |
| no ogg case in `decodeWasm.test.ts` there | Homebrew's ffmpeg has no libvorbis, and its own experimental Vorbis encoder makes files the two decoders read apart |

**The Windows job runs on a Windows agent through a Remote Shell Executor**, so nothing is installed
per run: Node, Git, Rust (rustup's `cargo`), sccache (WinGet's) and cargo-nextest
([rust-builds.md](rust-builds.md)) are the agent's, under the account its service logs on as.

A job on a shell executor names no executor and no agent. The executor's **agent selector** picks by
OS (an agent query on `Os`, checked on the Agents page first), and its **job match** takes that
platform's jobs and nothing else — `rust checks windows` on Windows, every job whose name holds
`macos` on macOS — since every other job in the file is a container job and must never land on it.
OneDev tries executors in the order they are listed and uses the first whose job match accepts the
job, so the Docker executor must not accept these either: list the shell executors first, or narrow
the Docker one's match.

## On GitHub

Two workflows remain. `.github/workflows/k7.yml` runs the TypeScript checks of `apps/k7`, the shared
tier and the native supervisor on every push to `main`, every `vX.Y.Z` tag and every pull request,
with no path filter; it gates nothing and publishes nothing. `.github/workflows/caches.yml` prunes
the repository's Actions caches after every `k7` run, and daily.

**`node_modules` is restored whole, not reinstalled** (`.github/actions/node/action.yml`): every
job takes it from a cache keyed on the runner's OS and the hash of `.nvmrc` and `package-lock.json`,
and runs `npm ci` only on a miss, which then saves it. The key is exact, never a prefix: a tree
restored from an older lock would be wrong in ways `npm ci` never checks.

**Only main writes a cache.** A branch, a pull request or a tag restores main's caches and saves
none: a run reads the caches of its own ref and of the default branch, so a branch's entry would serve
that branch only while it counts against the same 10 GB and pushes main's out.

**No entry is ever overwritten**, so the old ones are deleted on purpose. The exact-key caches
(`node_modules`) are saved under a new hash whenever their inputs move; nothing reads an older
generation again, and left alone it stays until eviction, which takes the least recently used entry,
often one still needed. `caches.yml` keeps the newest entry per ref and family — the key without its
trailing timestamp or hash — and deletes the rest. `gh cache list --sort size_in_bytes` shows what the
10 GB holds.

`k7-core` is a plain folder of the
repository, so the checkout brings it; its paths are kept short because Windows refuses a path past
260 characters.
