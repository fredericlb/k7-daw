# Architecture overview

k7 runs as five kinds of process on one device: a **backend**, which serves the page and holds the
device's files, a **browser** that draws it, and an **engine machine**, which makes the sound. On a
laptop or a phone the three are the same machine: k7 is the tray or the app, and there is no server.

```text
                        the device (one origin)
 ┌───────────────────────────────────────────────────────────────────────┐
 │  k7 backend (apps/k7/backend, Express, port 8789)                     │
 │   ├─ /auth/*        local user, mints the studio.sid session          │
 │   ├─ /api/files/*   file hub, the account's file service (in the Rust front)
 │   ├─ /api/engines   engine registry                                   │
 │   ├─ /api/engine    engine relay (WebSocket)                          │
 │   ├─ /api/engine-tunnel   tunnel hub  <───────────────┐               │
 │   ├─ prefs                                            │               │
 │   └─ the built SPA (apps/k7/frontend/dist)            │               │
 └───────────────────────────────────────────────────────┼───────────────┘
        ▲  HTTPS + WebSocket                             │ supervisor dials out
        │                                                │
 ┌──────┴────────┐                         ┌─────────────┴───────────────┐
 │ browser: k7   │ ── optional WebRTC ───> │ engine machine              │
 │ frontend SPA  │                         │  k7-supervisor ──> k7-runtime│
 └───────────────┘                         └─────────────────────────────┘
```

## The processes

| Process | Code | Runs where | Job |
|---|---|---|---|
| k7 frontend | `apps/k7/frontend` | Browser | The editor and the home page. A React SPA built by Vite. |
| k7 backend | `apps/k7/backend` | Device | One Express process behind k7's Rust front: the session it reads, preferences, the bearer tokens and the static SPA. The engine registry and relay, the tunnel hub and the file hub are the front's (`apps/k7/backend-rs/docs/`). |
| file service | `apps/k7/backend-rs/crates/k7-file-service` | Device (in the Rust front) | One runtime for the local account, made on demand by the front's file hub. Every file operation on the device's workspaces, the project compile, and the commands that drive an engine. |
| k7-supervisor | `engine/k7-supervisor` | Engine machine | Starts and watches the engine, lists it at the registry, holds the tunnel back to the backend, runs jobs the backend asks of the machine. |
| k7-runtime | `engine/k7-runtime` | Engine machine | The C++ audio engine. Receives a compiled project document over WebSocket, plays it, reports transport and meter state. It never reads song files. |

### The k7 backend

`apps/k7/backend/src/server.ts` is the whole entry point. In order, it:

1. layers its `env.yaml` section (`k7-backend`) into `process.env` with `loadServiceEnv`;
2. answers CORS preflights for every origin;
3. calls `setupAuth()` from `@studio/oidc` with cookie name `studio.sid`, for `/auth/me` only (code kept for a later rework, `docs/inventaire.md`): the
   logout is the Rust front's (`apps/k7/backend-rs/docs/routes.md`);
4. mounts the bearer tokens and the internal seam's file half (`apps/k7/backend/src/media/mount.ts`);
   the engine registry, the tunnel hub and the file hub are the Rust front's;
5. mounts per-account preferences at `/api/preferences`;
6. redirects pages on the `play.` and `k7.` host aliases to k7's home (`/`);
7. serves `apps/k7/frontend/dist` with hashed assets cached for a year and the shell never cached.

The front answers the file hub only with `STUDIO_FILES_ROOT` set (`apps/k7/backend-rs/crates/k7-front/src/routes/files/mod.rs`),
because that folder is the only place workspaces live. On `SIGTERM` the backend lets in-flight requests finish within four seconds.

### File services: one runtime per account

The Rust front holds each account's file service (`apps/k7/backend-rs/crates/k7-file-service`, wired by
`crates/k7-front/src/routes/files/service/`) as state in memory, made on first use and let go when idle. A
runtime of its own per account is the isolation boundary: one person's state never meets another's. The hub
authenticates each request with a signed ticket or the session and stamps only the identity it verified.

The file service drives an engine through its engine link (`crates/k7-file-service/src/engine/`). The hub binds
an account to the engine that account **claimed** (`apps/k7/backend-rs/crates/k7-hub/src/binding.rs`),
reaching it through that engine's tunnel; the tunnel hub gives a loopback address bound to the
remote engine, so the link speaks plain `ws://` and does not know how far away the engine is.

### Engine machines and the tunnel

An engine machine never accepts inbound connections from the server. Its supervisor dials the
backend at `/api/engine-tunnel` (`apps/k7/backend-rs/crates/k7-engine/src/tunnel/` is the server end, the C++
supervisor the machine end, `packages/engine-tunnel/agent.ts` its TypeScript reference), and
everything the server needs from the engine travels as channels over that socket.

A browser reaches an engine in one of two ways:

- **The relay**: a WebSocket to `/api/engine` on the origin, piped through the tunnel
  (`apps/k7/backend-rs/crates/k7-engine/src/relay/`). This exists because an HTTPS page may not open `ws://`
  to a LAN address.
- **WebRTC**: the offer and answer travel over the tunnel and the supervisor answers with a data
  channel piped to the local engine (`packages/engine-tunnel/rtc.ts`).

The engine registry (`apps/k7/backend-rs/crates/k7-engine/src/registry/`, answered by k7's Rust front) lists
every engine that heartbeats to the origin.
The browser's single entry to an engine is `@studio/engine-browser/access`.

Where engine machines run:

| Form | Code |
|---|---|
| Command-line engine (`k7-supervisor --config <file>`) | `engine/k7-supervisor`, staged by `engine/scripts/build-engine.mjs` |
| K7 DAW | `apps/k7-tray`: a Tauri app with a window and a tray icon that runs k7's backend on `127.0.0.1:18789`; its Rust front runs the supervisor (`k7-local`) and the file services, and the window (or the browser) shows k7 |
| Android app | `engine/k7-android`: the engine as a library, the supervisor as a program, and k7's backend bundled |
| Server engine | `engine/docker/Dockerfile`: a container that dials k7 over the compose network and plays into a PulseAudio null sink |

## Data flow for one edit

1. The browser edits a file through `/api/files/*`; the file hub hands it to the account's file service.
2. The capture (`apps/k7/backend-rs/crates/k7-capture`) records the edit as an operation in the workspace's log
   and orders it in the workspace's own history room.
3. The file service recompiles the project folder (`@studio/song`) to a `ProjectDoc` and `GraphDoc`.
4. It sends the document to the bound engine over the tunnel, using the wire format in `@studio/engine-protocol`.
5. The engine diffs it against what it is playing and applies the minimal change; transport and
   meter frames come back to the browser through the relay or WebRTC.

The song pipeline, the engine, and the backend each have their own section in this guide with the
details of these steps.

## What runs in production

k7 runs on devices: the tray and the phone apps carry the backend, bundled, and run it locally (see
[Releases and publishing](./09-releases-and-publishing.md)). The k7 image only serves `k7.rocks` and `docs.k7.rocks` (see [Images and deployment](./08-images-and-deployment.md)).
