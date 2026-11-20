# The engine's build scripts

`apps/k7-daw-engine/scripts/` builds the two native programs and packages what ships to a musician's machine.
They live under `engine/` because both deliverables are built around the same two binaries: the
command-line engine IS those binaries, and the desktop app (`apps/k7-daw-desktop`) carries them.

| script | what |
|---|---|
| `build-engine.mjs` (`npm run engine:build`) | the engine, then the supervisor, then the command-line engine staged in `dist/k7-runtime/` |
| `supervisorBuild.mjs` | building and finding the supervisor — one answer for this script, the dev session and the Android app's Gradle build |
| `package-release.mjs --kind engine\|desktop app` | one release file per kind into `dist/release/`, named for a stamp |
| `devEngine.mjs` | what the dev session stops before a relink: every running desktop app, supervisor and engine (`docs/dev.md`) |
| `buildTools.mjs` | where output goes, and spawning with the Windows shell quirk |

The engine builds through `apps/k7-daw-engine/k7-runtime/scripts/build.mjs`, so "how the engine builds" is
defined once. Each program's preset comes from the host platform, or from `STUDIO_ENGINE_PRESET` and
`STUDIO_SUPERVISOR_PRESET` when the build machine names its own — which CI does.

## The command-line engine is a folder, not an installer

`dist/k7-runtime/` holds everything the engine needs to run with no repo and nothing on `PATH`,
staged by `apps/k7-daw-engine/scripts/stageEngine.mjs` — the same function the desktop app
stages its `bin/` with, so the two deliverables cannot carry different things:

| file | what |
|---|---|
| `k7-supervisor` | what the user runs: `k7-supervisor --config engine.json` |
| `k7-runtime` | the audio engine, which the supervisor finds beside itself |
| ONNX Runtime | the library the supervisor loads for a stem cut |
| `engine.json` | a copy of `apps/k7-daw-engine/k7-supervisor/engine.example.json`, to edit |

One flat folder because each binary looks for the next beside itself. An archive of it is the
release: unpacking it is the whole install, and the config is a file the user edits, so there is
nothing for an installer to do.

`stageEngine` also takes a staged folder as it is, from `$STUDIO_PREBUILT_BIN`: that is how CI's
desktop app job packages the desktop app around the binaries the engine job built, compiling nothing.

### ONNX Runtime, beside the supervisor

The supervisor LOADS ONNX Runtime rather than linking it (`apps/k7-daw-engine/k7-supervisor/docs/supervisor.md`),
and looks for it beside itself first. Its build copies the library beside the supervisor binary, and
the staging takes it from there. Missing is a warning, not a failure: the engine runs, and says it
cannot cut stems.

## `dist/` is the repo's

`dist/` sits at the monorepo root, one place to look for anything this repo produces, and
`buildTools.mjs` holds the one definition of where it is. `dist/release/` is what a release
uploads; nothing else in `dist/` is.
