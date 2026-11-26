# Tests

## Runners

| Code | Runner | Where the tests are |
|---|---|---|
| TypeScript (packages, apps) | [Vitest](https://vitest.dev) 4 | `*.test.ts` / `*.test.tsx` next to the code they test; `*.bench.ts` for benchmarks |
| C++ engine | CTest over CMake targets named `studio_*` | `apps/k7-daw-engine/k7-runtime/tests` |
| Rust supervisor | cargo nextest, in the backend's workspace | `apps/k7-daw-backend/packages/k7-supervisor/tests` |
| Supervisor tunnel wire | Vitest, run against both the TypeScript agent and the Rust `tunnel_harness` | `apps/k7-daw-frontend/packages/engine-tunnel` |
| Engine behaviour that needs a running engine | Node harness scripts, run by hand | `apps/k7-daw-engine/k7-runtime/scripts` |

Every workspace that has tests declares `"test": "vitest run"`. A few packages also declare `"bench": "vitest bench --run"`
(`@studio/song`, `@studio/ui-chains`).

## Running them

```bash
npm run test                        # every workspace's `test`, one at a time
npm run test -- --jobs 4            # four workspaces at once
npm run test --workspace @studio/song          # one package
cd apps/k7-daw-frontend/packages/song && npx vitest run src/loader.test.ts   # one file
cd apps/k7-daw-frontend/packages/song && npx vitest                  # watch mode
```

`npm run test` and `npm run typecheck` go through `scripts/workspaces.mjs <script> [--jobs N]`. It
lists the workspaces with `npm query .workspace`, keeps those that declare the script, and runs
`npm run <script>` in each with `concurrently`, at most `N` at a time (default 1, the same as
`npm run <script> --workspaces --if-present`). Output is grouped per workspace, a summary line
gives the count and duration, and the run fails naming every workspace that failed. The number of
jobs is bounded on purpose: most suites touch a real disk.

The root `npm run test` includes the `k7-runtime` workspace, whose `test` script builds and runs
the C++ suites (below). On a machine where the engine has never been configured it prints
`SKIPPED` with the `cmake --preset` command to run and exits successfully, so the JavaScript run is
not blocked by a missing C++ toolchain.

## Vitest configuration

The root `vitest.config.ts` applies when Vitest is started from the repository root. It keeps
Vitest's default excludes and adds `apps/k7-daw-engine/k7-runtime/build/**`, `apps/k7-daw-engine/k7-runtime/build-*/**`
(CMake vendors JUCE there, whose demo apps contain `App.test.js` files) and `.claude/worktrees/**`.
It sets `testTimeout` and `hookTimeout` to 30 seconds.

A package's own `vitest.config.ts` **replaces** the root one, so packages that need the timeouts
repeat them. Several packages pin ambient state to folders that do not exist so that no test reads
the developer's real data:

| Package config | Pins | Also |
|---|---|---|
| `apps/k7-daw-frontend/packages/song/vitest.config.ts` | `STUDIO_MEDIATEK`, `STUDIO_HOME` | 30 s timeouts |

Other packages with a config of their own: `audio-passes`, `song-edit`, `song-video`, `translator`.

`@studio/env` never reads `env.yaml` under a test runner, so tests do not see local secrets.

### Writing a test

- Put it next to the file: `foo.ts` and `foo.test.ts`. Knip treats `*.test.*`, `*.bench.*` and
  `__tests__/**` as entry points, so test-only helpers are not reported as dead.
- A test that needs a library or an instance data folder sets `STUDIO_HOME` or `STUDIO_MEDIATEK` to its own
  temporary folder. `@studio/song/testSupport` has fixture helpers that copy a shipped demo project
  and edit one value in it by key path (a path that does not exist throws).
- Disk-touching tests should clean up their temporary folders; the 30-second timeout exists for
  them, not as a target.
- Packages that need an i18n catalogue compile it first: `k7-frontend`'s `pretypecheck` compiles its
  own catalogue and `@studio/k7-plugins`' one.

## C++ engine tests

`npm run test --workspace k7-runtime` runs `apps/k7-daw-engine/k7-runtime/scripts/run-tests.mjs`:

1. Picks the preset from the platform (`windows-msvc`, `macos-clang`, otherwise `dev-gcc`), or
   `$STUDIO_MOTEUR_PRESET`.
2. If `build/<preset>` has never been configured, prints `SKIPPED` and the configure command, and
   exits 0. It never configures on its own, because that downloads Tracktion and JUCE.
3. Asks CTest for the registered tests matching `^studio_`, excluding the `slow` label unless
   `--all` is passed (`npm run test:all --workspace k7-runtime`). Adding a target to
   `apps/k7-daw-engine/k7-runtime/tests/CMakeLists.txt` is enough for it to be picked up.
4. Builds those targets under the build-folder lock shared with the dev watcher, then runs CTest
   with `--output-on-failure`.

The engine's ordinary build only builds `k7-runtime`, so a suite that stopped compiling is only
noticed when its target is built: run the tests after touching the engine's data layer.

## Supervisor tests

The supervisor's tests are the backend workspace's (`cargo nextest run -p k7-supervisor`, in the Mac's checks). Its
`tunnel_harness` binary is what `apps/k7-daw-frontend/packages/engine-tunnel/testAgents.ts` finds under the backend's
cargo target (or `$TUNNEL_HARNESS_BIN`), and every tunnel suite runs against it as well as against the TypeScript
agent. Without that build, the Rust agent's cases are skipped.

```bash
(cd apps/k7-daw-backend && cargo build -p k7-supervisor --bin tunnel_harness)
cd apps/k7-daw-frontend/packages/engine-tunnel && npx vitest run
```

## Where tests run automatically

| Where | What |
|---|---|
| GitHub `k7` workflow | `npm run test --workspace=apps/k7-daw-frontend --if-present`, the supervisor tests and the tunnel conformance suites |

See [CI](./07-ci.md).
