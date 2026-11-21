# Surfaces on the local network

What lets one device open the surface of a project another device has open: the door the main device opens,
the relay on the device that wants the surface, the discovery between them and the presence of a remote screen.
The page that draws it is `apps/k7-daw-frontend/docs/external.md`; the engine's own trust is
`apps/k7-daw-engine/k7-runtime/docs/pairing.md`. The crates are `k7-lan` (discovery) and `k7-front`'s `routes/lan`.

## Choices

| choice | why |
|---|---|
| one switch, **Available on the local network** (`lanOpen`), opens the engine, the door and the advertisement | one switch means "this device is on the LAN"; a second one would let a device be drivable and unlisted, or listed and refused |
| the door is a second listener, bound only while the switch is on and unbound when it goes off (`K7_LAN_LISTEN`, `0.0.0.0:18790`) | the front's loopback listener and its guard stay exactly as they are; nothing answers the network while the switch is off |
| every request to the door carries `X-K7-Engine`, the engine's token, compared in constant time; private and loopback peers only | the token is given only to a peer the engine trusted, so the door adds no secret of its own and no pairing of its own |
| the page talks to loopback only; the front relays HTTP and the engine socket to the peer, adding the token itself | Android allows cleartext to `127.0.0.1` only, and the token never reaches the page's script |
| the relay's engine socket carries no `Origin` and arrives from this device's LAN address | the engine's own rule (a native client, a private peer) decides, so the page's trust is never assumed |
| the door's file mount is a deny list over the file service, confined to the served project's workspace | a surface reads and writes through the routes the editor uses; what opens, activates, creates, deletes, syncs or configures is refused, and `GET /workspace` too, since answering it loads the project into the engine |
| the remote claims nothing and sends nothing to the engine | the main device already holds the project; a second open would run the open sequence there and could interrupt what plays |
| a device serves one project, the one the engine holds; the bench is never listed | an engine holds one song per session |
| discovery is `_k7._tcp` over `mdns-sd` on desktop and Android, the host's own browser on iOS, a fixed `K7_LAN_PEERS` list in beds and tests | `mdns-sd` is pure Rust and both advertises and browses; raw multicast on iOS needs an entitlement Apple grants apart, so Network.framework feeds the same trait; the daemon starts on first use, so a device nobody browses from opens no multicast socket |
| a peer is listed whether or not it answers, and `whoami` gives its id and its token | the page says why a device cannot be used rather than hiding it |
| compatibility is the door's `api` and the engine's protocol, both stated in `hello` | the page code of one device drives the other's file service and engine, so version skew is the risk; `API` is raised whenever a file route a surface uses changes shape |
| presence is a list in the door, forgotten after 15 s without a beat | a stale "in use" locks the main out of a pane, so the TTL is short, not the claim's 90 s; a screen of another project is dropped |
| the main device reads the same list on its own loopback, and a remote may take a pane the main is on | the remote is the screen the person just walked to; the main moves away instead of refusing it |
| a phone's host tells the front the switch (`k7_lan_set_open`), since the supervisor is its own | the front cannot read a supervisor it does not run |

## The routes

| route | where | what |
|---|---|---|
| `GET /lan/hello` | the door | the device, its served project with the surfaces it declares, the screens other devices show |
| `PUT /lan/screens/:screen`, `POST /lan/screens/:screen/bye` | the door | presence |
| `/lan/files/*` | the door | the confined file service |
| `GET /api/lan/peers`, `GET /api/lan/self` | this device's front | what discovery heard, resolved; this device's name and engine id |
| `GET /api/lan/screens` | this device's front | the screens other devices show of this device's project |
| `/api/lan/:id/{hello,screens/*,files/*}` | this device's front | the relay to the peer's door |
| `GET (ws) /api/lan/:id/engine` | this device's front | the relay to the peer's engine |

The relay sits under `/api` so the page's dev server proxies it and the claim of the front's routes treats it as one family:
all here on a device with the engine core, all declined elsewhere.

## Testing

`K7_LAN_PEERS=host:door:engine,…` stands in for discovery (`none` for no peer), `K7_LAN_OPEN=1` for the switch and
`K7_LAN_LISTEN` names the door's address, so a test binds it on `127.0.0.1`. The `mdns-sd` round trip is an `#[ignore]` test,
since it needs multicast on the machine.
