# How the C++ engine links the Rust server

The server is a static library, `k7_runtime_server`, that the C++ targets link; CMake stays the
only build that links JUCE. The plan and its decision: `docs/rust-engine-server-plan.md`.

## A static library, built by CMake, in the build folder

The crate's types are `rlib` (the tests and the rest of the backend) and `staticlib` (the engine).
`apps/k7-daw-engine/k7-runtime/server/RustServer.cmake` runs `cargo build --release --lib` as a
custom target of every CMake tree, so each way into the engine (`scripts/build.mjs`, the tests'
script, Gradle's `buildEngine`, `stage.mjs` for iOS) builds the archive with no step of its own.

- **A static archive, not a `cdylib`.** The engine's targets are an executable, an Android `.so` and an iOS
  framework; each already holds JUCE, and one archive linked into each is the same code on all three.
  A second shared library would need its own loading and its own place in an app bundle, and iOS has no
  Rust `cdylib` without Apple's linker (`docs/ios.md`).
- **cargo's folder is the build folder's `rust/`, named with `--target-dir`.** Deleting a preset's build
  folder deletes its Rust build, and presets (a phone, the simulator, the host) never share objects.
  `CARGO_TARGET_DIR` is never set: sccache hashes every `CARGO_*` variable.
- **The triple follows the CMake platform**: `aarch64-linux-android`, `aarch64-apple-ios`,
  `aarch64-apple-ios-sim`, else the host's own. The deployment target is the preset's, passed to cargo as
  `IPHONEOS_DEPLOYMENT_TARGET` or `MACOSX_DEPLOYMENT_TARGET`, so Rust's objects and C++'s agree.
- **Release always.** The crate is small and the C++ build is the slow one; a debug archive would only
  add a second set of objects to keep.

## The archive needs the engine, so it links first

Rust's `entry` module calls `k7_engine_api`, which `studio_engine` defines. A linker reads archives in
order, so `k7_runtime_server` is listed before `studio::engine` on every target that links it.

- **The entry points stay C++ and call the archive** (`engine/capi/k7_server.h`): `main` hands argv to
  `k7_server_main`, the JNI `nativeServe` and the iOS `k7_engine_serve` call `k7_server_serve`, and their quit
  and suspend calls `k7_server_request_quit` and `k7_server_suspend_audio`. The hosts that start an engine (the
  supervisor, the desktop app, the phone apps) see the same names they always started, so none of them changed.
- **The link is the test**: each target calls its door, so a missing or renamed symbol fails the link on the
  platform that lacks it, not later on a device.

## The door runs the socket core

`serve` is `server::start` followed by the loop: the server is up and bound before the loop runs, and the
engine is held for `k7_server_request_quit`, which posts the quit as a job so it waits for a loop that has not
started. The real host is served this way by every engine run: the render harnesses
(`apps/k7-daw-engine/k7-runtime/docs/renderHarnesses.md`) and the e2e beds drive it over its socket.

## What the link does not prove

- **Windows** links the extra system libraries Rust's std names (`ws2_32`, `userenv`, `ntdll`, `bcrypt`,
  `advapi32`, `synchronization`), and the archive's C runtime is the dynamic one. Neither is built here.
