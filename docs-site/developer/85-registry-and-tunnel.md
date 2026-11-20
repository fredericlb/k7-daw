# Registry and tunnel

The registry, which knows which engines exist and who holds them, and the tunnel hub, which reaches them, are k7's Rust front: the crate `apps/k7-daw-backend/packages/k7-engine`, routed by `apps/k7-daw-backend/packages/k7-front/src/routes/engine/`, once for the whole origin (`apps/k7-daw-backend/docs/engine.md`). What stays TypeScript is the client side: `@studio/engine-tunnel` the tunnel's wire, its reference agent and the jobs client.

## The registry

`apps/k7-daw-backend/packages/k7-engine/src/`:

| File | What |
|---|---|
| `registry/mod.rs`, `registry/claims.rs`, `registry/list.rs` | The in-memory registry: entries, claims, TTL, the browser's list |
| `registry/heartbeat.rs` | Reading a heartbeat leniently |
| `registry/bindings.rs`, `registry/owners.rs`, `registry/state_file.rs` | id → `sha256(tunnelSecret)` bindings and the owners, on disk (`engine-bindings.json`, `engine-owners.json`) |
| `registry/ticket.rs` | The relay ticket |
| `relay/` | The relay: dialling, piping |
| `relay/gate/` | What a relayed socket may do before its account holds a claim |

The HTTP routes are `k7-front`'s `routes/engine/registry.rs` and `ownership.rs`; the pages are `apps/k7-daw-backend/docs/engine/`.

### Routes (under `/api`)

| Route | Auth | Does |
|---|---|---|
| `GET /registry/hello` | open | `{service: "k7-registry"}`; the supervisor asks before sending its token anywhere |
| `POST /registry/heartbeat` | open | Upsert an engine. Only `id` (letters, digits, `.`, `_`, `-`; ≤128), `token` and `port` are required; other fields are cleaned and the cleaning logged rather than the beat refused. Refusals: 403 bound to another secret, 409 token mismatch, 503 registry full, 429 too many new ids from this address. |
| `POST /registry/deregister` | open, but `{id, token, tunnelSecret}` must match | Remove an engine now |
| `GET /engines` | session | The list, tokens stripped, each with a fresh relay ticket for the caller, and `rtc: true` when a tunnel is up. Claims are named only to their own account; others read as `assigned`. |
| `POST /engine/:id/rtc` | session | Forward a browser's SDP offer through the engine's tunnel; 503 when it cannot |
| `POST /engine/select` | session | Claim an engine for a project, or renew (`{id, token, project, secret?}`). 404 when the heartbeat lapsed, 403 bad token, 409 `{heldBy, lost, tookFor}` when the claim moved on. The holder comes from the session, never the body. |
| `POST /engine/release` | session | Drop a claim (`{id, secret}`); always 200, since it arrives by `sendBeacon` |

### Entries and claims

Engines expire 15 s (`REGISTRY_TTL_MS`) after their last heartbeat. The list carries what a picker shows: host name, addresses, performance, library level, `assigned`, and the caller's own held projects.

The claim rules: one project per engine and session, one engine per project, and the newcomer wins. `select` without a secret opens a claim and displaces whoever held it (returning `tookOver`); a renewal with the secret is refused once the claim has moved, with the reason (`project`, `engine`, `lapsed`). Claims expire after `ENGINE_ASSIGN_TTL_MS` (90 s) without renewal. The browser side is `apps/k7-daw-frontend/packages/engine-browser/src/lease.ts` ([84](./84-engine-clients.md)).

### Identity and the tunnel secret

The engine token is handed to the engine and returned by `whoami`, so every browser that probes the engine can read it; it cannot be what proves a machine. The supervisor therefore also sends a **tunnel secret** with every heartbeat, deregister and tunnel. `registry/bindings.rs` binds an id to the first secret seen (trust on first use) and persists the digest, so the binding outlives the 15 s expiry and a server restart. From then on, a heartbeat, a tunnel or a deregister for that id must carry that secret; a bound id's token may change. A supervisor that sends no secret is listed as `legacy`, held by its token alone. `trust.test.ts` is the contract.

### The relay

An `https:` page may not open `ws://` to a LAN host, so a browser on another network reaches an engine through `wss://<origin>/api/engine/<id>/ws?ticket=…`:

1. The ticket is an HMAC over `{id, holder, exp}` with a per-process secret, valid one minute, minted by `GET /engines` for the asking account.
2. The relay dials the engine: through its tunnel when one is up, otherwise the address its heartbeat came from (`hostsFor`), never an address the heartbeat names, since the heartbeat route is open and must not aim the server anywhere.
3. The gate (`relay/gate/`) sits in the pipe. Until the ticket's account holds a claim on the engine, only `whoami`, `pair` and `list-plugins` go to the engine and only `whoami`, `plugins` and `error` come back; anything else is held (bounded) and the socket is closed with 4403 if no claim lands within 10 s. The claim is checked on every frame afterwards, so a takeover stops the loser at its next frame.
4. Frames are capped at `MAX_FRAME_BYTES` (16 MiB).

## The tunnel

The server end is `apps/k7-daw-backend/packages/k7-engine/src/tunnel/` (`hub.rs`, `hub_tunnel.rs`, `files/`, `put.rs`, `uploads.rs`): it accepts supervisors on `/api/engine-tunnel`, opens channels and serves files. The rest of the wire's TypeScript is `apps/k7-daw-frontend/packages/engine-tunnel/`:

| File | Subpath | What |
|---|---|---|
| `agent.ts`, `agentFiles.ts`, `pathMap.ts` | `./agent` | The TypeScript reference agent (the supervisor's C++ agent is the one that ships) |
| `channel.ts` | `./channel` | `TunnelChannel`: one virtual connection, shaped like a `ws` WebSocket |
| `frames.ts` | `./frames` | The wire, with validators for every message |
| `jobsClient.ts` | `./jobs-client` | `requestJob`: one request over a jobs channel |
| `rtc.ts` | `./rtc` | The TypeScript WebRTC answerer (node-datachannel, optional and loaded lazily) |
| `drain.ts` | | Backpressure: wait while more than 1 MiB is unsent |

### The wire

One WebSocket per supervisor and server. JSON control messages are text frames; anything carrying bytes is a binary frame whose first four bytes give the length of a JSON header, followed by the payload. File chunks are 256 KiB.

| Hub → agent | Agent → hub | Binary headers |
|---|---|---|
| `open {ch, roots, kind, synced?}` | `opened {ch}` | `data {ch}` (engine bytes) |
| `close {ch}` | `close {ch, reason?}` | `chunk {req, last}` (a download) |
| `data {ch, text}` | `data {ch, text}` | `put {req, ch, path, last}` (an upload) |
| `stat-reply`, `get-error`, `put-reply` | `stat {req, ch, paths}`, `get {req, ch, path}` | |
| `rtc-offer {req, sdp}` | `rtc-answer {req, sdp?, error?}` | |

The agent authenticates with headers, never the query (so no access log keeps them): `x-studio-engine-token`, `x-studio-tunnel-secret`. The hub asks the registry; a refused newcomer leaves a live tunnel in place, and a verified reconnect replaces the old socket.

### Channels

A channel is `engine` (piped to the local engine socket) or `jobs` (answered by the agent). Server code gets one of two ways:

- `Hub::open` / `Hub::open_ready` for the front's own routes;
- a loopback path `/api/engine-tunnel/local/<key>` (`Hub::bind_local`), which a caller asks for through the internal seam's `POST /bindings` and dials as an ordinary `ws://` URL, the random key being its only credential.

### Path rewriting

A **root** (`TunnelRoot`) is a server directory shared with a channel: `remote` (absolute POSIX path on the server), `name` (a folder name under the agent's cache, never a path), `writable`, and `perChild` for library folders whose children are separate clones. `PathMap` rewrites whole JSON strings under a root in both directions as frames cross:

- toward the engine, a string naming a server file becomes the local cache path, and the file is downloaded (if missing or older by mtime) **before** the frame is forwarded; one queue per direction keeps frames in order;
- toward the server, a string naming a local file under a writable root becomes the server path, and the file is uploaded first.

A `synced` channel (the server copied every file already) is only rewritten. The hub confines every stat, get and put to the channel's roots, checked on the real path after resolving symlinks.

### Jobs

`requestJob(sock, body, isLast, …)` sends one request and resolves with the reply `isLast` accepts, failing with `JobError` on an `error` reply, a closed channel, an hour in total (`JOB_TIMEOUT_MS`) or 15 minutes of silence (`JOB_IDLE_MS`). Callers in this repository:

| Job | Caller |
|---|---|
| `separate` | `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/jobs/separate.rs`, `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/verb_host/stems.rs` |
| `prefetch` | `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/media/prefetch.rs`, `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/journal_undo.rs` |
| `send-up` | `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/journal_undo.rs` |
| `local` | `apps/k7-daw-backend/packages/k7-front/src/routes/engine_reach/engine_jobs.rs` |
| `render` | `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/jobs/freezer.rs` |
| `log` | `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/runtime/logs.rs` |
| `onnx-providers` | `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/verb_host/onnx.rs` |

The machine side of each job is in [82](./82-k7-supervisor.md#tunnels-jobs-and-webrtc). The file service learns which jobs path belongs to its bound engine through `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/jobs/machine.rs`.

To add a job: add a handler to `machineJobs()` in `apps/k7-daw-engine/k7-supervisor/src/MachineJobs.cpp` (and to the reference agent's tests if the TypeScript agent should answer it), confine any input path to the machine's copies of the server's folders, and call it with `requestJob` from the server side.

## Tests

```sh
cargo build --manifest-path apps/k7-daw-backend/Cargo.toml -p k7-front --features test-hooks --bin k7-engine-testbed
ENGINE_CORE_BIN=<that binary> npm test --workspace @studio/engine-registry
ENGINE_CORE_BIN=<that binary> npm test --workspace @studio/engine-tunnel
```

The registry suites cover heartbeat cleaning, claims, the relay and its gate, and trust bindings; they run against the Rust core, started per test from `ENGINE_CORE_BIN` (`apps/k7-daw-backend/docs/engine.md`, "The testbed"), and skip without it unless `ENGINE_CORE_REQUIRED=1`, as CI sets. The tunnel suites (`tunnel.test.ts`, `jobs.test.ts`, `hubConfinement.test.ts`, `pathMap.test.ts`, `rtc.test.ts`, `renderKey.test.ts`, `frames.test.ts`, `jobsClient.test.ts`, `drain.test.ts`) run that hub against the agents listed in `testAgents.ts`: the TypeScript agent always, and the native supervisor's `tunnel_harness` when a build of it exists, so both agents are held to one wire. The native agent is skipped, not failed, when it is not built.
