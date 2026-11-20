# The engine core in Rust

The registry, the relay and its gate, and the tunnel hub — the server's engine core — answered by the
front (`apps/k7-daw-backend/packages/k7-engine`, routed by `apps/k7-daw-backend/packages/k7-front/src/routes/engine/`). What it decides is in
`engine/`: who has which project (`engine/claim.md`), a heartbeat read (`engine/heartbeat.md`), which
engines the device's account may use (`engine/ownership.md`), the relay (`engine/proxy.md`), one registry per origin
(`engine/shared.md`) and an engine's credentials (`engine/token.md`); the tunnel's pages are
`apps/k7-daw-frontend/packages/engine-tunnel/docs/`. This page is how the front holds it.

## The switch

`K7_ENGINE_CORE=front` in the front's settings (`ENV_KEYS`) turns it on, and every host turns it on.
Off, no process answers the engine paths: they reach the backend, whose answer is a 404. The core also
needs the auth folder, as every session route does (`front.md`), and stays off without it.

| choice | why |
|---|---|
| one switch for every engine path: `/api/registry/*`, `/api/engines*`, `/api/engine/*` with the relay's upgrade, `/api/engine-tunnel*`, and the internal seam's engine half | they share one memory: the relay asks the registry's claims on every frame, the hub admits a tunnel on the registry's word, the seam reads both |
| the local engine's half of the seam (`/api/internal/engine/local*`) is not this switch's | it is the local engine's, which has its own (`local.md`) |
| the routes go through the front's chain (`routes.md`), the upgrades and the seam do not | an upgrade is checked before its 101 by its own credential (a ticket, a tunnel secret, a bound key), and the seam has a stricter gate of its own |

## What turning it on requires

| requirement | why |
|---|---|
| `STUDIO_ENGINES_DIR` in the front's settings, the folder the previous release used | the bindings file is the registry's memory across restarts, and a rollback reads the same files |

## The choices

| choice | why |
|---|---|
| a crate of its own, `k7-engine`, with no HTTP framework in it | the trust decisions are tested alone, by their own unit tests and vectors; the front only reads requests and writes answers |
| `tokio-tungstenite` over upgrades the front accepts itself, not axum's `ws` | one message type for the sockets the hub and the relay accept and the one the relay dials to an engine; the frame cap is `MAX_FRAME_BYTES` (16 MiB) on all of them |
| every socket has a reader task and a writer task, the writer's queue counted in bytes | backpressure reads a peer's unsent bytes (`bufferedAmount`); a tunnel counts each channel's apart, so a stalled reader's channel closes alone (`apps/k7-daw-frontend/packages/engine-tunnel/docs/backpressure.md`) |
| the registry's state behind one lock, never held across an await | the claims and the bindings change together; nothing in them waits on a socket |
| `indexmap` for every map whose order shows (a crate serde_json already brought) | the listing's order and the bytes of `engine-bindings.json` are insertion order, as the previous release reads and writes them |

## The testbed

The suites of `apps/k7-daw-frontend/packages/engine-tunnel` — the relay's, the hub's, the two-agent suite — run against the core
when `ENGINE_CORE_BIN` names `k7-engine-testbed` (`apps/k7-daw-frontend/packages/engine-tunnel/testCore.ts`): one process per test, its
routes and sockets the front's own. Without it they skip; `ENGINE_CORE_REQUIRED=1`, as CI sets it, makes
them fail instead.

| choice | why |
|---|---|
| a binary of its own, built only with the `test-hooks` feature, refusing to start without `K7_ENGINE_TESTBED=1` | its hooks call the registry and the hub directly and stop the clock; no shipped build carries them |
| a hook stands for what a test needs set up and nothing more: an `upsert` with a chosen source address, a ticket, a stubbed `verify`, a device table, a stopped clock | the assertions stay the suites' own; what reaches the routes is real |
| a channel a test opens is a one-shot binding dialled on loopback | that is how the seam's callers open one |

## The answers

| choice | why |
|---|---|
| a refusal before the 101 carries `Content-Length` and `Date` | a client reads the status |
| a frame held before a claim or a dial, and the parse bound of a droppable push, are counted in bytes | the bounds are a ceiling on memory, which bytes measure |
| a text frame that `serde_json` cannot read (a lone surrogate escape) is no probe, and is dropped, not stripped, when it may carry the token to an account that may not read it | no engine writes one, and a token must never pass unstripped |
| the frame types a tunnel says it ignored are remembered 256 at most | the types are the agent's to invent |
| a hostname cut inside an emoji loses the emoji whole | as the device names (`routes.md`) |
| an unreadable state file's log line names serde's parse error, a failed file request names the system's | both are logs, read by a person |
| a bound path's tunnel is resolved at the check before the 101; one replaced in between closes the client with 1011 | the client redials, as on any drop |

## Security review

The right column is what fails if the core stops keeping a decision; the TypeScript suites named run
against the testbed (above).

| trust decision | what proves it | pinned by |
|---|---|---|
| the heartbeat is open, so what it may grow is bounded: an id is 1–128 of `[A-Za-z0-9._-]`, a token at most 256, 1000 engines, 30 new ids per address and minute | `read_heartbeat`, `State::admit` | `heartbeat.test.ts`, `trust.test.ts` (caps); `tests/vectors.rs` |
| an id is bound to its first tunnel secret, kept as sha256 in a 0600 file in a 0700 folder and set aside when unreadable; another secret, or none, never lists, tunnels or deregisters it | `registry/bindings.rs`, `state_file.rs` | `trust.test.ts`; a real supervisor run (below) |
| a tunnel opens only for a listed id, its token and its bound secret, checked before the incumbent is touched; below the protocol minimum a 426 | `Hub::check_tunnel` | `trust.test.ts`, `hubLifecycle.test.ts`, `protocol.test.ts`, with both agents |
| the relay dials only where the heartbeat came FROM, through the engine's tunnel first | `hosts_for`, `relay::dial` | `registry/tests.rs`, `relaySocket.test.ts` |
| a ticket is HMAC-SHA256 under a per-process secret, names one engine and one account, lives a minute; the account it names is the one that drives | `registry/ticket.rs` | `ticket.test.ts`; `ticket.rs` unit tests |
| before a claim only the probe crosses the relay; the rest waits 10 s, 256 frames or 8 MiB, then 4403; the claim is asked on every browser frame and reused 250 ms for the engine's; the token leaves a `whoami` for an engine the account may not use | `relay/gate` | `relayGate.test.ts`; `gate/tests.rs` |
| which engine may be claimed is ownership's (it runs on this machine); the claim rules arbitrate the rest | `registry/claims.rs`, `Registry::may_use` | `registry.test.ts`, `registry/tests.rs` |
| a bound loopback path's key is 24 random bytes, refused off loopback (the socket's peer at the front) and once unbound; a key named by its caller comes through the gated seam alone, 32 characters at least | `Hub::check_local`, `routes/engine/internal.rs` | `hubLifecycle.test.ts`, `seamCore.test.ts`; `tests/all/engine.rs` |
| the seam answers loopback peers holding `K7_FRONT_SECRET`, compared in constant time | `routes/engine/internal.rs` | `seamCore.test.ts`; `tests/all/engine.rs` |
| a file crosses only when its path is plain, under a root, every segment below it a safe name (no device name, no `:`, no trailing dot on Windows), and its REAL path inside the root or its library clone | `tunnel/files` | `hubConfinement.test.ts`, the cases of `PathMap_test.cpp` and `segments.test.ts`; `files/tests.rs` |
| an upload lands only in a writable root, through an exclusive random `.part` renamed when complete, bounded per tunnel (16 open, 2 GiB, 20 GiB an hour, 60 s idle) | `tunnel/put.rs`, `uploads.rs` | `hubConfinement.test.ts` |
| a slow peer loses only what a newer frame replaces, and is closed past 64 MiB | `tunnel/backpressure.rs` | `registry/tests.rs`, vectors |
| nothing off a socket is trusted for shape, and nothing read can stop the process | `parse_agent_message`, `decode_binary`; no input reaches a panic | `frames/tests.rs` |

A real C++ supervisor (the dev checkout's, in the desktop app's stdio mode, its `STUDIO_PORTS` naming a side
front with the core on) was listed, bound to its secret on disk, tunnelled, relayed — its `whoami`
before the claim, its pushes after — read through the seam, and unlisted by its deregister when its
stdin closed.

What it does not cover: a link swapped between the real-path check and the open; the LAN path to an
engine.
