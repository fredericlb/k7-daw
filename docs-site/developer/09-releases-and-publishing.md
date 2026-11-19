# Releases and publishing

No CI job builds or publishes a release: every file below is built on a developer's machine, by the
commands in this page, when one is needed.

## What a build makes

| File | Built from | Platforms |
|---|---|---|
| `k7-runtime_<version>_<platform>-<arch>.tar.gz` (`.zip` on Windows) | `engine/scripts/build-engine.mjs` + `engine/scripts/package-release.mjs --kind engine` | `windows-x64`, `macos-arm64`, `linux-x64` |
| `k7-android_<version>_arm64.apk` | `./gradlew assembleRelease` in `engine/k7-android` | Android arm64 |
| `k7-daw_<version>_<platform>-<arch>.<exe/AppImage/deb/dmg>` (K7 DAW) | `npm run build --workspace k7-desktop` + `package-release.mjs --kind desktop` | `windows-x64`, `linux-x64`, `macos-arm64` |
| the iPhone and iPad app | Xcode, after `engine/k7-ios/scripts/stage.mjs` | iOS |

### The version

Release files are named for the root `package.json`'s version, which every build reads.

The version is the root `package.json`'s, copied into every private workspace:

```bash
node scripts/check-version.mjs --set 1.2.0
```

The APK is signed with the release key, since Android refuses to update an app signed by a different
key. The files of a desktop build are attached by hand to a release of the GitHub repository, which is where the site's
front page sends people; the phone apps go through the stores.

## The command-line engine

`npm run engine:build` (`engine/scripts/build-engine.mjs`):

1. builds the engine with `engine/k7-runtime/scripts/build.mjs <preset>` (preset from the platform, or
   `$STUDIO_ENGINE_PRESET`);
2. builds the supervisor (`engine/scripts/supervisorBuild.mjs`, preset overridable with
   `$STUDIO_SUPERVISOR_PRESET`);
3. stages both into `dist/k7-runtime/` with `stageEngine` (`engine/scripts/stageEngine.mjs`), which copies
   the newest build of each together with what they load at run time;
4. copies `engine/k7-supervisor/engine.example.json` there as `engine.json`.

It prints the command to run the result: `dist/k7-runtime/k7-supervisor --config dist/k7-runtime/engine.json`.
`package-release.mjs --kind engine` then archives that folder into `dist/release/`, as `.tar.gz`
outside Windows (to keep the executable bits) and as `.zip` on Windows, using the system `tar.exe`.

Environment variables the build reads:

| Variable | Effect |
|---|---|
| `STUDIO_ENGINE_PRESET`, `STUDIO_SUPERVISOR_PRESET` | CMake preset names. |
| `STUDIO_FETCHCONTENT_DIR` | Where CMake keeps downloaded dependency sources (Tracktion, JUCE and others), so they are fetched once. |
| `STUDIO_COMPILER_LAUNCHER` | Compiler launcher read by the presets (`sccache` or `ccache`). |
| `CMAKE_BUILD_PARALLEL_LEVEL` | Compiler parallelism; the build scripts default to cores minus two. |

## K7 DAW (`apps/k7-desktop`)

`npm run stage --workspace k7-desktop` (`apps/k7-desktop/scripts/stage.mjs`) fills
`apps/k7-desktop/src-tauri/resources/`: the page and the `k7-core` library bundled by
`apps/k7/backend-rs/scripts/stageBundle.mjs` (the same bundle the phone runs), the front's code root, and the engine binaries via `stageEngine`; on a Mac, `stage` also signs every binary of the resources (`apps/k7-desktop/docs/macos.md`).
`npm run build --workspace k7-desktop` then runs `tauri build`: an NSIS installer on Windows,
a `dmg` on macOS, an AppImage and a `.deb` on Linux.

`stageEngine` takes the binaries from `$STUDIO_PREBUILT_BIN` when it is set, instead of from the
local build folders, to package the desktop app around an engine archive without compiling anything. `package-release.mjs --kind desktop` copies each installer
to `dist/release/` under its release name.

[K7 DAW and the phone apps](./86-desktop-and-android.md) covers the apps themselves.

## The Android APK

Gradle builds everything: the engine as `libk7runtime.so` in the `android-arm64` preset, the
supervisor, and the Capacitor page from the `engine/k7-android/web` workspace (which is why `npm ci` runs
at the root first). The versions it needs are pinned in `engine/k7-android/app/build.gradle.kts`:
compile SDK 36 and NDK 29.0.14206865, on JDK 21.

```bash
npm ci
cd engine/k7-android && ./gradlew assembleDebug
# app/build/outputs/apk/debug/app-debug.apk
```
