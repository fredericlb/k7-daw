# Building and shipping engines

The engine and the supervisor are two CMake projects. Everything that builds them goes through a few Node scripts, so a developer, CI, the Android Gradle build and the server image all build the same way. **A cold engine build fetches and compiles Tracktion, JUCE, sfizz, Surge XT, QuickJS and NAM: expect tens of minutes and several GB.** On the Windows dev machine, never build by hand; Tilt's `native` resource owns it.

## CMake presets

Both projects define the same preset names in their `CMakePresets.json`. Build output goes to `build/<preset>/`.

| Preset | Platform | Notes |
|---|---|---|
| `dev-gcc` | Linux | Default on Linux for development; RelWithDebInfo, because at `-O0` the DSP cannot keep up |
| `dev-clang` | Linux | Clang alternative |
| `asan` | Linux (engine only) | Debug with AddressSanitizer and UBSan |
| `release-gcc` | Linux | Release; used by the release script and the server image |
| `windows-msvc` | Windows | Visual Studio 2026 generator; `/MP` and `/FS` are set |
| `windows-msvc-ninja` | Windows | Same compiler under Ninja, so CI's compiler cache works; embedded debug info |
| `macos-clang` | macOS | Ninja + AppleClang |
| `android-arm64` | Android | Against `$ANDROID_NDK_HOME`; the engine becomes `libk7runtime.so` |

Build presets pin `targets: ["k7-runtime"]`, so the engine's test targets are only built when named (which `run-tests.mjs` does). `$STUDIO_COMPILER_LAUNCHER` (ccache, sccache) is passed to CMake as the compiler launcher. `$STUDIO_FETCHCONTENT_DIR` moves FetchContent's downloads out of the build folder, so several presets share them.

Platform dependencies fetched at configure time: the Steinberg ASIO SDK on Windows (pinned by hash; set `ASIOSDK_DIR` to use a local copy), and a prebuilt ONNX Runtime per platform for the supervisor (DirectML on Windows).

## Scripts

| Script | What |
|---|---|
| `engine/k7-runtime/scripts/build.mjs <preset>` | Configure only when there is no `CMakeCache.txt`, then build. Takes `engine/k7-runtime/scripts/buildLock.mjs`'s lock on the build folder (a second build waits instead of corrupting Ninja's records) and caps compiler parallelism. Behind `npm run build:mac`, `build:win`, `build:android` in the `k7-runtime` workspace. |
| `engine/k7-runtime/scripts/run-tests.mjs` | Builds and runs the `studio_*` test targets in the platform's preset, under the same lock; skips an unconfigured tree |
| `engine/scripts/supervisorBuild.mjs` | `buildSupervisor(repo)` (preset from `$STUDIO_SUPERVISOR_PRESET` or the platform), `findSupervisor(repo)` (newest build, Debug last) |
| `engine/scripts/build-engine.mjs` | `npm run engine:build`: builds the engine (preset from `$STUDIO_ENGINE_PRESET` or the platform: `macos-clang`, `windows-msvc`, `release-gcc`) and the supervisor, then stages the command-line engine into `dist/k7-runtime/` with `stageEngine` and copies `engine.example.json` as `engine.json` |
| `engine/scripts/package-release.mjs --kind engine\|tray [--stamp <s>]` | Into `dist/release/`: `engine` archives `dist/k7-runtime/` as `k7-runtime_<stamp>_<platform>-<arch>.tar.gz` (`.zip` on Windows, made with System32's bsdtar); `tray` renames the installers `tauri build` left under `apps/k7-tray/src-tauri/target` to `k7-tray_<stamp>_<platform>-<arch>.<ext>` and writes the platform's fragment of the updater's `latest.json` (`engine/scripts/trayRelease.mjs`). The stamp comes from `--stamp`, `$STUDIO_RELEASE_STAMP`, or `dev`. |
| `engine/scripts/buildTools.mjs` | `distDir`, and `run` with the Windows shell quirk |
| `engine/scripts/devEngine.mjs` | What the dev session stops before a relink: every running tray, supervisor and engine (a running image cannot be relinked on Windows) |
| `scripts/dev-tray.mjs` | The dev session's tray task (the k7-tray workspace's `dev` script): build the engine, the supervisor, the backend bundle and the tray, run the tray on them, rebuild and restart on save |
| `engine/scripts/stageEngine.mjs` | Copy the binaries and what they load into a `bin/` ([86](./86-desktop-and-android.md#the-supervisor-host-k7-local)) |

## The command-line engine archive

`build-engine.mjs` then `package-release.mjs --kind engine` produce the archive users download. Its `k7-runtime/` folder holds `k7-supervisor`, `k7-runtime`, the ONNX Runtime libraries, and `engine.json`. The supervisor finds the engine beside itself, and the supervisor finds ONNX Runtime beside itself.

## CI

No CI workflow compiles the C++ engine: a developer builds it locally with `engine/scripts/build-engine.mjs`, and packages it with `engine/scripts/package-release.mjs`. `.github/workflows/k7.yml` runs the TypeScript checks and builds the native supervisor for its unit tests and the tunnel suites, but not the engine.
