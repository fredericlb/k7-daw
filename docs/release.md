# CI: what runs, and what releases nothing

No CI job builds or publishes a release. An artefact — the Mac app, the iPhone app, the APK, the
tray, an image — is built locally by a developer when one is needed. CI only checks the code, on
GitHub, and a branch is checked on the Mac first (`docs/agent-playbook.md`): `npm run ci:local`
runs the repo's lints and every workspace's tests, and `scripts/agent/mac-checks.sh` covers what
a developer's machine may lack (the Cargo locks need cargo, the engine core's suites need a testbed).

## On GitHub

Two workflows exist. `.github/workflows/k7.yml` runs the TypeScript checks of `apps/k7`, the shared
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
