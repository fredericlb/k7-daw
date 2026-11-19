# Rust builds — the compiler cache, the debug info and the test runner

The two Cargo workspaces, `apps/k7/backend-rs` and the desktop app's `apps/k7-desktop/src-tauri`, are the
slowest builds after the engine's C++: Wasmtime, Cranelift, QuickJS and SQLite compile from source,
and every new worktree or target folder compiles them again. Three choices make that
cheaper. The linker is the platform's own: it is not one of them.

## sccache in front of rustc, wherever it is found

`engine/scripts/sccache.mjs` finds sccache (the `PATH`, WinGet's package folder, Homebrew's and
cargo's bins) and returns `RUSTC_WRAPPER` with the cache's settings. Every cargo build a script of
ours starts goes through it: the dev session's `k7-front` and desktop app (`dev-front.mjs`,
`dev-desktop.mjs`), Android's `buildFront`, the Rust checks and the macOS gate's engine testbed.

| choice | why |
|---|---|
| never required: no sccache, one log line, an uncached build | a machine without it must still build; a `RUSTC_WRAPPER` naming a missing program fails every cargo command |
| set by the scripts, not by a repo-wide `.cargo/config.toml` | a `rustc-wrapper` there would break every machine without sccache |
| a `RUSTC_WRAPPER` already in the environment wins, `RUSTC_WRAPPER=` turns the cache off | a developer's own setting is not replaced by the dev default |
| one server per use, each on its own port: the dev session on sccache's default (4226, its default folder), the lasting trees on 4227 (`~/k7-trees/sccache`), the Rust checks on 4228 (`sccache-rust` beside it) | a server keeps the folder and the size it was started with: one shared server would take whichever settings started it, and a job ending could cut a dev build |
| outside Windows each account's ports moved by `100 × (uid % 100)` (`userPort` in `engine/scripts/sccache.mjs`), a `SCCACHE_SERVER_PORT` already set kept | a server writes its outputs as the user who started it: one account's builds through another's server fail on `Permission denied` |
| 10 GB for the dev cache, 15 GB for the lasting trees (C++ and Rust), 5 GB for the checks: 30 GB on a machine | the disk is shared with the engine's build folders |
| a target folder named with `--target-dir`, never `CARGO_TARGET_DIR` | sccache puts every `CARGO_*` variable in its key: a per-branch `CARGO_TARGET_DIR` would make each branch miss what the others compiled |
| the lasting trees' server left running, to its idle timeout | the engine and the APK builds share it; a stop at the end of one would cut the other |

What sccache does not cache: a workspace crate built incrementally (every workspace member in a
debug build), a `bin`, `cdylib` or `proc-macro` crate, and a compile whose paths changed. What it
saves is the dependencies, which are most of the time.

Measured on a Windows machine (the seconds are rough): `cargo build -p k7-front` in a fresh target folder takes 759 s with an empty cache and 453 s
with a warm one, 73 % of its rustc calls hits; the rest are the workspace's own crates and the
build scripts.

## Lighter debug info in debug builds

Both workspaces set `[profile.dev] debug = "line-tables-only"`: a panic's backtrace still names each
frame's file and line, but a debugger sees no variables. `CARGO_PROFILE_DEV_DEBUG=2` (or `full`) in
the environment gives full debug info back for a session at the debugger, at the cost of rebuilding
everything once. A fresh `cargo build -p k7-front` on Windows leaves a 3.2 GB target folder where full debug info left
7.3 GB, and links less: 708 s against 994 s on the same busy machine.

## An agent's builds

An agent runs `eval "$(node engine/scripts/sccache.mjs agent --export)"` before cargo, where a person's script uses `dev`.

| choice | why |
|---|---|
| `CARGO_BUILD_JOBS=4` and `NEXTEST_TEST_THREADS=8` for each agent (set through `sccache.mjs agent --export`, which `eval` needs; a script reads the plain lines) | four agents at cargo's default each start one compiler per core: the Mac's 15 cores reached a load of 72, and every build stretched |
| the cap fixed, never lowered while something else runs | sccache keys a compile on every `CARGO_` variable: a cap that moves makes the agents miss each other's cache |
| `CARGO_PROFILE_DEV_DEBUG=line-tables-only` | an agent never opens a debugger; the links are shorter and a worktree's target folder, 7 to 20 GB, smaller |
| `dev`'s cache, not a cache of its own | the agents' compiles hit each other's entries, since they all spell the same variables |
| while the e2e bed runs its suites, this user's compilers and the sccache server run at nice 15 (`engine/scripts/ci/e2e-macos.sh`) | a render is timed; the compiles run in the sccache server, so only renicing that server slows them, and a user cannot lower a niceness again: the server keeps it until its idle stop |

## Builds run at background priority on the Mac

`gate.sh`, `mac-checks.sh` and `ci:local` re-exec under `taskpolicy -b` (`scripts/agent/throttle.sh`), so a bed's front, engine and Chromium get the CPU first; an ad-hoc cargo build is prefixed `bash scripts/agent/throttle.sh cargo …`, and `K7_NO_BACKGROUND=1` turns it off. `bed.sh` and what it spawns are never throttled.

## cargo-nextest for the Rust checks

`mac-checks.sh` runs the backend's tests with `cargo nextest run`, and
falls back to `cargo test` with a log line on a machine without it. nextest runs every test in its own
process, all of them at once, where `cargo test` runs the test binaries one after another.

| choice | why |
|---|---|
| nextest, not `cargo test` | on macOS 70 s against 107 s for the 527 tests, on Windows about 290 s against 430 s: the slow binaries no longer wait for each other |
| one nextest setting, `apps/k7/backend-rs/.config/nextest.toml`: the tests named `hostile` or `thousand` run one at a time (`file-churn`); otherwise no test relies on sharing a process with another | those tests make folders of up to 20,000 files; five at once took five cores and woke the Mac's file-event and security agents until the load passed 60, and their time bounds assume they run alone |
| the milestone gate (`mac-checks.sh --narrow`) runs a piece's file-churn tests only when that piece, or a piece that needs it, changed (`scripts/agent/affected-tests.mjs`); every other test always runs, and the merge gate and `ci:local` run them all; `K7_TESTS=all` forces the whole run at a milestone | the serial `file-churn` group is the slow chain of a gate; a piece's own change cannot break another piece's churn tests unless that piece needs it. The table is fail-closed: a core file (`mod.rs` beyond its `mod` lines, the hubs), an unclaimed or new file, `k7-compile` or a workspace file runs everything, since a skipped test is only found at the merge |
| `file-churn` stays at one thread until two at a time is measured over three runs | the churn tests' time bounds assume they run alone |
| `NEXTEST_TEST_THREADS` equal to `CARGO_BUILD_JOBS` (4 on Windows, 5 on macOS) | the agents are shared with the developer and the release builds |
| `nextest.toml` gives the known load flakes `retries = 2`, and the agent gate reruns a failed test alone once more (`scripts/agent/rerun-failed.sh`, docs/agent-playbook.md) | a timed test that passes alone is a loaded Mac, not a regression; the list of retried tests shrinks as each is fixed |
| no `cargo test --doc` beside it | the workspace has no doctest; nextest does not run them, so a crate that adds one adds that line too |

Installed by hand: on the Mac `brew install sccache cargo-nextest`; on Windows the official
nextest binary in `%USERPROFILE%\.cargo\bin` and WinGet's `Mozilla.sccache`.

## Windows-only failures, caught before the push

Windows is set aside: `win-clippy.sh` is what keeps its build compiling, and a failure that shows only
on a Windows run costs a long round, so each check below exists to spend fewer of them.

| choice | why |
|---|---|
| every `cargo test` of the checks runs with `--no-fail-fast`, the browser crates' too | without it a crate stops at its first failing test binary, and the next failure costs another round |
| `scripts/agent/win-clippy.sh` before a merge: clippy for `x86_64-pc-windows-gnu` on the Mac, about a minute | a `cfg(windows)` or `cfg(unix)` path compiles only on its platform: an import used by a Unix-only test is an error on Windows alone |
| the GNU target, not MSVC | it needs no Windows SDK; clippy checks without linking, so the ABI does not matter |
| llvm-mingw in `~/.cache/llvm-mingw`, unpacked from its macOS release tarball, not Homebrew's `mingw-w64` | `ring` and `rquickjs-sys` compile C for the target; the tarball needs no admin rights and Homebrew on the Mac belongs to another account |

What the script cannot see is a test whose answer differs on Windows (a path spelling, an error
code): that one still needs a Windows machine.

## What the Mac checks skip

| choice | why |
|---|---|
| an area the branch did not touch since `main` is skipped: `backend` (`apps/k7/backend-rs`, the lockfile) and `browser` (the three crates under `packages/*/rust`); a browser change runs both, since `k7-compile` links the song crate (`engine/scripts/ci/areas.mjs`) | a branch in one area paid for the other's clippy and tests on every round |
| a change to the checks' own scripts or the workflows run everything, and so does a base that cannot be fetched | a skip on a doubt would pass a branch nothing checked |
| nextest for the browser crates too | it spreads the test binaries over the cores; none of the three crates holds a doctest, which nextest does not run |
| no incremental build in the native checks (`CARGO_INCREMENTAL=0`), and no folder copied for a new branch | the incremental caches were most of a target folder's writes and sccache caches no crate built that way; sccache brings back the dependencies a copy would, writing only what a build reads |
| the song crate's 131 test files are one binary, `tests/all/main.rs`, each file a module named as it was; `lanes_memory.rs` stays alone | most of a test build was linking 131 binaries (measured with sccache warm: a clean `cargo test --no-run` 50 s to 42 s, a one-file `src` change 9.2 s to 4.4 s, 146 test binaries down to 4 with the doctests; the same 1445 tests pass, 1446 with `--features ts`); a global allocator that counts live bytes would count every other test's, so that file keeps its own process; a gate is `--test all -- --ignored <module>::` |

## One test binary per backend crate

| choice | why |
|---|---|
| each crate of `apps/k7/backend-rs` with more than one test file has one binary, `tests/all/main.rs`, each former `tests/<name>.rs` a module of the same name and the support folders declared once there; a crate with one file keeps it as it is | every test binary links the whole crate and its dependencies, so each one is a link to wait for and a copy of them on disk: the twelve crates' 82 binaries are 16, the workspace's test executables 105 to 39 and 3.4 GB to 1.1 GB; a test build after touching `k7-js` relinks in 7 to 11 s instead of 10 s, `k7-front` in 4 to 10 s instead of 7 s, within the noise of a Mac loaded to 20-30 by other agents |
| a hostile test over a wide folder has a 20,000-entry variant that always runs and a full-size one, `#[ignore = "scale: ..."]` and named `*_at_scale`, that only the whole `mac-checks.sh` run (the merge gate, `ci:local`) runs, with `--run-ignored ignored-only` | a quadratic walk hides at a tenth of the size and shows at the full one, but 200,000 files written on every gate run flooded the Mac's file-event and security agents |
| a single test is `--test all <name>::`, an ignored one `--test all -- --ignored <name>::` | the module path is the filter, as in the song crate |
| `k7-front`'s `files_health.rs`, `files_hub.rs` and `files_hub_refusal.rs` keep a binary each, beside `tests/files_hub_support/`, which only they use | each fronts one hub over a folder of its own and checks that no node program ran, which one binary per test keeps simple under `cargo test`'s threads |
| `k7-mediatek`'s `analysis.rs` keeps its own binary | it sets `FFPROBE_BIN` for its whole process, which every other test would then read |
| two files never share a scratch folder name nor a process-wide setting, and a file that runs its own binary as a child names its test by module path (`watching::child_writes`) | `cargo test` runs a binary's tests as threads of one process, and the agents use it beside nextest |
| a timed test is not loosened to fold: one that fails only folded keeps its own binary, with its reason in `main.rs`'s header | a bound that passes alone and fails beside other tests is measuring the neighbours |
| a new test file goes in `tests/all/` and is listed in `main.rs`; a file of its own at `tests/` is only for one of the hazards above, named in that header | a new top-level file is one more binary to link at every run |

## The stage gates run on the Mac

`scripts/stage-gates.sh`, one of `npm run ci:local`'s jobs, compares the song crate with the
TypeScript over the golden corpus, the sweeps and the twins.

| choice | why |
|---|---|
| on the Mac, in no CI job | the gates hold on any platform, and cost minutes a CI would pay on every push |
| skipped when nothing they read changed since the merge-base with `main`; `--all` runs them | they read the song crate, its TypeScript, the file service and the corpus; a branch elsewhere gains nothing from their 9 minutes on the Mac |
| the sweeps are never cut | the views and graph gates hold floors on the sweeps' counts: a gate without its sweep fails |
| the TypeScript's outputs (the staged corpus, every dump, the smoke stage) are cached in `~/.cache/k7-stage-dumps/<key>/`, the newest 3 kept; `--fresh` ignores the cache, `--key` prints the key | most port work changes Rust only, and the TypeScript half is most of a cold run |
| the key hashes everything the dumps read: `packages` (but not the song crates' Rust sources, which the dump scripts do not import; their JSON cases, which they read, count), the compiler's `js/`, `k7-core`, `demos`, `e2e/fixtures`, the lockfile, and the sweeps' sizes and seeds; committed, modified and untracked files alike | the dump scripts import most of `packages` through `@studio/*`, so a narrower list would hold a stale dump after an edit it missed; a Rust-only change leaves the key alone, a TypeScript one changes it |
| the twins and the cargo gates are never cached | Rust writes the twins and TypeScript checks them; the gates are what is under test |
| a hit clones the cached trees (`cp -cR`, an APFS clone) and rewrites the folder paths they spell | the first compile repairs the staged tree it reads, and the dumps name the folder they were staged in, the doc dump's own `tree/` among them |
| the cache is stored before any gate runs | a gate that reads the staged tree changes it |

Measured on the Mac with other agents' builds running (load average 25, so the times below move
with it): `--all --fresh` took 15 to 29 minutes (15:03 at best, 28:40 at worst); the same run
with a one-line change under `packages/song/rust/src` and the cache warm took 7:38 at best and
15:03 at worst. What a hit leaves is the gates' own tests, about 12 minutes of test time under
that load, and the song crate's rebuild. The counts every gate prints, and the twin check's, are
identical between a fresh run and a cached one.
