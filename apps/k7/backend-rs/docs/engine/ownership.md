# Ownership — whose engine it is, and who else may use it

Registration is open unless the studio asks a heartbeat to prove the render machine (`heartbeat.md`). A claim
says which project an engine drives for whom (`claim.md`), and the tunnel secret says which
machine an id is (`token.md`). Neither says whose engine it is. Ownership does.

## The rule

An engine belongs to the account its **record** names. Other accounts may use it only if the
owner **shared** it with them, by email or with **everyone** on this server. Two engines need no
owner at all: the server's own — `server`, its tunnel proved by the render key
(`packages/engine-tunnel/docs/jobs.md`) — and an engine on this backend's own machine.

## The server's engine is its admins'

The server's own engine has no record, yet it is the
server's: an **admin** — an account in `AUTH_ADMIN_GROUPS` — is its `owner`, and every other account
lists it as `server`. Both may use it, so the relay and the claims do not tell them apart; what
differs is what the Engines card says. The listing's `server` flag stays on for an admin too, since
it is what lets a browser take the engine unasked, and it is about the machine, not the account.
Its owners share nothing and give nothing up: every account already uses it, and the admins are
whoever `AUTH_ADMIN_GROUPS` names, which no route here changes.

| role (`PublicEngine.access`) | listed | relay ticket, token through the relay | `select` |
|---|---|---|---|
| `owner` | yes | yes | yes |
| `shared` | yes, with `ownedBy` | yes | yes |
| `server` (`owner` to an admin) | yes | yes | yes |
| `unowned` | yes, LAN addresses only to its own network (`sameSourceIp`) | no | **403** `unowned` |
| anyone else | **no** — no name, no address | no | **403** `not-yours` |

Between accounts that may use an engine, the claim rules are unchanged: the newcomer still displaces
(`claim.md`). An anonymous session — no email, no subject — can neither own nor claim.

The account is spelled as `claimHolder` spells it (the session's email, else its subject), so the
owner check and every claim check compare one name. Shares are emails, compared lowercase.

## What assigns an owner

No heartbeat does: one that proves nothing, or proves the render machine, changes no record. An open makes no
owner either: holding the engine's token proves only that a browser reached the machine — any account signed in
on any browser there, which is how a stub dev account or a guest's session would end up owning a musician's
engine. An engine with no record stays unowned, usable only by the two rules that need no owner; the records
already on disk keep deciding `owner` and `shared`, and the routes below edit them.

## An engine on this backend's own machine is every account's on it

An engine whose heartbeat came from loopback runs on the machine this backend runs on: the tray's or
the phone's own engine, a developer's in a dev session, a server's next to it. Several accounts sign
in there — an app's local one before any other, then the person's real one, or the functional
suite's — and the machine is theirs alike. So such an engine is `owner` to every signed-in account
(`Registry::role` in `crates/k7-engine/src/registry/mod.rs`, the one rule the listing, a select and the relay apply), whatever its
record says, and its record is left as it is for every other backend it beats to.

"Came from loopback" is any heartbeat in the last 15 s (`on_this_machine`, in the same file), not the
last one: a supervisor may list the same backend twice, once on `localhost` and once through a
reverse proxy whose address is all the second heartbeat shows, and the engine would flip between
everyone's and its owner's every few seconds.

## Sharing and giving up

Behind the session, under `/api/engines` (`crates/k7-front/src/routes/engine/ownership.rs`):

| route | who | does |
|---|---|---|
| `GET /engines/:id/shares` | the owner, an admin | `{ owner, shared, everyone, since }` |
| `PUT /engines/:id/shares` `{ emails, everyone? }` | the owner | replaces the list (at most `MAX_SHARED`), and the flag when given; a removed account's claims end now |
| `DELETE /engines/:id/owner` | the owner, an admin | the record goes, and no heartbeat assigns it again |
| `GET /engines/owners` | an admin | every record, with its machine's `hostname` and whether it is `online` |

Anyone else gets `404 not-yours` from the first three — the same answer as for an engine nobody
owns, so a stranger cannot probe which ids are owned. An admin is an account in one of the groups
`AUTH_ADMIN_GROUPS` names, as for every admin route of the front.

**Everyone** is a flag beside the list rather than a list of every account, so an account created
after the share is included without the owner editing anything. Absent from a `PUT`, it stays as
it was: a client that only edits the emails does not withdraw it.

In k7 the Engines card of `/settings` (`apps/k7/frontend/src/settings/EnginesSettings.tsx`) badges each
engine with its role and gives the owner the share list and the way to give it up; the picker
(`apps/k7/frontend/src/components/EngineSelect.tsx`) shows the same badge.

## What inherits it

Every server route that reaches an engine for an account asks for a claim (`claim.md`, "The claim
is what lets the server reach an engine"), and only an account that may use the engine gets one. So
the file hub's binding, a stem cut, a song brought down for the player, a WebRTC offer and the
relay's driving frames are all ownership-gated through `select`. A claim taken before a share was
withdrawn, or before the engine moved, is evicted with it.

A browser that loses a claim this way is told `ClaimLost` with reason `refused`
(`packages/engine-browser/src/registry.ts`), and the lease stops for good rather than retrying.

## Where it is kept

`engine-owners.json` beside `engine-bindings.json` in `STUDIO_ENGINES_DIR`, else
`<instance data folder>/.engines`: written whole and atomically, 0600, set aside when unreadable
(`crates/k7-engine/src/registry/state_file.rs`). Capped at `MAX_OWNED` ids, the oldest record going
first. Its bytes are the ones the previous release reads, so a rollback keeps every owner.

A record with no heartbeat behind it is kept: dropping it would leave an engine nobody can use through the
relay.
