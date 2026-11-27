# Backpressure — a peer that stops reading

Every relay forwards frames between two sockets: the engine relay and a loopback client with its
tunnel channel, in the front's hub (`apps/k7-daw-backend/packages/k7-engine/src/relay/`,
`apps/k7-daw-backend/packages/k7-engine/src/tunnel/local.rs`). A socket never refuses a send: when the far end stops reading, the bytes
queue in the server's memory, without bound. A tab left in the background, a phone on a dying
network, a stuck supervisor would each grow the server until it fell over.

So every forwarded frame goes through one rule — `admit`
(`apps/k7-daw-backend/packages/k7-engine/src/tunnel/backpressure.rs`) in the hub — which reads the
peer's unsent bytes (`bufferedAmount`) and decides:

- below **8 MiB**, it is sent;
- past it, a frame a newer one **replaces** is dropped: an audio broadcast frame (the `LSTR`
  binary of `apps/k7-daw-frontend/packages/engine-protocol/stream.ts`), and the text pushes `state`, `io-levels` and
  `plugin-params-live` (`apps/k7-daw-backend/packages/k7-runtime-server/docs/protocol.md`). The engine already skips a state
  frame for a socket behind; this is the same rule one hop further. The frame is classified only
  then, so a healthy socket pays no parse;
- anything else — a reply, a document, a command — is still queued, since dropping it would
  leave a caller waiting for ever; past **64 MiB** of those the peer is closed, with **1013**
  ("try again later") where it is a WebSocket. A reconnect is cheaper than a server out of memory.

8 MiB is several seconds of the stereo float broadcast, so a short stall loses nothing; 64 MiB is
far above any single reply, and a peer that far behind is not coming back.

## On a tunnel, a channel is counted apart

One tunnel carries every channel of its engine over one socket, so the socket's own
`bufferedAmount` cannot say WHICH channel backs it up. The hub (`tunnel/hub_tunnel.rs`) counts each channel's bytes
from the moment it hands them to the socket until the socket's write callback says they left, and
that count is the channel's `bufferedAmount`. A channel past the hard limit is closed **alone**;
the tunnel and its other channels stay up. File chunks (a `get`) keep their own rule, the 1 MB
`wait_below` of the agent's link (`apps/k7-daw-backend/packages/k7-supervisor/src/tunnel/link.rs`), since they are no channel's.

## What waits for a connection is bounded too

Frames a client sends before its peer is connected — the relay's dial, a channel not yet `opened`,
the engine socket of a data channel — are held in a `HeldFrames`: 256 frames or 8 MiB, the same
bounds as the relay's claim gate (`apps/k7-daw-backend/packages/k7-engine/src/relay/gate/`), and the
client is closed past them.
