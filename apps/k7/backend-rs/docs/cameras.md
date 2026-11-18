# The remote cameras' hub

`src/routes/cameras/` in `k7-front`: the routes and the signalling socket behind the home page's
`camera mode` and the `remote:` camera nodes. What a remote camera is, and why the picture goes peer
to peer, is `packages/web-protocol/docs/cameras.md`. How the front serves them (the crate, the lock,
the vectors) is `routes.md`, "The remote cameras".

| | |
|---|---|
| `GET /api/cameras` | the account's cameras that are on: `[{ name, since }]` |
| `POST /api/cameras/ticket` | `{ ticket, iceServers }` — what opens the socket |
| `WS /api/cameras/ws?ticket=…&role=camera&name=…` | a device in camera mode |
| `WS /api/cameras/ws?ticket=…&role=viewer` | a browser drawing or recording one |

## A ticket, because an upgrade carries no body

A page cannot put its cookie's guard on an upgrade. So the session is checked on the ticket route,
which mints an HMAC binding the **account** for a minute, and the upgrade checks that — the engine
relay's pattern (`engine.md`). The secret is per process: a restart costs a socket one reconnect,
which the camera does anyway.

## In memory, by the socket's life

A camera is listed for exactly as long as its socket is open: nothing is written, and a restart
lists none until the devices reconnect. A list on disk would list cameras that are off. A socket
that misses a ping for thirty seconds is dropped, so a phone that lost its network leaves the list.

## What the hub decides

- **Which camera a viewer gets**: `matchRemoteCamera`'s rule over the account's cameras, never
  anyone else's. The sentence explaining a fallback travels on the camera's offer.
- **The unique name**: a second camera under a name already on gets ` (2)`.
- **Who may talk to whom**: a camera's frame reaches a viewer only if that viewer asked for that
  camera; a viewer's reaches only the camera it was matched to.
- **The caps**: eight cameras and thirty-two viewers per account, and 64 KiB per frame — signalling
  is SDP and candidates, and each viewer is a peer connection the camera encodes for.
- **A slow socket is dropped, never waited for**: each socket's outbox holds 256 frames, and one
  that fills is closed 1013; a write that takes more than ten seconds drops the socket. An
  unbounded outbox would let one account's viewer that stops reading grow the server's memory
  with every frame its camera sends.

`K7_CAMERA_ICE_SERVERS` names the STUN/TURN servers handed to both ends: comma-separated URLs, or a
JSON array of `RTCIceServer` when a TURN server needs credentials. Unset, there are none.
