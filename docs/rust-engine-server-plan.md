# Plan: the engine's server in Rust

The C++ engine (`apps/k7-daw-engine/k7-runtime`) is two things in one process: the Tracktion host
that plays a `ProjectDoc`, and the WebSocket server k7, the previews and the DJ desk speak to
(`server/Serve*.cpp`, `Pairing`, `Outbox`, `AudioSender`, `Supersede`, `HeldNotes`,
`PendingQueries`, `WireVersion`, over IXWebSocket). The server moves to Rust, in the crate
`apps/k7-daw-backend/packages/k7-runtime-server`; the host stays C++ and is reached through one C
ABI (`apps/k7-daw-backend/packages/k7-runtime-server/docs/abi.md`).

The line between the two is JUCE: what calls JUCE or Tracktion stays C++, everything else on the
server side moves.

## Decisions (2026-11-26)

- **The C++ targets link the Rust server as a static library.** CMake stays the only build that
  links JUCE. `k7-runtime` (the desktop and CLI engine, a process of its own), Android's
  `libk7runtime.so` and iOS's `k7runtime.framework` keep their names and their entry points; those
  entry points call Rust's `serve`. Nothing that starts an engine (the supervisor, the desktop app,
  the phone apps) changes.
- **Rust parses the `ProjectDoc`; the diff and the apply stay C++ this step.** Rust parses it,
  bounds it, dedups it, checks its paths, merges `tracks` and handles supersede, then hands C++ the
  validated doc as JSON, read by a plain C++ reader with no checks of its own. Moving the diff and
  apply to Rust is a later decision, not part of this plan.
- **On a phone, the Rust server lives inside the engine library**, not in `k7-embed`: the engine
  stays one library with one door, as on the desktop.
- **The wire is the C++ server's, byte for byte.** Every client and the conformance suite read it:
  - the JSON as nlohmann prints it, sorted keys and number format included (the printer is built,
    held to `fixtures/nlohmann_dump.txt`, which `tests/WireJson_test.cpp` holds nlohmann to as well);
  - `kEngineProtocol` 1 (`apps/k7-daw-frontend/packages/engine-protocol/docs/version.md`);
  - the LSTR audio framing, the sessions, the pairing and its `paired-devices.json`;
  - the Origin and frame-size screens, the refusal of JSON nested deeper than 64 levels;
  - a ping every 10 s, a send that times out after 5 s;
  - the supervisor's in-app `render` over the socket, unchanged;
  - no permessage-deflate at first.
- **The Rust supervisor needs no change**: it starts and watches the same `k7-runtime` on the
  same socket.
- **No new crate beyond `k7-runtime-server`**: each later part is a module of it.

## Milestones

| milestone | content | hours |
|---|---|---|
| M1 the ABI and the wire's leaves (built) | the C ABI's top table and core area, its C++ implementation and test; `engine` (the safe wrapper, `Mt`, posted jobs, the fake engine); `wire` (the nlohmann-exact printer, the audio frame, the verb catalogue, the version) | 6 |
| M2 the socket core | accept, the Origin and frame-size screens, nesting refusal, ping, send timeout, the per-client outbox, sessions, dispatch by verb against the fake engine | 10 |
| M3 pairing (built) | who counts as the machine, the paired-devices store, the pairing verbs | 6 |
| M4 the link | the static library; `build.mjs` runs cargo with `--target-dir`; a slow CMake test runs `serve` on the real host and probes `whoami`; the Android and iOS cross builds | 6 |
| M5 project and transport | the `DocJson` port with its bounds, supersede, held notes; the project and transport areas | 12 |
| M6 the state frame | the state frame, `recorded`, `plugin-params`, `engine-warning` | 6 |
| M7 plugins | the scan, the probe (4 at once), the pending queries, the plugin verbs | 10 |
| M8 the machine | the `AudioSender` broadcast, sessions, drivers, devices, `io-levels`, `instrument-status`, `render` | 10 |
| M9 the CLI and the switch | argv in Rust, C++ `main` cut to a few lines, the JNI and iOS doors calling `serve`; `Serve*`, `Pairing`, `Outbox`, `AudioSender`, `Supersede`, `HeldNotes`, `PendingQueries`, `WireVersion`, IXWebSocket and their tests deleted; their pages moved to this crate's `docs/` | 12 |

Each milestone owns one ABI area (`abi.md`), so its header, `.cpp` and Rust module touch no other
milestone's files. The order:

- **M2, M3 and M4 run in parallel after M1.**
- **M5 to M8 run after M2**, and after M4 for their runs on the real engine; at most three at once.
- **M9 runs alone**, last: it is the one that deletes the C++ server.

## Tests

- **Every milestone**: the behaviours the C++ server's tests hold, rewritten against the fake
  engine (`src/engine/fake.rs`), which keeps the real threading contract.
- **From M4**: the k7-runtime harnesses (`apps/k7-daw-engine/k7-runtime/docs/renderHarnesses.md`)
  against the real engine served by Rust.
- **At M9**: the conformance specs (`e2e/docs/conformance.md`) on the Mac bed, Android's
  `assembleE2e`, `scripts/agent/ios-check.sh`, and a signed desktop build checked by hand.

## M3's seam for the socket core

`src/pairing/` (choices in `apps/k7-daw-backend/packages/k7-runtime-server/docs/pairing.md`) is built
and tested alone; the handler table of M2 calls one `Gate` (`pairing::gate`), owns no trust set of
its own, and numbers its sockets with a `SocketId` (`u64`).

- **At the handshake**: `refuse_origin(id, origin)` (the `Origin` header or `None`); true means close
  with 1008 `origin not allowed`. Then `is_refused(id)` drops every message of that socket.
- **Per frame, before parsing**: `screen_frame_size(id, peer_ip, bytes)` returns an `Out::Reply` to
  send (the 16 KB cap for a socket not yet trusted), or `None`.
- **Per message, once parsed and its `type` read**: `screen_message(id, peer_ip, &msg, ty)`. `None`
  passes to the verb table; `Some(outs)` is the whole answer and the verb must not run. The core
  performs each `Out` in order: `Reply` to the sender, `To(id, v)` to that socket if connected,
  `ToLocals(v)` to every id of `gate.locals()`, `Close(id)` closes the socket (its trust is gone).
- **At a close**: `forget_socket(id)`, whose `Out`s are performed the same way.
- **At a broadcast**: a machine-wide frame goes only to sockets where `gate.is_trusted(id)`.
- **At boot**: `Gate::new(Pairing::open("<state>/paired-devices.json"), GateConfig {..})`, the
  origins from `pairing::net::parse_origins`; `lan-open` is answered by the gate itself.
- **Sending**: every `Value` is printed with `wire::json::dump`.

Its deviation from C++: a `name`, `key`, `requestId` or `deviceId` that is not a string is read as
empty, where nlohmann's `value()` threw into the message handler's catch.
