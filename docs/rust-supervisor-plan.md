# The supervisor in Rust

`apps/k7-daw-engine/k7-supervisor` (C++, about 11,700 lines of source and 2,900 of tests) is ported to a Rust
crate, `apps/k7-daw-backend/packages/k7-supervisor`, which then REPLACES it: no fallback, no shadow run, no switch
between the two. The supervisor goes first because it runs no realtime audio. Minor bugs are allowed after the
switch and fixed forward. The rules the program keeps are the C++ supervisor's pages (`supervisor.md`,
`supervisorPolicy.md`, `hostProtocol.md`, `crashLoop.md`, `slots.md`, `stemSplit.md`, `instruments.md` in its
`docs/`), which move beside the crate at the switch, rewritten where the mechanism changes.

## Where it lives, and in what shape

| choice | why |
|---|---|
| a crate of the backend workspace, `packages/k7-supervisor`: a library and the `k7-supervisor` binary | one lock with the front, so tokio, serde_json, rustls, tokio-tungstenite, `zip` and the waveform decoder are the versions already reviewed; the backend's gates (clippy, nextest, `win-clippy.sh`, `android-check.sh`) cover it with no new step |
| a PROGRAM first on the desktop and on Android, with the same arguments (`--config <file> [--port <n>]`, `--probe`, `--separate`) and the same stdio | the hosts (`k7-local`, `SupervisorHost.kt`, `SupervisorHost.swift`) and the conformance suite stay as they are, and the switch is a binary path; a process of its own is also what keeps a supervisor's crash from taking the front, the window and the tunnel hub with it |
| on iOS a library, `k7_supervisor_run(commandsFd, framesFd)`, `k7_supervisor_cuts_in_flight()` and `k7_supervisor_log(tag, text)` exported from `k7-embed`'s static library | no iOS app may run a program; two Rust static libraries in one binary carry two copies of `std` and collide, so the supervisor joins the one the front already is, and `k7supervisor.framework` goes |
| the core is one task owning the state; every subsystem is a PART, a task of its own fed events, listed by one line in `parts/mod.rs` | the C++ `Supervisor.cpp` was one 1,364-line class every subsystem edited; with parts, two builders adding the registry and the slots touch different files, and the core never learns a new command |
| a part's slice of the snapshot is a patch the core applies over its own fields at every publish, the engine-scoped ones dropped by a stop | the snapshot stays one whole picture sent when its bytes change (`hostProtocol.md`), and a part cannot clobber the engine's status |
| settings changed only by the core, asked by `Ctx::update_settings` | the file has one writer, as the C++ loop was; a job writing it beside the core's copy would be undone by the next save |
| blocking work (a start, a probe, `lsof`, a kill) on `spawn_blocking`, never on the core's task | a blocked core freezes the crash watch and the host lock's refresh |
| `k7-local`'s pidfile proof reused (`inspect`, `alive`, `check`, `kill_tree`), the supervisor adding the `port=` line and the writer | one verdict on "is this pid the engine" for the host's backstop kill and for the supervisor; at the switch the module moves into this crate and `k7-local` reads it from here |
| in-process hosting on the desktop is NOT part of this port | it would remove the stdio host and its relaunch, and make a supervisor panic the front's; it is a decision of its own, after the switch (below) |

## The wires it keeps, byte for byte

- **The host protocol**: NDJSON frames `state`, `result` and Android's `engine`, held to
  `schema/hostProtocol.schema.json`'s fixtures by `tests/all/protocol.rs`. The schema and its fixtures move into
  the crate at the switch.
- **The state files**: `engine-id.json` (0600), `engine.pid` (pid, `start=`, `exe=`, `port=`), `engine-host.pid`
  refreshed every 2 s, `engine-host.json`, and `.studio.log`'s `HH:MM:SS [tag] text` lines, which the Logs panel
  reads by byte offset.
- **The engine's WebSocket**: the queries (`whoami`, devices, sessions, plugins, pairing, slots, `render` in an app)
  over one connection, one call in flight, and `kEngineProtocol` with the build stamp, still mirrored in
  `apps/k7-daw-frontend/packages/engine-protocol/version.ts`.
- **The front's endpoints**: the registry's hello, heartbeat and deregister (`apps/k7-daw-backend/docs/engine/heartbeat.md`),
  and the tunnel's upgrade with the tunnel secret.
- **The tunnel**: its frames are `k7-engine`'s own (`apps/k7-daw-backend/packages/k7-engine/src/tunnel/frames/mod.rs`),
  the hub's twin until now and the agent's from here; the path map, the file requests, the per-channel queues and
  the backpressure marks (256 KB drop, 1 MB high water) are ported. `engine-tunnel`'s suites stay the one judge:
  the crate builds a `tunnel_harness` binary and `testAgents.ts` finds it in the cargo target.
- **The WebRTC fragments** (`rtcFrames.ts`), only if WebRTC is kept (below).

## What it takes from the backend, and what it needs beside

| need | answer | new crate? |
|---|---|---|
| TLS (tunnel, heartbeats) | rustls with ring, as the front's outgoing HTTPS: no process-wide random generator to guard and no TLS write to patch | no |
| the OS's trusted roots, private CAs and *Never Trust* included | `rustls-native-certs`, or the same by hand over `security-framework` (in the lock) and the `windows` crate | yes, or none by hand |
| WebSockets | tokio-tungstenite with a rustls connector | no |
| HTTP (heartbeat, hello, instrument downloads) | hyper with hyper-rustls, every call bounded, redirects refused on the hello | no |
| decoding a mix, resampling stems | the `waveform` crate linked directly: the `rust/` C ABI and `SeparationDecode.cpp` go | no |
| unpacking instruments | `zip` (in the lock), in place of miniz | no |
| the addresses a heartbeat carries | `if-addrs` (in the lock) | no |
| ONNX Runtime | the `ort` crate with `load-dynamic` on computers, linked statically from the NuGet xcframework on iOS | yes |
| WebRTC | `str0m` (sans-IO) or `webrtc` (webrtc-rs) | yes, if kept |
| Windows' kill-on-close job, the TCP table, DXGI for DirectML | the `windows` crate (in the lock) | no |

A panic in a part ends that part's task, not the process; the panic hook writes `fatal: …` to the merged log first
(`run::log_panics`). A part whose task ended leaves its slice as it last was; the core respawning a dead part is
M2's to add if a test shows it is needed.

## Milestones

Each milestone adds files and lines to `parts/mod.rs`, `diagnostics.rs`'s `ALL` or `lib.rs`'s module list, and
edits nothing else of an earlier milestone unless its brief says which function. Tests replay the C++ tests'
behaviours, not their code, sized to what they prove and seconds in debug; a network is a server on `127.0.0.1`.

| | what | depends on | agent-hours | test strategy |
|---|---|---|---|---|
| **M1** (built) | the crate, paths, settings, identity, merged log, host protocol and snapshot, frame sinks, pidfile, ports, process spawn, engine start/adopt/stop (computer and in-app), crash loop and relaunch, host lock, the core with its parts API, the stdio, `--config` and embedded runs, `k7-fake-engine` | — | 6 | the fixtures byte for byte; settings, identity, log, pidfile, lock, crash loop and ports as `Settings_test`, `HostLock_test`, `CrashLoop_test`, `EnginePorts_test` held them; the program end to end against the fake engine: start, a kill relaunched, stop, start, an unknown command, stdin's end, `--config` and SIGTERM |
| **M2** (built) engine queries | the one-connection request/reply client, wire version, `whoami` inspection (replace a foreign build or a safe engine, name the suspect), hang watch, devices poll with the audio policy and `audio.priority`, sessions, edit plugins, plugins (fast while scanning), `lan.open`, `plugins.rescan`, `plugin.showUI`, `session.drop`, pairing poll and `pair.*` | M1 | 10 | a fake engine speaking the engine's WebSocket on 127.0.0.1 (tokio-tungstenite server in the test); `AudioPolicy_test`'s cases; the signatures change exactly when the rows do |
| **M3** (built) network and registries | the URL rules (`services.ts`'s cases replayed), the HTTP client, trust roots, TLS config shared by every socket, addresses and presence, the performance measure and its score (`performance.ts`'s numbers), the registry part (resolve, heartbeat skipped while one is in flight, the log policy, deregister bounded, `weburi.*`, `registry.deregister`, the paired studios listed once, origins), `--probe` | M1 | 10 | `Url_test`, `Registry_test`, `Performance_test` behaviours; a registry on 127.0.0.1 that refuses, redirects, hangs and answers; the plaintext refusal |
| **M4** tunnel agent | the agent with redial and its close reasons, engine channels piped, the path map, file stat/download/upload with the server's mtime, serial queues and their bounded drain, backpressure, the jobs channel with a job table (one line per job), `tunnel_harness` | M3 | 14 | `PathMap_test`, `SerialQueue_test`, `Backpressure_test` behaviours; `apps/k7-daw-frontend/packages/engine-tunnel`'s vitest suites green against the harness |
| **M5** slots and machine jobs | the slot store, `SlotFiles` (an SFZ's folder, a plugin refused), the slot commands, the enabled slot applied at each start and followed in the sessions poll, the jobs `log` (1 MB slices), `local`, `prefetch`, `send-up`, `chains`, `onnx-providers`, `render` (a child, or `ServedRender` in an app), the slot jobs, the kept-history purge | M2, M4 | 12 | `Slots_test` and `MachineJobs_test` behaviours, the jobs over the harness |
| **M6** instruments | the catalogue, the hash-pinned download, the unpack, one at a time with progress in the snapshot, `instruments.download`/`remove`, the first boot's choice | M3 | 6 | `Instruments_test`'s; an archive served on 127.0.0.1, a wrong hash refused |
| **M7** (built) stem separation | the windows and overlap-add, the 24-bit WAV, the decode through `waveform`, the budget and the phone's memory refusal, the ONNX session on the first provider that builds, the hour's end, `JobSlots`, the `separate` job, `--separate` | M1 (the job's line waits for M4) | 12 | `SeparationAudio_test`, `SeparationBudget_test`, `SeparationDecode_test`, `JobSlots_test` behaviours; a tiny ONNX model written by hand in the test, a cut that passes saying it skipped when the runtime is absent. `ort` 2.0.0-rc.15 with `load-dynamic` (feature `dynamic-ort`), `api-24`, `coreml` on Apple and `directml` on Windows; `waveform` linked as a path dependency with its `encode` feature, which brings symphonia, rubato, opus-rs, ebur128 and flacenc into the backend lock; the choices are `apps/k7-daw-backend/packages/k7-supervisor/docs/separation.md`; the Windows adapter choice stays M8's |
| **M8** Windows | the kill-on-close job the host holds, the TCP table's listener, the wide hostname, DirectML's discrete adapter, the `Disallowed` store and the server-auth usage | M1, M3, M7 | 6 | `win-clippy.sh`; the behaviours on a Windows machine when one is back |
| **M9** the switch | the builds (`supervisorBuild.mjs`, `stageEngine.mjs`, `build-engine.mjs`, `dev-desktop.mjs`, the Android Gradle task, iOS's `stage.mjs` and the C ABI in `k7-embed`), `k7-local`'s `real.rs` against the Rust binary, the schema and pages moved, the C++ supervisor and its CMake deleted, the notices and the workflows | all | 12 | `bash scripts/agent/gate.sh` with `android-check.sh` and `ios-check.sh`, the conformance specs on the Mac's e2e bed, a signed desktop build run by hand |

**What M2 chose.** The queries are one part, `parts/queries`, over `engine_client` (one connection, one call in flight,
a reader task that discards the engine's broadcasts and the news of any session). A result is kept only if the engine it
came from is still the one running, so a stop or a relaunch cannot be undone by a late answer. Its choices:

- The first questions of a start (the LAN setting, `whoami`) wait for the engine to listen, then every poll runs at
  once, in that order; a spawned engine is not yet listening when the core calls it up.
- A rescan wakes the catalogue poll, which otherwise waits out its minute with the spinner showing.
- A command that changes the settings waits to see the change before it reads it, so the ranking in force is the one
  just sent; commands run one after another, since they share the one call in flight anyway.
- `Paces` and `queries::set_paces` exist for the tests alone; `k7-ws-engine` is the engine the tests start (it answers
  from files and logs what it received), and `build.rs` takes the build stamp from k7-runtime's `package.json`.
- M5 builds its own `EngineClient` for the slot's project and follows the sessions with its own poll.

**M3's choices** (built; the files are `urls`, `trust`, `http`, `addresses`, `performance`, `registry`, `probe` and the `parts/registry` part):

- **The URL rules are the `url` crate's WHATWG parser**, in place of the C++ hand parser, so they match Node's `new URL` by construction; `services.ts`'s cases are replayed and `$STUDIO_PORTS` is read by one pure function.
- **Trust roots are `rustls-native-certs`** on computers (its lock entries: `openssl-probe`, `schannel`), the shipped bundle on iOS and the conscrypt folders on Android; `$STUDIO_EXTRA_CA_CERTS` is appended. A platform that gives no root falls back to Mozilla's, where the C++ client had no trust at all.
- **One TLS configuration** (`trust::client_config`, ring, read once) serves every socket; `http::send_with` takes another, which is how the tests trust a private CA.
- **The HTTP client is hyper with no pool and no redirect**; `timeout` bounds the connect and the rest, each, so a call answers within twice it. Redirects, `Range` and the resumed download are M6's, with the instruments they serve.
- **The registry's bodies go out with sorted keys**, as the C++ JSON library wrote them, so the bytes match; a heartbeat to every registry is one task each, awaited in order.
- **The part holds the registry state and writes the settings only through `Ctx::update_settings`**; its own copy of the pinned list and the tokens is what the next heartbeat reads, since the core's copy follows a tick later. The first heartbeat of a pairing carries its token for that reason.
- **The engine's `protocol` and `build` ride the presence through `registry::set_presence_extra`**, called by M2's wire version; `safeMode` follows the start (a spawned engine in safe mode). The pidfile gate of the C++ beat is the core's engine view here.
- **The fingerprint's MACs come from `getifaddrs`** (libc); Windows lists no interface and no memory size until M8.

**Parallel groups.** After M1: **M2, M3 and M7** at once, on disjoint files (queries; network and registry;
separation). After M3: **M4 and M6**, and M7 finishing. After M2 and M4: **M5**, beside **M8**. **M9** alone,
last. Two to three builders are busy throughout; the whole is about 90 agent-hours, M1's included.

## Decisions for the user

1. **WebRTC.** It serves a browser that cannot reach the engine directly, which only a remote server's page is;
   with no hosted app the path is dormant (`docs/inventaire.md`, the second-device rework). Recommended: not
   ported; the Rust supervisor answers an offer with a refusal and the browser stays on the tunnel. Porting it
   means `str0m` (small, sans-IO, data channels without media; we drive its sockets) or `webrtc` (complete, heavy,
   the libdatachannel of Rust), about 10 more agent-hours.
2. **ONNX Runtime binding.** Recommended: the `ort` crate. Strengths: the binding every Rust user of ONNX
   Runtime has, `load-dynamic` keeps "loaded, never linked" on computers, static linking supported for iOS,
   execution providers (CoreML, DirectML) as features. Weaknesses: a 2.0 release-candidate line whose releases
   follow ONNX Runtime's versions, and a large API surface we use a sliver of. The alternative is our own FFI
   over `OrtApi`'s function table, as `OnnxSession.cpp` does: no crate, about 400 lines of `unsafe` to review.
3. **Trust roots.** Recommended: `rustls-native-certs`. Strengths: the macOS keychain read WITH its trust
   settings, Windows' ROOT stores, the Linux bundles, maintained by the rustls team. Weaknesses: no `Disallowed`
   store or usage filter on Windows (M8 adds those), nothing on Android or iOS (the bundle the app ships, as today).
   The alternative is porting `TrustRoots.cpp` over `security-framework` and `windows`, both in the lock.
4. **Windows.** Windows is set aside; M8 ports what only Windows needs. Recommended: M8 before M9 only if a
   Windows desktop build is wanted again by then; otherwise M9 deletes the C++ supervisor and Windows builds the
   Rust one with M8 still to do, knowingly.
5. **In-process on the desktop** (after the switch): the front calling the library instead of spawning it.
   Recommended: no. It saves a process and the NDJSON copy but loses the crash isolation `k7-local`'s relaunch
   gives, and the engine still needs the job and pidfile rules.
