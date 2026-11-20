# The front inside a host

The hosts that cannot, or should not, run another program run the front inside their own process,
on a thread of its own with a small tokio runtime (`k7_front::embedded`). The desktop app, being Rust, links
`k7-front` and calls it directly. The phones go through `k7-embed`, a C library over the same calls:
Android loads it as a `cdylib`, iOS links it as a `staticlib`. Its ABI is the supervisor's on iOS
(`Embedded.cpp`): C calls, text in.

`k7-embed` builds only those two C outputs, never an `rlib`: a crate a Rust host also linked would
make that host's build produce the C library beside it, for nothing.

| call | |
|---|---|
| `int32_t k7_front_start(const char *config_json)` | starts the front on a thread of its own; the port it listens on, or a negative code: -1 not UTF-8, -2 not a valid config (a backend named without its secret, or none named without every switch `front`: `front.md`), -3 one runs already, -4 the runtime or the bind failed |
| `int32_t k7_front_stop(void)` | stops it and waits for it; -1 when none runs |
| `void k7_roam_install(list, put)` | iOS: the host's list store as two callbacks, before the start (`roam.md`); a host that never calls it has no list |

## The choices

| choice | why |
|---|---|
| on Android the front's panics and its stop are written to logcat under `k7-front` (`src/logcat.rs`); a host that links the front gives `exit::watch` its own sink, as the desktop app does | the app's stderr is not kept, and the front runs in the app's process (`front.md`, "Every end leaves a line") |
| on Android the process's stdout and stderr are pipes whose lines go to logcat under `k7-front`, stdout at INFO and stderr at WARN, from the front's first start | the app's stdout and stderr are not kept, and every crate of the front prints its lines there (`files |`, `sync |`, `[registry]`): the lines are the binary's, one entry each, no more |
| the engine's start takes stderr for its own pipe (`k7-runtime`, `apps/k7-daw-engine/k7-runtime/docs/android.md`), the front's stderr lines with it; a stdout or stderr that is already a pipe when the front starts is left to its reader | stderr is the process's, and the engine runs in the same one; `adb logcat -s k7-front k7-runtime` reads both |
| a panic is written to logcat directly, then again by the default hook through stderr | the direct line is written before the process can end; the pipe's thread may not get to its copy |
| JSON config, the same fields as the environment (`front.md`) | one shape for every host, and a C ABI needs no struct layout agreed across languages |
| the port is returned only once the socket is bound | a host can hand the address to its views at once, and `listen: 127.0.0.1:0` works |
| one front per process; a second start is refused | a host that restarts it stops it first, and two fronts on one backend would be two secrets |
| four worker threads | families start together (the files', the supervisor's and others) and each blocks a worker while it starts, so the file family — the one a starting page asks first — must not queue behind the others; a phone still serves one person, and the runtime's default of one thread per core is a desktop's |
| the JNI entry points are in the crate (`src/jni.rs`), with no JNI crate: the string is read through the JNIEnv's own `GetStringUTFChars` | JNI finds a native method only by its mangled name in the library itself; two calls and one string do not justify a dependency |

## Built for

- `aarch64-linux-android` with NDK 29's `aarch64-linux-android29-clang` as the linker (the app's
  `minSdk`). The `.so` needs only `libc`, `libm` and `libdl`.
- `aarch64-apple-ios` as a staticlib only: `cargo rustc --release -p k7-embed --target aarch64-apple-ios --crate-type staticlib`.
  A `cdylib` for iOS needs Apple's linker, which only a Mac has.

## On a phone

The workspace's tests run on an Android phone as they run on a computer: `cargo test --no-run
--target aarch64-linux-android`, each test binary pushed to `/data/local/tmp` and run with
`adb shell`, which is why no test reads a file from the checkout. The app's own front, on its studio
port, is what the conformance suite passes through over `adb forward` (`e2e/docs/conformance.md`).

## In the Android app

Every build ships `libk7_embed.so`, built by Gradle's `buildFront` through cargo with the NDK's clang at the
app's `minSdk`, and `RustFront.kt` runs it on the studio's port, in front of node, with a secret drawn per
launch (`apps/k7-daw-engine/k7-runtime/docs/android.md`). A release build therefore needs Rust's `aarch64-linux-android`
target on every machine that builds one: GitHub's android job adds it.

## In the iOS app

`apps/k7-daw-ios/scripts/stage.mjs` builds `libk7_embed.a` through cargo, and `BackendHost.swift` starts it on the studio's port
with no node behind it (`apps/k7-daw-engine/k7-runtime/docs/ios.md`); a developer builds the app in Xcode.
