# Accepting a WebSocket — `acceptSocket` and `heartbeat`

Every WebSocket the TypeScript backend accepts — a share's visitor, a sync replica — completes its
upgrade through `acceptSocket` (`../upgrade.ts`). The front's sockets (a browser on the engine relay,
a supervisor's tunnel, a loopback client of a tunnel channel) keep the same two rules in Rust
(`apps/k7/backend-rs/crates/k7-engine/src/ws.rs`), where a peer's fault ends its socket and nothing
else.

## An error listener first, always

`ws` emits `error` on a socket for a protocol fault of the PEER: a frame with RSV1 set where no
extension was negotiated, an unmasked client frame, a frame over `maxPayload`, invalid UTF-8 in a
text frame. An `EventEmitter` with no `error` listener throws, the throw reaches
`uncaughtException`, and the backend exits — with every session, relay room and file service it
carries. Nothing about the fault needs an account: a refused share socket still reads frames
until its close completes.

So `acceptSocket` attaches the listener inside the upgrade's callback, before the caller's own
code runs, and the listener terminates the socket. A caller that wants to log the fault passes
`onError`; one that wants to clean up listens to `close`, which follows the termination. No
caller writes its own `handleUpgrade`: a path that forgets the listener is a remote crash.

## A peer that stops answering is dropped

A phone that leaves the network sends no FIN, and TCP can take hours to give up. A socket the
server keeps for such a peer holds whatever it pins: an engine session, a share's visitor slot, a
relay pair. `heartbeat(ws)` pings every 30 s and terminates a socket that left the previous ping
unanswered, so a dead peer is gone within a minute. The sync relay runs the same rule over its whole
server in one loop; a share's visitor, served alone, calls `heartbeat` on itself, and the front's
relay pings both its ends on the same 30 s (`HEARTBEAT_MS`, `apps/k7/backend-rs/crates/k7-engine/src/relay/mod.rs`).
