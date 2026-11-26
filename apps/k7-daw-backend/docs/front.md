# The Rust front

`k7-front` is k7's backend: it listens on a device's loopback port (8789 by default) and answers the
session, the preferences, the file hub, the engine core, the apps' downloads and the built page. A request
no family takes ends in the last routes (`last.md`), never in another process. The plan and its phases are
`docs/rust-backend-plan.md`.

## The choices

| choice | why |
|---|---|
| the routes it owns are an axum `Router`, the last routes its fallback | a route is a line in `build()`, and nothing else in the front changes |
| the front's own accept loop (`src/listen.rs`): hyper-util's auto builder with a timer; 60 s for a request's headers and for the wait between requests on a kept connection, 30 minutes for a request's body, 4096 connections at once | `axum::serve` gives hyper no timer, and without one hyper enforces no header timeout: a client that opens connections and sends nothing would hold them for ever |
| bodies stream both ways, never buffered | uploads reach 2 GB (app releases) and the event streams (the local engine's) must reach the page frame by frame; the conformance suite checks the first frame arrives before the stream ends |
| the client's address is decided HERE, with `trust proxy` arithmetic (`K7_FRONT_TRUST_HOPS`) | the rate limits, the registry's same-network listing and the share player's per-address cap read one address |
| with no trusted hop, `X-Forwarded-Host` is dropped and `X-Forwarded-Proto` is `http` | a client must not choose the origin the front derives its redirect URIs from |
| a forwarded address that names loopback, from a socket that is not loopback, is replaced by the socket's own | a hop count trusts whatever stood one hop away: on a network shared with other containers, any of them could otherwise claim to be this machine |
| "this machine" — a registry engine's `loopback_seen`, so its use by the account and the relay dialling `127.0.0.1` — is a loopback client address, which only a loopback socket gives (above); a hint naming loopback is never dialled on its own | an engine on this machine is every signed-in account's; that must not be one header away. The socket alone would not do either: behind the dev session's Vite every socket is loopback, a LAN engine's included |
| the front draws its own seams' secret per process when none is given (`Config::settled`) | the internal seam and the engine slots need a secret no client knows |
| a debug build optimizes its dependencies (`opt-level = 2` for every non-workspace package), the desktop app's workspace too | the front runs the song compiler and the WASM modules in Wasmtime: at opt-level 0 a conformance run takes two to three times longer; the workspace's own crates stay unoptimized, so an edit still rebuilds in seconds |
| what the state keeps never holds the state: a door's routes are kept without it and given it per request, a middleware or hook kept there holds it weakly | a router given the state, or a closure holding it, kept inside that state keeps it alive for good: the families' services, sockets and child processes outlive a stopped front, and a test binary never exits; `src/stop_tests.rs` stops a front after each door answered and checks the state is let go |

## The roots it trusts

| choice | why |
|---|---|
| every outbound TLS call (Claude, the git listings) is built from one root store, `k7_front::trust` | a self-hosted server under the person's own CA is reached by all of them, or by none |
| Mozilla's roots (`webpki-roots`), plus the PEM file the host names in `K7_EXTRA_CA_CERTS` | a self-hosted server under the person's own CA is trusted by every outbound call; a file is a dependency nothing else is: `rustls-pki-types`' PEM reader is already rustls' |
| no OS store | the OS store needs `rustls-native-certs`, a crate not taken. The supervisor reads the OS store on a desktop for the engine's own calls (`apps/k7-daw-backend/packages/k7-supervisor`) |
| read once, at the front's start, before any client is built; an unreadable file is said and leaves Mozilla's roots | a trust store that moves under running clients would be two answers for one server |

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
| a kill from outside, or any of the above | the exit code, spelled (`0xc00000fd: a stack overflow`) | the host: Tilt's `dev-front.mjs`, the image's `start.sh`, the test beds |

| choice | why |
|---|---|
| the lines go where the host keeps the front's: stderr for the binary, the merged log under `[front]` for the desktop app, logcat under `k7-front` for Android, with every other line the front prints (`k7-embed`'s `logcat.rs`) | the desktop app and the phone run the front in their own process, whose stderr nobody reads |
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
| `K7_FRONT_SECRET` | drawn per process | the front's own seams' secret; the dev session and the beds name one so the conformance specs behind the internal seam can send it |
| `K7_ENGINE_CORE` | unset | `front` answers the engine registry, the relay, the tunnel hub and the seam's engine half here (`engine.md`) |
| `K7_LOCAL_ENGINE` | unset | `front` on a device runs the machine's supervisor here (`local.md`) |
| `K7_FRONTEND_DIST` | `apps/k7-daw-frontend/dist` above the front | the built page the last routes serve |
| `K7_DEV_FRONTEND` | unset | a running Vite the last routes stream the page from, as the dev desktop app asks |
| `K7_EXTRA_CA_CERTS` | unset | a PEM file of certificate authorities every outbound TLS call trusts beside Mozilla's roots, read once at start |
| `K7_FRONT_TRUST_HOPS` | `0` | proxies before the front whose `X-Forwarded-*` is believed: `1` behind Traefik or the dev Caddy |

The routes read their settings from `ENV_KEYS` in `src/config.rs`: `K7_SESSION_TOKEN`, `STUDIO_LOCAL_USER`, the prefs and auth folders…

The same fields, camelCased, are the JSON a host hands `k7_front_start` (`embed.md`), with an `env`
object for those settings: an embedded front cannot be handed an environment of its own.

## Where it runs

| host | the front |
|---|---|
| the dev session | the Tiltfile's `k7-front`, on 127.0.0.1:8789, one trusted hop (Vite) — `docs/dev.md` |
| the desktop app | linked into the desktop app, on 18789 — `apps/k7-daw-desktop/docs/desktop.md` |
| Android | `libk7_embed.so` in the app's process, on the studio's port — `apps/k7-daw-engine/k7-runtime/docs/android.md` |
| iOS | `k7-embed` linked into the app — `apps/k7-daw-engine/k7-runtime/docs/ios.md` |
