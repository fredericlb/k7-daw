# The Rust front

`k7-front` is the first piece of k7's backend in Rust, and the way every later piece arrives: it
listens where the backend listened, answers the routes already ported, and proxies every other
request to the TypeScript backend unchanged. A route family moves across by being answered here
and deleted there; the plan and its phases are `docs/rust-backend-plan.md`.

## The choices

| choice | why |
|---|---|
| a strangler in front, never a second backend beside it | one address, one cookie, one CORS policy for the browser while both languages serve; a route family moves in one commit, and moving it back is reverting that commit |
| the routes it owns are an axum `Router`, the proxy its fallback | a ported route is a line in `build()`, and nothing else in the front changes |
| it speaks plain HTTP/1.1 to the backend, on loopback | the backend is Express on node's `http`; a second protocol between two processes on one machine buys nothing |
| bodies stream both ways, never buffered | uploads reach 2 GB (app releases) and the event streams (the local engine's) must reach the page frame by frame; the conformance suite checks the first frame arrives before the stream ends |
| a WebSocket upgrade it does not own is forwarded, then the two sockets are spliced byte for byte | the engine relay and the tunnel keep their own framing and back-pressure in TypeScript until each is ported; the front never reads their frames |
| the front's own accept loop (`src/listen.rs`): hyper-util's auto builder with a timer; 60 s for a request's headers and for the wait between requests on a kept connection, 30 minutes for a request's body, 4096 connections at once | node's `headersTimeout` and `requestTimeout` (`server.ts`). `axum::serve` gives hyper no timer, and without one hyper enforces no header timeout: a client that opens connections and sends nothing would hold each for good. Past the cap the front stops accepting rather than refusing, so a burst waits instead of failing; an upgraded socket leaves the count |
| a backend that does not answer is a 502 with a sentence, never a hang | a host's readiness probe, and a person, can tell "the front is up, the backend is not" |
| the client's address is decided HERE, with Express's `trust proxy` arithmetic (`K7_FRONT_TRUST_HOPS`), and the backend gets it as the single `X-Forwarded-For` entry | the backend keeps `trust proxy` 1 and reads the same `req.ip` it read before the front existed; the rate limits, the registry's same-network listing and the share player's per-address cap are unchanged |
| with no trusted hop, `X-Forwarded-Host` is dropped and `X-Forwarded-Proto` is `http` | a client must not choose the origin the backend derives its redirect URIs from |
| a forwarded address that names loopback, from a socket that is not loopback, is replaced by the socket's own | a hop count trusts whatever stood one hop away: on a network shared with other containers, any of them could otherwise claim to be this machine |
| "this machine" — a registry engine's `loopback_seen`, so its ownership by every account and the relay dialling `127.0.0.1` — is a loopback client address, which only a loopback socket gives (above); a hint naming loopback is never dialled on its own | an engine on this machine is every signed-in account's; that must not be one header away. The socket alone would not do either: behind the dev session's Vite every socket is loopback, a LAN engine's included |
| `K7_FRONT_TRUSTED_PROXIES` (CIDRs) believes `X-Forwarded-*` only from a socket in the set, and walks `X-Forwarded-For` past the set's members, as Express's trust function does; a peer outside it has every forwarded header dropped | a hop count cannot tell Traefik from a neighbour on its network; a set can. Unset, the hop arithmetic stands, since the deployment's proxy address is not known to the image |
| a debug build optimizes its dependencies (`opt-level = 2` for every non-workspace package), the tray's workspace too | the front runs the song compiler and the WASM modules in Wasmtime: at opt-level 0 a conformance run takes two to three times node's; the workspace's own crates stay unoptimized, so an edit still rebuilds in seconds |
| what the state keeps never holds the state: a door's routes are kept without it and given it per request, a middleware or hook kept there holds it weakly | a router given the state, or a closure holding it, kept inside that state keeps it alive for good: the families' services, sockets and child processes outlive a stopped front, and a test binary never exits; `src/stop_tests.rs` stops a front after each door answered and checks the state is let go |

## The roots it trusts

| choice | why |
|---|---|
| every outbound TLS call (Claude, the git listings) is built from one root store, `k7_front::trust` | a self-hosted server under the person's own CA is reached by all of them, or by none |
| Mozilla's roots (`webpki-roots`), plus the PEM file the host names in `K7_EXTRA_CA_CERTS`, else `NODE_EXTRA_CA_CERTS` | node trusted its own bundle plus `NODE_EXTRA_CA_CERTS`; a host that set it for node (Android's `TrustedCas`) keeps working with node gone. A file is a dependency nothing else is: `rustls-pki-types`' PEM reader is already rustls' |
| no OS store | node read none, so no host lost it; the OS store needs `rustls-native-certs`, a crate not taken. The supervisor reads the OS store on a desktop for the engine's own calls (`engine/k7-supervisor`) |
| read once, at the front's start, before any client is built; an unreadable file is said and leaves Mozilla's roots | node read its file at start too; a trust store that moves under running clients would be two answers for one server |

## The peer travels with a secret

Behind the front, every socket the TypeScript backend accepts is loopback. Its loopback locks
(`apps/k7/backend/src/loopback.ts`, which a device's backend runs ahead of every route) read the peer
address, so without care **every request would pass them**.

| choice | why |
|---|---|
| the front sends the socket's peer in `x-k7-peer`, with `x-k7-front: <K7_FRONT_SECRET>` | the backend can believe a header only when it knows who wrote it |
| the backend records a vouched peer on the request under a symbol (`FRONT_PEER`, `peerAddress()` in `@studio/web-protocol/services`), and every address check reads `peerAddress(req)` | a client can set any header, never a symbol; a check that forgot `peerAddress` reads the socket, which is still the old behaviour |
| a request that names the front with a wrong secret — or reaches a backend that was given none — gets the peer `front-unverified`, and fails every loopback check | the dangerous misconfiguration is a front running before a backend that does not check it; it fails closed instead of opening every lock |
| a request with no front header is read from its socket | the file services and the local engine call the backend on its own loopback port, bypassing the front, and must keep doing so |
| the front refuses to start without `K7_FRONT_SECRET` when a backend is named (`Config::settled`, every host's path) | there is no setting in which a front before a backend, without one, is safe |
| with no backend named it starts only once `K7_LAST_ROUTES`, `K7_FILES` and `K7_ENGINE_CORE` are all `front`, draws its own seams' secret per process, and answers what it does not own 502 `no backend behind this front` | nothing is behind it to trust or be trusted by; the seams it asks of itself still need a secret no client knows, and a route nobody answers must not wait on a connection to nowhere |
| the front strips `x-k7-front` and `x-k7-peer` from what a client sent, always | a client's guess never reaches the backend, even one that matched |

## Every end leaves a line

A front that stops answering with nothing in its log cannot be told from one killed from outside. Each
way it ends is said, by the front where it still can, by whoever started it where it cannot (`src/exit.rs`).

| how it ends | the line | written by |
|---|---|---|
| a stop it was sent: SIGINT, SIGTERM, SIGHUP; on Windows Ctrl-C, Ctrl-Break, the console closing, a logoff, a shutdown | `k7-front: stopping on <which>`, then `k7-front: stopped` | the binary (`main.rs`) |
| a panic, in any thread: a request's task, the hub's | `k7-front: panic in thread '<name>': <where and what>` and a backtrace | the hook `exit::watch` installs |
| `abort()`: the C runtime's asserts, a Rust abort on Unix | `k7-front: aborted …`, a fixed text | a SIGABRT handler |
| a crash Windows hands to nobody (an access violation in C) | `k7-front: crashed, exception 0x… at 0x…` | the last-chance exception filter |
| a stack overflow | Rust's own `has overflowed its stack` | the standard library |
| a kill from outside, or any of the above | the exit code, spelled (`0xc00000fd: a stack overflow`) | the host: Tilt's `dev-front.mjs`, the image's `start.sh` (which of the two processes ended, and its code or signal), the test beds |

| choice | why |
|---|---|
| the lines go where the host keeps the front's: stderr for the binary, the merged log under `[front]` for the tray, logcat under `k7-front` for Android, with every other line the front prints (`k7-embed`'s `logcat.rs`) | the tray and the phone run the front in their own process, whose stderr nobody reads |
| SIGABRT only on Unix, no SIGSEGV handler | Wasmtime owns SIGSEGV and SIGBUS for its traps; a launcher reads the signal from the exit status |
| on Windows a last-chance filter, not a vectored handler | Wasmtime's traps and Rust's stack-overflow check are first-chance handlers, and must see their faults first |
| a signal handler writes a fixed text with `write`/`WriteFile` | nothing may allocate inside `abort()` or a crash |
| a console's Ctrl-C is named | it reaches every process attached to that console: a front started in the background of a script stops with the script's own Ctrl-C, gracefully and, without this line, silently |
| a test bed under Git Bash starts the front through node, not `( front; echo $? )` | Git Bash's `$?` reads a Windows status (`0xc0000409`, `0xc00000fd`) as 127; node's `exit` event gives the real one |
| a release build is stripped, so its backtrace is addresses; the panic's file and line are still said | the location is part of the panic, not of the debug info |

## Configuration

| variable | default | |
|---|---|---|
| `K7_FRONT_LISTEN` | `127.0.0.1:8789` | where it listens |
| `K7_FRONT_UPSTREAM` | `127.0.0.1:8790`; none when `K7_LAST_ROUTES`, `K7_FILES` and `K7_ENGINE_CORE` are all `front` | the TypeScript backend's `host:port` |
| `K7_FRONT_SECRET` | required with a backend; drawn per process without one | the same value as the backend's `K7_FRONT_SECRET`, and the front's own seams' |
| `K7_ENGINE_CORE` | unset | `front` answers the engine registry, the relay, the tunnel hub and the seam's engine half here (`engine.md`) |
| `K7_LOCAL_ENGINE` | unset | `front` on a device runs the machine's supervisor here (`local.md`) |
| `K7_LAST_ROUTES` | unset | `front`, with the file hub here, answers the backend's last routes, so nothing goes to node (`last.md`) |
| `K7_FRONTEND_DIST` | `apps/k7/frontend/dist` above the front | the built page the last routes serve |
| `K7_DEV_FRONTEND` | unset | a running Vite the last routes stream the page from, as the dev tray's backend does |
| `K7_EXTRA_CA_CERTS` | unset, else `NODE_EXTRA_CA_CERTS` | a PEM file of certificate authorities every outbound TLS call trusts beside Mozilla's roots, read once at start |
| `K7_FRONT_TRUST_HOPS` | `0` | proxies before the front whose `X-Forwarded-*` is believed: `1` behind Traefik or the dev Caddy |
| `K7_FRONT_TRUSTED_PROXIES` | unset | the proxies' addresses or CIDRs, comma- or space-separated; set, only they are believed and the hop count is 1; an entry that is not one stops the front |

The routes it owns read the TypeScript backend's own settings under the same names (`ENV_KEYS` in
`src/config.rs`: `SESSION_SECRET`, `STUDIO_LOCAL_USER`, the prefs and auth folders…),
so a host gives both processes the same values.

The same fields, camelCased, are the JSON a host hands `k7_front_start` (`embed.md`), with an `env`
object for those settings: an embedded front cannot be handed an environment of its own.

## Where it runs

| host | the front | node behind it |
|---|---|---|
| the dev session | the Tiltfile's `k7-front`, on 127.0.0.1:8789, one trusted hop (Vite) — `docs/dev.md` | `k7-backend` on 127.0.0.1:8790 |
| the tray | linked into the tray, on 18789 — `apps/k7-tray/docs/tray.md` | a loopback port drawn per run |
| the server image | `k7-front` on 8789, one trusted hop (Traefik) — `docs/images.md` | 127.0.0.1:8790, both under `apps/k7/docker/start.sh` |
| Android | `libk7_embed.so` in the app's process, on the studio's port — `engine/k7-runtime/docs/android.md` | a loopback port drawn per process |

The backend is told `K7_FRONT_PORT`, the port pages reach, so what it hands out of its own address — the
supervisor's pin under the tray — names the front and not node.
