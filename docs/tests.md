# The test suite — how it runs, and the two ways it lied

`npm run test` is `scripts/workspaces.mjs`: one `vitest run` per workspace that declares a
`test` script, `--jobs N` at a time, and no cache. A suite can still be **green because it never
ran**, which is the hazard below.

**The default is one at a time, and CI passes `--jobs 4`.** Four is what the suite measurably
holds: 144 s sequential, ~62 s at four, green on every run, on a 12-thread machine. Thirty
workspaces racing for CPU is what made it **red because the machine was busy**, and the C++
perf assertion below is the one test still sensitive to that, which is why a developer's run
stays sequential unless asked. `npm run typecheck` goes through the same runner, and CI runs it
(`--jobs 2`) beside the tests and the two lints, so the four together use the CPU the tests leave. The generous
timeouts later in this page are cheap insurance kept regardless: on a shared host at four jobs, tests that touch the disk took over
thirty seconds beside a build.

This page is the reason the numbers in `vitest.config.ts` are what they are.

## The runner walks package scripts, so a package without one is invisible

`npm run test` asks each workspace for its `test` script. A package
that has none is not reported as skipped — it is not reported at all. The run says *42
successful* and means *42 of the packages that opted in*.

Two had opted out by accident:

| package | test files | tests | why |
|---|---|---|---|
| `apps/k7-daw-frontend/packages/ui-eyes` | 3 | 37 | no `test` script, no `vitest` devDependency |
| `apps/k7-daw-engine/k7-runtime` | 6 (C++) | — | only `test:mac`; nothing ran on Windows or Linux |

Both are fixed, and both were green the moment they were wired in — tests that had been
written, committed, and then protected nothing. `ui-eyes` is the one that stings: `compositor.test.ts`
and `timeline.test.ts` cover the video transitions, and they were added in the same branch that
shipped them.

### The check that catches this

A package with `*.test.ts` files and no `test` script is the whole bug, and it is one command:

```bash
for f in apps/*/package.json apps/*/packages/*/package.json; do
  d=$(dirname "$f")
  node -e "process.exit(require('./$f').scripts?.test?0:1)" && continue
  n=$(git ls-files "$d" | grep -cE '\.(test|spec)\.tsx?$')
  [ "$n" -gt 0 ] && echo "$d has $n test files and no test script"
done
```

## The C++ suite: two tiers, and a skip that says so

`apps/k7-daw-engine/k7-runtime` has `"test": "node scripts/run-tests.mjs && node scripts/run-supervisor-tests.mjs"` (the second runs `apps/k7-daw-engine/k7-supervisor`'s one test binary, under a second, and skips the same way when its `build/<preset>` is not configured), so
`npm run test` compiles and runs it like any other workspace. Three
things that script does, each because the obvious version is wrong:

**It filters on `^studio_`.** `ctest -N` in this build tree lists **ten** tests, and two of them
are not ours: Tracktion vendors its own `Benchmarks` and `TestRunner` examples, and CMake registers
them into our project. They are not even built, so a bare `ctest` fails on a missing executable
with exit 8. Exactly the same leak the root `vitest.config.ts` excludes `apps/k7-daw-engine/k7-runtime/build/**`
for — a vendored dependency's own tests turning up as ours.

**It splits fast from slow on a ctest label.** Six of the eight targets are JUCE-free by design —
`tests/CMakeLists.txt` keeps them that way deliberately, compiling the plain-data TUs straight into
the test binary so they "build in seconds and run headless with no audio device". The other two,
`studio_midigraph_tests` and `studio_dspgraph_tests`, include `DspGraph.h`, which reaches Tracktion,
so they link the engine. Building those can recompile Tracktion — measured here at **over four
minutes**, and the CMakeLists already called them "opt-in the same way".

So those two carry `LABELS slow`, and the script runs `-LE slow`. The default is the fast tier,
build included. `npm run test:all --workspace k7-runtime` adds the slow two. The label lives in `tests/CMakeLists.txt` rather than in a list inside
the script, so a new test is fast unless it says otherwise, and the script's target list comes from
`ctest -N` rather than being spelled out twice.

**It skips rather than configures.** Configuring fetches Tracktion and JUCE — minutes and a
network. A tree with no `build/<preset>/CMakeCache.txt` therefore prints what to run and exits 0:

```
  SKIPPED — apps/k7-daw-engine/k7-runtime has no build/dev-gcc.
  The C++ tests need a configured tree; configuring fetches Tracktion.

    cmake --preset dev-gcc
```

That makes it non-blocking in the sense this repo already uses for `lint:dead` and
`lint:comments` — a check that is read rather than suffered. A TypeScript-only contributor is not
made to build a DAW engine to run the suite; anyone who has ever built the engine gets the tests.
The preset comes from the platform (`windows-msvc` / `macos-clang` / `dev-gcc`) and can be forced
with `$STUDIO_MOTEUR_PRESET`.

A genuine failure still fails: ctest exits 8 on a failing test and the script forwards that status.

### One build directory, one builder

Two MSBuild runs in the same `build/<preset>` collide, and the failure looks like a compile error
in Tracktion rather than like contention — which is exactly how it reads when it happens.
Hit twice while writing this page: once by running `test` and `test:all` at the same time, and it
is the same hazard `npm run dev` carries, because the desktop app task watches and rebuilds the engine
in that very directory (`dev-desktop.mjs`, and see [dev.md](dev.md)). If the C++ tests fail with a
compiler error you cannot explain, check whether something else is building before reading the
error.

## The default timeout was smaller than the tests

Vitest defaults to **5 000 ms** per test, and many of this repo's test files scaffold a real
project on disk. Passing alone, such a suite goes over that budget under thirty concurrent
workspaces on Windows, where it fails on a different test every run, most of them in
`@studio/song`.

Every one of them passes alone. **The tests were never wrong; the budget was.** A red that moves
is worse than a red that stays, because it teaches you to re-run instead of read.

So `testTimeout` and `hookTimeout` are **30 000 ms**. That is not a guess at how long a test
should take — it is a ceiling high enough that only a genuine hang reaches it. A test that is
actually stuck still fails, just later, and later is cheap compared to a suite nobody trusts.

`hookTimeout` matters as much as `testTimeout`: the disk-backed suites scaffold their fixture in
`beforeEach`, and a hook that times out fails the whole file rather than one case.

### It has to be repeated in four files

A package-level `vitest.config.ts` **replaces** the root one; it does not merge with it. Four
packages carry their own (to pin `$STUDIO_MEDIATEK` away from the developer's real data folder),
so those four repeat the two timeouts:

`apps/k7-daw-frontend/packages/{song,song-edit,song-video}/vitest.config.ts`

Every other package inherits the root `vitest.config.ts`, because vitest walks up from the
package directory when it finds no config of its own. That is also why the root file's `exclude`
list is load-bearing for packages that never read it.

## The root config excludes four trees

Running `vitest run` from the repository root is the obvious thing to do, and without a guard it
walks into code that is not a vitest suite:

- `apps/k7-daw-engine/k7-runtime/build/**` — CMake vendors Tracktion and JUCE there, and JUCE ships React demo
  apps whose `App.test.js` get collected as ours;
- `.claude/worktrees/**` — every branch ever worked on is checked out here. A bare root run once
  reported 667 failures that belonged to other branches;
- `apps/k7-daw-engine/scripts/**/*.test.mjs` — `node:test` files too, which k7-desktop's `test` runs: the build
  scripts are no workspace, and the desktop app is what they release.

All four are excluded alongside `defaultExclude`, never instead of it: dropping `**/node_modules/**`
would be a worse problem than any of them.

## What this page does not claim

That the suite is fast, or that 30 s is the right number forever. If a test file starts *needing*
thirty seconds, the fix is that test, not this ceiling. The claim is narrower: the suite runs
everything it has, and it fails only when something is actually broken.

## `$STUDIO_HOME` and the pool

A package whose tests redirect `$STUDIO_HOME` wants `pool: "forks"` in its own vitest config.

The default pool runs test FILES concurrently in one process, and `process.env` is shared
across them. A file that points `$STUDIO_HOME` at a temp dir of its own in `beforeEach` then
redirects another file's writes mid-test, and the failure shows about one run in six and never in
isolation, which is the worst shape a flake can have.

`forks` gives each file its own process, so the env var is the isolation it already looks
like. It costs a process start per file.

## The Rust suite runs under cargo-nextest on the Mac

`mac-checks.sh` runs `apps/k7-daw-backend`'s tests with `cargo nextest run`:
each test in its own process, so a test that leans on another's process-wide state — an environment
variable, a cache filled earlier, a lock meant to serialize — fails there first. `cargo test` stays
what a developer runs and the fallback on a machine without nextest ([rust-builds.md](rust-builds.md)).
