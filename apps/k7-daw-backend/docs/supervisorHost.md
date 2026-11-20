# The supervisor host

`apps/k7-daw-engine/k7-supervisor` as the child of the front: `packages/k7-local`, which the front runs on the desktop app
to host the machine's supervisor and so its engine (`localEngine.md`, `local.md`). The Android and iOS
apps drive the same supervisor the same way, from Kotlin and Swift (`SupervisorHost.kt`,
`SupervisorHost.swift`, `apps/k7-daw-engine/k7-runtime/docs/android.md`, `apps/k7-daw-engine/k7-runtime/docs/ios.md`): the
choices below are the three hosts', and a change to one is made to all three.

The binaries every package carries are staged by `apps/k7-daw-engine/scripts/stageEngine.mjs`, which copies the
newest engine and supervisor builds into an app's staging `bin/`: the desktop app's
(`apps/k7-daw-desktop/scripts/stage.mjs`) and the command-line engine's (`apps/k7-daw-engine/scripts/build-engine.mjs`).

## Not a server

`apps/k7-daw-engine/k7-supervisor` is native and speaks NDJSON on stdio
(`apps/k7-daw-engine/k7-supervisor/docs/hostProtocol.md`), so it listens on no port and the host spawns it
directly and keeps it running (`packages/k7-local/src/host.rs`). The choices:

- **Death is read from the process, never the pipe.** The engine outlives a supervisor crash by
  design, and a stdout copy inside it would hold the pipe open; the child's own `wait` is the
  process's end, which no pipe can delay.
- **A crash is relaunched on 1 s→30 s backoff**, reset only after a run of 20 s. One that dies at
  once is broken, and relaunching it every second would bury the log.
- **A stop is `shutdown`, then the pipe closed, then 6 s**, after which the supervisor is killed. A
  supervisor on Windows takes about 4 s to deregister, stop its engine and exit; a shorter grace
  kills it at the end of its own clean stop.
- **The backstop kills only an engine the pidfile proves.** An engine still alive after the
  teardown is killed from `engine.pid`, and a pid is reused by the OS: `packages/k7-local/src/pidfile.rs`
  reads the start time and executable the supervisor wrote there and kills only when the OS reports
  the same for an executable named `k7-runtime`, the verdict `PidFile.cpp` gives. A bare pid, or a
  process the OS will not describe, is left running (`apps/k7-daw-engine/k7-supervisor/docs/supervisorPolicy.md`).
  The desktop app's quit links the same proof for its own backstop (`apps/k7-daw-desktop/src-tauri/src/pidkill.rs`).
- **A write to a dead supervisor is said, never an error.** The supervisor may die between a page's
  command and its delivery; the command is lost with it, and the relaunch is what answers.
- **On Windows the supervisor makes the kill-on-close job itself, told `STUDIO_HOST_PID`.** The
  host puts the child in no job of its own and out of the console's Ctrl-C group
  (`CREATE_NO_WINDOW`, `CREATE_NEW_PROCESS_GROUP`), so the supervisor's job is the only one, its
  handle held by the host alone, and a force-killed host still takes the engine with it.

## One schema for the wire

`apps/k7-daw-engine/k7-supervisor/schema/hostProtocol.schema.json` is the host protocol's single definition
(`apps/k7-daw-engine/k7-supervisor/docs/hostProtocol.md`), beside the supervisor that writes it, where the
engine's CMake points `HostProtocol_test.cpp` at `schema/fixtures/`. The choices:

- **The writer is not generated.** nlohmann's `NLOHMANN_DEFINE_TYPE_*` cannot say what the wire
  says: an `error` absent when empty, `downloads` absent when empty, `pluginCount` null, `paths`
  folded from two fields, a fixed key order. The writer stays, and the fixtures hold it to the schema.
- **The fixtures are what holds each side.** `HostProtocol_test.cpp` compares the writer with them
  byte for byte, so a change to the schema changes its fixtures in the same commit.
- **A snapshot has every list and no null.** A frame may leave an empty list out and writes null
  for "none"; `packages/k7-local/src/protocol.rs` fills them, so a page reads one shape whatever the
  supervisor's version left out.
- **The phones decode no typed frame.** `SupervisorHost.kt` and `SupervisorHost.swift` hand each
  snapshot, as the dictionary it arrived as, to the page that draws it, and read only a handful of
  keys themselves: the `engine` frame, `sessions[].playing`, `instances[].url`, a result's `ok`.
  A typed decoder on Android needs Klaxon, Jackson or kotlinx.serialization, none of which the app
  has, and a typed copy beside the dictionary would decode the heaviest frame twice for those keys.
