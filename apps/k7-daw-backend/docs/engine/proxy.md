# The relay — reaching an engine the browser may not call

The engine listens in the clear, on `ws://<machine>:8765`. Studio pages are served over HTTPS.
Those two sentences do not hold together, and that is the whole subject of this page.

## The wall

An HTTPS page cannot open a `ws://` or a `fetch("http://…")`: the browser refuses it as **mixed
content**, before any network request. The one exception is loopback, deemed safe. The exact
consequence:

| from | `ws://127.0.0.1:8765` | `ws://192.168.x.y:8765` |
|---|---|---|
| the engine's machine | allowed, and it is the engine | allowed but useless |
| another machine on the LAN | allowed, and it is **not** the engine | **blocked** |

Hence a symptom that sends you looking in the wrong place: from a remote machine the only address
that produces a real network attempt is loopback — which hits the remote machine itself. The LAN
address being blocked appears in no Network tab, only in the console. So one reads "the client
insists on localhost" where one should read "the LAN address was refused before it left".

This wall is **independent of the network**. Everything else can be green — engine listening on
`0.0.0.0:8765`, Windows firewall rules on `Allow` for the exact binary, `nc -vz <ip> 8765`
succeeding from the remote machine — and the browser still refuses. Checking the network proves
nothing here: read the console.

macOS adds a second wall at the same place: since macOS 15 a browser needs "Local network"
permission to reach a private address, and a `nc` succeeding from Terminal says nothing about the
browser, each application holding its own.

## The way out

The browser composes no LAN address at all. It opens a `wss://` towards **the registry's own backend** — k7's front,
the page's own origin — and the front composes towards the engine (`packages/k7-engine/src/relay/`):

```
browser ──wss://app.priv.k7.rocks/api/engine/<id>/ws──▶ k7's front ──tunnel, or ws://<address>:8765──▶ engine
```

The front is not a secure context: it calls whom it likes, which is why it calls only one place
(`hosts_for`, `packages/k7-engine/src/registry/mod.rs`): the address the engine's **heartbeat came from** — loopback when the engine shares the
backend's machine, its LAN address when it shares its network. Never an address the heartbeat
NAMES: the heartbeat is open, and dialling what it names would let anyone aim this server at any
host it can reach. An engine whose heartbeat arrives over IPv6 or through a NAT is reached through
its tunnel alone.

No mixed content, no local network permission, no port to open in a firewall.

**The engine's own tunnel comes first.** An engine whose supervisor dialled in
(`apps/k7-daw-frontend/packages/engine-tunnel/docs/tunnel.md`) is reached through that socket, whatever network it is
on; the addresses above are the fallback, and they only answer when the server shares the
engine's network.

## Sockets only

The relay is an upgrade the front accepts outside its route chain, like every socket of the core
(`packages/k7-front/src/routes/engine/upgrade.rs`), and its credential is a **ticket**, not the
cookie: a page of another origin dials it too (below). There is no file half: files are the server's own
(`apps/k7-daw-backend/docs/files/hub.md`), so nothing plain-HTTP crosses to the engine's machine here.

The upgrade is checked before any session, for anyone who sends one: an engine id that is not
valid percent-encoding is a 400, and nothing read off the request or either socket reaches a panic
(`apps/k7-daw-frontend/packages/engine-tunnel/docs/tunnel.md`, "Nothing on a tunnel is trusted for shape").

The front is the one process that holds a registry (see [shared.md](shared.md)); every page of the
origin relays through it.

## The ticket

`/engines` mints one ticket per engine at the same time as it returns the list: that route is
already the authenticated one, and already returns every engine. A ticket asked for per engine
would have cost a round trip per engine, before a single one had been measured.

It lives a minute, HMAC-signed by a secret drawn at the startup of the **process that verifies
it** (`packages/k7-engine/src/registry/ticket.rs`): nothing to share between machines, no secret to wire into each backend. A restart
invalidates those in flight, which costs the browser a reload of a list it polls anyway.

The engine's `id` is **in the path**, and the ticket is verified against it: a ticket cannot be
replayed against another engine. The ticket also carries the **account** it was minted for, so the
relay knows whose claim to ask about. And the target always comes from the **registry**, never from
the request — a `?host=&port=` would make every backend an open relay towards everything it can
reach.

**The ticket is the upgrade's only credential, whatever page holds it.** A page with no cookie here dials
this relay with the ticket a listing carried; the ticket names the account that asked, the claim that account
takes is what the gate below asks for, and an engine that account may not use was never listed to it, so it
got no ticket.

## Before a claim, the probe and nothing else

The engine trusts the relay's socket as it trusts its own machine (`token.md`): whatever crosses it
can load a document, drop a session, approve a pairing. So a ticket is not enough to drive.
The gate (`packages/k7-engine/src/relay/gate/`) asks whether the ticket's account holds a live claim on the engine (`claim.md`) on
every frame the browser sends, and on the engine's frames at most every 250 ms (`CLAIM_REUSE_MS`):
the engine streams audio far faster than a claim changes, and asking for each frame is a registry
walk per audio block. A claim found missing is asked again on the next frame.

| | without a claim | with one |
|---|---|---|
| browser → engine | `whoami`, `pair`, `list-plugins` — what `probe.ts` and `compat.ts` send to pick | everything |
| engine → browser | `whoami`, `plugins`, `error` | everything |

Either way a `whoami` reaches the browser without its `token` unless the ticket's account may use the
engine — an engine of this device ([ownership.md](ownership.md)). `/engines` mints no ticket for any other engine, so this strip is the
second lock; it is asked for every text frame (a binary one never carries the token), so an engine
that stops being local takes effect at once.

Anything else the browser sends is **held**, in order, rather than dropped: a page opens its driving
socket and claims the engine at the same moment, and the claim may land a few hundred milliseconds
after the socket's first `hello`. Held frames go through the moment the claim exists; after 10 s
without one (`CLAIM_GRACE_MS`), or past a bound on what is held, the socket closes with 4403. A
claim taken over by another account stops the loser at its next frame, without a reconnect; what
the engine sends the loser stops within `CLAIM_REUSE_MS`.

What this gate does not cover: a browser that reaches the engine DIRECTLY — loopback on its own
machine, or a LAN address once paired — talks to the engine with nobody in between; the engine's
own origin check and pairing are what guard that path (`apps/k7-daw-engine/k7-runtime/docs/pairing.md`).

## What the probe does with it

`candidatesFor` (`apps/k7-daw-frontend/packages/engine-browser/src/probe.ts`) builds loopback, each LAN address, then
the relay. The order is documentary only: `rank` launches them **all in parallel** and keeps the
fastest. The relay pays one hop more, so it loses on its own wherever the direct link is allowed.

## The first message is queued

The browser sends `whoami` the instant the socket opens — and at that instant the *upstream*
socket is still being dialled. The relay holds what arrives before that and replays it once
upstream is open. Losing it would time out every probe: the relay would be "reachable" and answer
nothing, which is worse than unreachable.

That queue is bounded like the claim gate's (256 frames, 8 MiB — `HeldFrames` in
`packages/k7-engine/src/tunnel/backpressure.rs`): a client that sends more before the engine answers is
closed with 1013. Once both ends are up, every frame goes through the same file's `admit`, so
a browser that stops reading costs the server at most a bounded backlog
(`apps/k7-daw-frontend/packages/engine-tunnel/docs/backpressure.md`).

## Both ends are pinged

A phone that drops off the network sends no FIN, and the relay pair, the engine session behind it
and the gate's timer would live until TCP gives up, hours later. The browser's socket, and the
upstream when it is a socket the server dialled, are pinged every 30 s (`HEARTBEAT_MS`,
`apps/k7-daw-frontend/packages/engine-tunnel/docs/upgrade.md`): one unanswered ping and the socket is terminated, which
closes its pair. A tunnel channel
needs none of its own: the tunnel is pinged by the hub, and its channels end with it.

## What the lifecycle tests pin

`ticket.test.ts` and `relaySocket.test.ts` (`packages/engine-registry/`, against the testbed,
`../engine.md`) hold the relay to what a browser and an engine can observe:

- **The ticket** is good for one engine (another id's upgrade is a 401), for its minute inclusive
  and not a millisecond after, any number of times within it — nothing spends it —, and only in
  the process that minted it. It is a bearer credential: the account it names is the account that
  drives, whatever cookie or header the upgrade also carries.
- **Either end closing closes the other**, a close frame or a dropped socket alike.
- **A socket stays on the engine it was dialled to.** An engine that re-registers at another
  address is reached there by the next socket; an open one is not moved.
- **An engine whose heartbeat lapsed** is a 404 to a new socket, and an open socket's driving
  frames stop reaching it, since no claim on an unlisted engine holds.
- **A frame past `MAX_FRAME_BYTES`, from either end,** ends both sockets and is never forwarded:
  the browser's (1009, or 1006 when the teardown outruns the close frame), and the engine's, since
  the socket `dial_direct` opens has the same cap.
- **Order is kept** through both waits: what a browser sends before the dial and before its claim
  reaches the engine exactly in the order sent, the probe frames included.
