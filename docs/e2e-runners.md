# The e2e suites on GitHub's Linux and Windows runners

Two workflows, `.github/workflows/e2e-linux.yml` and `.github/workflows/e2e-windows.yml`, run the web
and the conformance suites on a fresh GitHub runner, one job per suite. Both start the bed of
`apps/k7-daw-engine/scripts/ci/e2e-runner.sh`, a single-slot port of the Mac bed (`e2e-macos.sh`):
the engine and the supervisor built from the tree, the Rust front in release, Playwright's Chromium,
a fresh home, then the suite and the native counters (`scripts/agent/bedCounters.mjs`).

## Manual, and no gate

| choice | why |
|---|---|
| `workflow_dispatch` only: no push, pull-request or schedule trigger | k7 is a Mac app for now; a cold run costs an hour or more of runner time per job |
| no required status, nothing in `scripts/agent/gate.sh` or the playbook's merge steps waits for them | a feature is judged on the Mac's bed (`e2e/docs/e2e.md`); these runs say where Linux and Windows stand, they hold nothing back |
| dispatched on a branch with `gh workflow run e2e-linux.yml --ref <branch>` (or `e2e-windows.yml`) | `workflow_dispatch` needs the file on the default branch, then runs any ref's copy of it |

Each run uploads its `e2e-out` folder (logs, the HTML report, the JSON counts, the failed specs'
traces) as an artifact, `e2e-<os>-<suite>`, kept 14 days.

## What the runner forces

| choice | why |
|---|---|
| one job per suite, in parallel, each building for itself | the two suites share no state, and a suite on a 4-core runner must not wait for the other |
| `--retries=1` by default (the `retries` input) | a 4-core runner is as loaded as a busy Mac; a spec passing on retry counts as flaky, one failing twice still fails |
| `release-gcc` on Linux, `windows-msvc-ninja` under MSVC's environment on Windows | the Visual Studio generator ignores a compiler launcher, and Ninja's build goes through sccache |
| sccache on a local folder saved as one cache entry, never sccache's GitHub backend | the backend writes an entry per object, which `caches.yml`'s prune by family would delete as one family |
| the caches (sccache, the engine's dependencies, cargo's registry, Chromium) saved by the web job only, and only on `main` or with `save_cache` ticked | the repository's caches share 10 GB (`docs/release.md`); a branch being fixed ticks it to keep its own runs warm |
| a run-unique 64-hex suffix on the sccache key | `caches.yml` keeps the newest entry per ref and family, the key minus that suffix |
| Linux: PulseAudio's null sink as ALSA's default device; Windows: a virtual cable (`LABSN/sound-ci-helpers`) | the runners have no sound card, and the engine opens an output even when `STUDIO_ENGINE_SILENT` writes silence to it |
| Windows: cl.exe's folder first on `PATH`, short roots (`C:/k7d`, `C:/k7t`…) | Git Bash's `/usr/bin/link.exe` would be the linker; a path past 260 characters is refused |
| Windows: every path handed to a native program in the mixed form (`C:/…`), `MSYS_NO_PATHCONV=1` | MSYS rewrites a `/flag` into a path |
| Windows: the front's cargo runs without sccache | the bed's processes run with the bed's home as `USERPROFILE`, where sccache cannot start |
