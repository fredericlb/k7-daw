# One registry, and it is the origin's

An engine heartbeats to **one** registry per studio family, answered by k7's front
(`crates/k7-engine/src/registry/`, routed by `crates/k7-front/src/routes/engine/`) on k7's origin — `engineRegistryUrl` in
`packages/web-protocol/services.ts`. `play.st0.app` is a former host read as that origin, so a supervisor
that saved `https://play.st0.app` reaches the same registry (`packages/web-protocol/docs/addresses.md`).
The editor and the other apps that drive an engine list their engines from it.

## The problem it solves

The registry answers one question: *which engines can this browser reach?* Every app that drives
an engine asks it — the editor for a song, a share's player for a visit — so
an app without one to consult could only hardcode `127.0.0.1:8765` (`probeLocalEngine`).

## Why one, and why the BROWSER asks it

With one registry per service, an engine would have to be pointed at each of them to be pickable in
each — and the heartbeat count would grow with every service. One registry makes "this engine is
connected to that studio" one fact.

The case against a central registry is that one backend would become a request-time dependency of
another. That coupling does not arise here: no BACKEND asks another. Every page that calls it is
k7's own and calls it same-origin, the relay included; a credentialed call from any other origin is refused (`routes.md`, the chain).

## Saying what it is

`GET /api/registry/hello` answers `{ service: "k7-registry" }`, open like the heartbeat. It is how the
supervisor tells a registry from any other website before it sends one the engine's token
(`engine/k7-supervisor/docs/supervisorPolicy.md`); it says nothing else.

## What this costs

The registry is down whenever k7's front is, and so is everything else: every page, the workspaces
included (`apps/k7/backend-rs/docs/files/hub.md`), goes through it, so the front down is the studio
down. A registry that is down is absent from every picker, which is right: the
engines cannot be listed.

The remembered engine is the ACCOUNT's: the apps read and write one
preferences document on k7's server (`engineId`, the command preferences, the playback
destination), so a machine picked in one is picked in the other.

An engine is still listed once and driven by several apps at once, through separate sessions
(`packages/engine-protocol/docs/sessions.md`).

## Which routes are open

The registry's routes are the front's, listed in one place (`crates/k7-front/src/routes/engine/mod.rs`).
The ones an ENGINE calls — `heartbeat`, `deregister`, `hello`, the tunnel — are open, since a machine
proves itself with its tunnel secret and holds no session; every one a BROWSER calls checks the session
first. One list draws that line, so it is one place to read rather than a rule every mount must keep: a
guard put over the whole of `/api` would leave the heartbeat protected only by whoever answered first.

The engines' secret bindings and owners outlive a restart in `STUDIO_ENGINES_DIR`, else
`<instance data folder>/.engines` (`token.md`, `ownership.md`); an admin (`AUTH_ADMIN_GROUPS`) may reset an
engine's owner.

## What `assigned` means, and why it is nearly obsolete

`assigned` is a lease: a tab re-POSTs `/engine/select` every few seconds, and the flag
expires on its own because a closing tab can only say goodbye best-effort. It hides an engine
already being driven, from a world where a second driver would destroy the first one's project.

Sessions remove that danger — two apps on one engine hold two projects. The flag survives as
**information** ("someone is on this"). Retiring it is its own decision.
