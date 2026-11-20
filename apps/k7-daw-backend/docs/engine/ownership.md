# Ownership — which engines this device's account may use

A device holds one local account, so an engine has no owner record, no share list and no admin that
could reset either. What remains is one question: does the engine run on this machine?

## The rule

An engine **on this backend's own machine** is the account's to use. Every other listed engine is
`unowned`: it is listed, its LAN addresses go only to the network it sits on (`sameSourceIp`), and a
`select` of it is refused `403 unowned`.

| `PublicEngine.access` | listed | relay ticket, token through the relay | `select` |
|---|---|---|---|
| `owner` — runs on this machine | yes | yes | yes |
| `unowned` — any other engine | yes, LAN addresses only to its own network | no | **403** `unowned` |

The wire keeps the name `owner` for the first row: the engine cards read it as "yours".
An anonymous session — no email, no subject — can neither use an engine nor claim one.

## What decides "on this machine"

The engine's heartbeats came from loopback: `Registry::may_use` and `on_this_machine`
(`packages/k7-engine/src/registry/mod.rs`) are the one rule the listing, a select and the relay apply.
"Came from loopback" is any heartbeat in the last 15 s, not the last one: a supervisor may list the same
backend twice, once on `localhost` and once through a reverse proxy whose address is all the second
heartbeat shows, and the engine would otherwise flip between usable and not every few seconds. A heartbeat
that only claims loopback in its body proves nothing; the socket does.

## What inherits it

Every route that reaches an engine for the account asks for a claim (`claim.md`), and only an engine the
account may use is claimed. So the file hub's binding, a stem cut, a song brought down for the player, a
WebRTC offer and the relay's driving frames are all gated through `select`. The relay asks again on every
text frame that could carry the token, so an engine that stops being local stops it at once.

A browser that is refused this way is told `ClaimLost` with reason `refused`
(`apps/k7-daw-frontend/packages/engine-browser/src/registry.ts`), and the lease stops for good rather than retrying.
