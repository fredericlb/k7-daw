# Engine architecture

This section covers everything that links a k7 page to sound: the C++ engine, its Rust supervisor, the wire between them and the browsers, the registry that lists engines, the tunnel that reaches them, and the three hosts that ship them (the desktop app, the phone, the server image). This page is the map; the following pages go into each part.

| Page | Covers |
|---|---|
| [81 — The C++ engine](./81-k7-runtime.md) | `apps/k7-daw-engine/k7-runtime`: layout, CLI modes, the server, the diff, sessions, hosted plugins, the Strudel bundle, tests |
| [82 — The supervisor](./82-k7-supervisor.md) | `apps/k7-daw-backend/packages/k7-supervisor`: run modes, the host protocol, settings, registry, tunnel agent, machine jobs |
| [83 — The engine protocol](./83-engine-protocol.md) | `apps/k7-daw-frontend/packages/engine-protocol` and the C++ handlers: every message, the state frame, the ProjectDoc, sessions, the audio stream |
| [84 — Engine clients](./84-engine-clients.md) | `apps/k7-daw-frontend/packages/engine-client` (Node) and `apps/k7-daw-frontend/packages/engine-browser` (browser): access, probe, choice, lease, store, pairing, WebRTC |
| [85 — Registry and tunnel](./85-registry-and-tunnel.md) | `k7-engine` (the Rust registry, relay and hub) and `apps/k7-daw-frontend/packages/engine-tunnel`: heartbeats, claims, the relay, the hub and agent, jobs |
| [86 — The desktop app and the phone apps](./86-desktop-and-android.md) | `k7-local`, `apps/k7-daw-desktop`, `apps/k7-daw-android` |
| [87 — Building and shipping engines](./87-engine-build-and-shipping.md) | `apps/k7-daw-engine/scripts`, CMake presets, release archives |

## The processes

```text
 browser (k7 page)                       k7 front (apps/k7-daw-backend)
 ├─ @studio/engine-browser  ── HTTPS ──► ├─ engine registry  (/api/registry/*, /api/engines, /api/engine/*)
 │      │                                ├─ engine relay     (/api/engine/<id>/ws)
 │      │                                ├─ tunnel hub       (/api/engine-tunnel)
 │      │                                └─ file hub → file service per account (@studio/engine-client)
 │      │                                          ▲
 │      │   ws / wss / WebRTC                       │ one outbound WebSocket per studio (the tunnel)
 │      ▼                                          │
 engine machine ───────────────────────────────────────────────────────────── ├─ k7-supervisor   (Rust) — starts/adopts/watches the engine, heartbeats, tunnel agent, jobs
 └─ k7-runtime      (C++) — WebSocket server on :8765, Tracktion Edit per session, audio device
```

Two choices shape all of it.

**The engine never parses song files.** The file service compiles a project folder into a `ProjectDoc` (JSON) and sends that. The engine diffs it against what it is playing and changes only what moved. Any app that can build a `ProjectDoc` can drive the engine: k7's editor, the DJ desk, a share player, a one-track module preview. This keeps YAML, schemas and the compiler in TypeScript and keeps the C++ side a player.

**The machine dials out.** An engine machine is usually behind a home router. Rather than ask the musician to open a port, the supervisor holds one outbound WebSocket to every studio it is listed at, and the studio routes everything it needs through it: the relayed browser socket, the project files, machine jobs, and the WebRTC offer that lets a browser connect peer to peer.

## How a browser reaches an engine

`@studio/engine-browser`'s probe races every candidate address and keeps the fastest that answers `whoami` with the expected id:

| Candidate | URL | Offered when |
|---|---|---|
| Loopback | `ws://127.0.0.1:<port>` | always |
| LAN | `ws://<lanIp>:<port>` | the page is `http:` (an `https:` page may not open `ws://` to a LAN host: mixed content) |
| Relay | `wss://<studio>/api/engine/<id>/ws?ticket=…` | the registry minted a relay ticket (always, per listing) |
| WebRTC | `rtc:<studio>/api/engine/<id>/rtc` | the engine has a tunnel up |

The relay dials the engine through its tunnel first, then the address its heartbeat came **from** (never an address the heartbeat names, since the heartbeat route is open). A relay ticket is bound to one engine and one account; behind it a `RelayGate` lets an account with no claim on the engine send only the probe verbs (`whoami`, `pair`, `list-plugins`) and closes the socket if no claim arrives within ten seconds. WebRTC uses no STUN or TURN servers on either side (`iceServers: []`), so it connects only with host candidates, which in practice means the same network. It exists so an `https:` page can reach a LAN engine directly without the relay.

Over a direct LAN connection, the engine requires pairing ([83](./83-engine-protocol.md#pairing)); loopback peers and relayed traffic (which arrives from the supervisor on loopback, already authenticated by the server) are trusted. Independently of the peer address, a **browser page** is only served when its `Origin` is allowed: loopback, `https://k7.rocks` and its subdomains, or a studio the supervisor is listed at.

## Trust

Anyone may run an engine and heartbeat it to an open registry, so trust is layered:

| Layer | Rule | Code |
|---|---|---|
| Identity | `engine-id.json` holds an id, a token (handed to the engine, echoed on `whoami`, readable by browsers) and a **tunnel secret** the supervisor alone sends. The registry binds an id to the first secret it sees, on disk; the token alone never takes a bound id back. | `apps/k7-daw-backend/packages/k7-supervisor/src/identity.rs`, `apps/k7-daw-backend/packages/k7-engine/src/registry/bindings.rs` |
| Registry admission | At most 1000 engines listed, 30 new ids per source address per minute | `apps/k7-daw-backend/packages/k7-engine/src/registry/mod.rs` |
| Browser page | Origin allow-list | `apps/k7-daw-engine/k7-runtime/server/Pairing.cpp` (`originAllowed`) |
| LAN device | Pairing | `apps/k7-daw-engine/k7-runtime/server/ServePairing.cpp` |
| Relay | Ticket bound to account; claim required beyond probing | `apps/k7-daw-backend/packages/k7-engine/src/registry/ticket.rs`, `apps/k7-daw-backend/packages/k7-engine/src/relay/gate/` |
| Auto-pick | Only the account's remembered engine, one reached directly, or the server's own engine is taken without a click | `apps/k7-daw-frontend/packages/engine-browser/src/choose.ts` (`mayAutoPick`) |

## Where a project's audio files come from

The account's files live on the server. When a project opens on an engine, the file service opens an engine channel through the hub with **roots**: server directories shared with the channel. The agent's `PathMap` rewrites every JSON string under a root to a path in its local cache (`<state>/remote/<server>/…`) and **downloads the file before forwarding the frame that names it**. Files the engine writes (a take) are uploaded before the frame naming them reaches the server. A channel the server has already synced is only rewritten. See [85](./85-registry-and-tunnel.md#path-rewriting).

## Where the code lives

| Path | What |
|---|---|
| `apps/k7-daw-engine/k7-runtime/` | The engine (C++20, JUCE via Tracktion Engine, IXWebSocket) |
| `apps/k7-daw-backend/packages/k7-supervisor/` | The supervisor (Rust: tokio, rustls, tokio-tungstenite, hyper, `ort` over ONNX Runtime) |
| `apps/k7-daw-android/` | The Android app (Kotlin, Capacitor) |
| `apps/k7-daw-engine/scripts/` | Build and packaging scripts for the engine and supervisor |
| `apps/k7-daw-frontend/packages/engine-protocol/` | Wire types, ProjectDoc types, sessions, the binary audio frame |
| `apps/k7-daw-frontend/packages/engine-client/` | The Node client and `~/.k7` path helpers |
| `apps/k7-daw-frontend/packages/engine-browser/` | The browser client, React stores, access/probe/choose/lease, pairing, WebRTC socket, audio stream player |
| `apps/k7-daw-backend/packages/k7-engine/` | The registry, the relay, the tunnel hub (routed by `k7-front`'s `routes/engine/`) |
| `packages/engine-registry/` | The internal seam's paths and client, through which TypeScript reaches the registry and the hub |
| `apps/k7-daw-frontend/packages/engine-tunnel/` | The tunnel's wire, the TypeScript reference agent, jobs client, WebRTC answerer |
| `apps/k7-daw-backend/packages/k7-local/` | Running the supervisor as a child of k7's Rust front under the desktop app |
| `apps/k7-daw-desktop/` | The desktop app (Tauri), k7 on a computer |

## Two implementations, one wire

Several contracts have a C++ side and a TypeScript side kept equal by hand:

| Contract | C++ | TypeScript |
|---|---|---|
| ProjectDoc | `apps/k7-daw-engine/k7-runtime/engine/ProjectDoc.h`, `apps/k7-daw-engine/k7-runtime/server/DocJson.cpp` | `apps/k7-daw-frontend/packages/engine-protocol/project.ts` |
| Host protocol (supervisor ↔ app) | `apps/k7-daw-backend/packages/k7-supervisor/src/protocol/mod.rs` | the Rust `apps/k7-daw-backend/packages/k7-local/src/protocol.rs`, the Kotlin in `apps/k7-daw-android` |
| `~/.k7` layout | `apps/k7-daw-backend/packages/k7-supervisor/src/paths.rs` | `apps/k7-daw-frontend/packages/engine-client/src/paths.ts` |
| URL rules (registry, family, cache key) | `apps/k7-daw-backend/packages/k7-supervisor/src/urls.rs` | `apps/k7-daw-frontend/packages/web-protocol/services.ts` |
| Tunnel agent | `apps/k7-daw-backend/packages/k7-supervisor/src/tunnel/agent.rs` | `apps/k7-daw-frontend/packages/engine-tunnel/agent.ts` |

TypeScript cannot check a wire change against C++. When you change one side, change the other in the same commit and verify by running the engine. The tunnel is the exception that has a shared test: `apps/k7-daw-frontend/packages/engine-tunnel` runs its suites against both agents (see [85](./85-registry-and-tunnel.md#tests)).
