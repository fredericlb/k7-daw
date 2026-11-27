# The front's sockets — a peer's fault, and a peer that stops answering

Every WebSocket the front accepts itself — a browser on the engine relay, a supervisor's tunnel, a
loopback client of a tunnel channel — is read and written by `apps/k7-daw-backend/packages/k7-engine/src/ws.rs`.

## Choices

| choice | why |
|---|---|
| a peer's protocol fault ends that peer's socket and nothing else | a frame with RSV1 set where no extension was negotiated, an unmasked client frame, a frame over the cap or invalid UTF-8 in a text frame need no account to send: a tunnel is held without one, and a fault that reached past its socket would be a remote crash of every session and relay the front carries |
| both ends of the relay and the tunnel are pinged (`HEARTBEAT_MS` in `relay/mod.rs`, `ping_ms` in the hub), and a peer that left a ping unanswered is dropped | a phone that leaves the network sends no FIN and TCP can take hours to give up; what its socket pins (an engine session, a relay pair, a tunnel's channels) is held for as long |

`apps/k7-daw-backend/packages/k7-front/tests/tunnel/hub_protocol.rs` holds both: an RSV1 frame ends
the tunnel it came on, a peer that never answers a ping is dropped, and one that answers is kept.
