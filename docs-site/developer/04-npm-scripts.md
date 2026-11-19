# Root npm scripts

Every script in the root `package.json`, grouped by purpose. Run them from the repository root.

## Session and apps

| Script | Runs | Purpose |
|---|---|---|
| `dev` | `node scripts/dev.mjs` | The dev session under Tilt. Extra arguments go to `tilt up` (`npm run dev -- k7-front`). See [Dev setup](./03-dev-setup.md). |
| `dev:desktop` | `node scripts/dev-desktop.mjs` | The engine, supervisor and desktop app build-run-watch loop on its own, outside Tilt. |
| `storybook` | `storybook dev -p 6006` | Storybook for components under `packages/*/src` and `apps/*/frontend/src`. |
| `build-storybook` | `storybook build` | Static Storybook into `storybook-static/`. |

## Build, typecheck, test

| Script | Runs | Purpose |
|---|---|---|
| `build` | `npm run build --workspaces --if-present` | Every workspace's `build`. For most packages that is `tsc --noEmit`; for `k7-frontend` it is `vite build && tsc --noEmit`; for `@studio/dsp` it runs its emitter. `k7-runtime` has no `build`, so this never compiles C++. |
| `typecheck` | `node scripts/workspaces.mjs typecheck` | Every workspace's `typecheck` script. Accepts `--jobs N` (`npm run typecheck -- --jobs 2`). |
| `test` | `node scripts/workspaces.mjs test` | Every workspace's `test` script. Accepts `--jobs N`. See [Tests](./05-tests.md). |
| `engine:build` | `node engine/scripts/build-engine.mjs` | Builds the C++ engine and the supervisor in this platform's preset (`macos-clang`, `windows-msvc`, otherwise `release-gcc`; `STUDIO_ENGINE_PRESET` and `STUDIO_SUPERVISOR_PRESET` override it) and stages the command-line engine into `dist/k7-runtime/`. |

## Lints

| Script | Runs | Blocking? |
|---|---|---|
| `lint` | `biome check .` | Yes (GitHub `k7` `lint` job). |
| `lint:fix` | `biome check --write .` | Applies Biome's safe fixes and formatting. |
| `lint:bytes` | `node scripts/check-source-bytes.mjs` | Yes (GitHub `k7` `lint` job). |
| `lint:docs` | `node scripts/check-doc-links.mjs` | No: run it before pushing. |
| `lint:comments` | `node scripts/check-comments.mjs` | No: reports a backlog. |
| `lint:version` | `node scripts/check-version.mjs` | Yes (GitHub `k7` `lint` job). Every private workspace carries the root `package.json`'s version; `--set X.Y.Z` writes every copy, `--tag vX.Y.Z` also checks a release tag. |
| `lint:dead` | `knip` | No: reports a backlog. |
| `lint:panels` | `tsx packages/song/scripts/checkPanelBinds.ts` | No: needs the external plugins installed on the machine. |
| `lint:layouts` | `tsx packages/ui-module-builder/scripts/checkLayouts.ts` | No: run by hand after editing module panels. |

Each is described in [Lints and conventions](./06-lints-and-conventions.md).

## Configuration and maintenance

| Script | Runs | Purpose |
|---|---|---|
| `clean` | `rm -rf …/node_modules` | Deletes every `node_modules` folder in the tree (root, packages, apps). |

## Remote dev box (Windows)

| Script | Runs | Purpose |
|---|---|---|

## Workspace scripts worth knowing

These are run with `--workspace <name>`:

| Workspace | Script | Purpose |
|---|---|---|
| `k7-runtime` | `build:mac`, `build:win`, `build:android` | Engine build in the `macos-clang`, `windows-msvc` or `android-arm64` preset, through `engine/k7-runtime/scripts/build.mjs`. |
| `k7-runtime` | `test`, `test:all` | Builds and runs the C++ `studio_*` test targets; `test:all` adds the slow Tracktion-linked ones. |
| `k7-runtime` | `gen:builtin-params` | Regenerates `packages/song-schema/src/builtinParams.generated.json` from a real engine, one plugin per line. |
| `k7-runtime` | `gen:strudel-bundle`, `gen:surge-panel` | Regenerate committed generated files (the pattern-lane bundle the engine embeds; the Surge XT module's panel in `k7-core`). |
| `k7-desktop` | `stage` | Stages the frontend, the page and library bundle and the engine binaries into `apps/k7-desktop/src-tauri/resources/`. |
| `k7-desktop` | `dev`, `build` | `tauri dev`; `tauri build`, the installers. |
| `@studio/song` | `bench` | Vitest benchmarks of the compile. |
