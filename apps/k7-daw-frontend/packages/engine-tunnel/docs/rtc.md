# The WebRTC path — a browser straight to an engine on another machine

An HTTPS page cannot open `ws://` to a LAN address (`apps/k7-daw-backend/docs/engine/proxy.md`), so
a phone on the stage, or a laptop across the room, reaches the engine through the server's relay:
every play press and every meter frame goes out to the server and back. A WebRTC data channel is
the one direct path such a page is allowed, and it is **offered beside the relay, never instead
of it**: the probe races both and keeps the faster (`apps/k7-daw-frontend/packages/engine-browser/docs/access.md`).

## Who answers the offer

The agent at the engine's machine, over the tunnel it already holds: the browser POSTs its offer to
`/api/engine/<id>/rtc` on the registry, the hub hands it to the agent, and an agent that answers
pipes the channel to the local engine exactly as the tunnel pipes a channel. The engine never
learns WebRTC exists. An engine is offered this path only when its supervisor holds a tunnel
(`PublicEngine.rtc`), since that is what carries the offer.

The offer is answered only for an account that **holds a claim** on that engine
(`apps/k7-daw-backend/docs/engine/claim.md`); anyone else gets a 403. The channel reaches the engine as
a socket of its own machine, past the relay's gate (`apps/k7-daw-backend/docs/engine/proxy.md`), so the
gate is here, once, before it opens. A picker's first probe, made before its claim, therefore
measures the relay alone; the probes after the claim race WebRTC too.

Signalling is **one request and one answer**, candidates included — no trickle. It costs up to
1.5 s of gathering on the first connect, and it spares the server a signalling session to keep.

## Who answers it today

The machine's supervisor (`apps/k7-daw-backend/packages/k7-supervisor`) answers an offer with an `rtc-answer`
carrying an error, and the browser stays on the tunnel: only a remote server's page needs this path, and no
page is hosted. The TypeScript reference agent answers with node-datachannel, an optional dependency loaded on
first use, so the server's tests run where its binary is missing and skip only the WebRTC case; `rtc.test.ts`
skips an agent that says `rtc: false` (`testAgents.ts`).

## The local network only, by default

Neither end is given a STUN or TURN server, so the candidates are the machines' own addresses: the
path works on the engine's network, which is where a second device usually is, and falls back to
the relay elsewhere.

## Frames are fragmented

A browser refuses a data-channel message past its size limit, and a whole project document passes
it. Every frame is therefore cut into 16 KB fragments carrying one byte of kind (text or binary,
more or last) — `@studio/web-protocol/rtc-frames`, the one codec both ends import. The channel is
ordered and reliable, as the engine's protocol assumes a WebSocket is.
