# `engineAccess` — one object for reaching an engine

The pieces exist separately: `registry.ts` (the routes), `probe.ts` (measuring), `choose.ts`
(the policy), `lease.ts` (the lease). `access.ts` binds them to the registry's address, once.

## The registry's address is given once, to the constructor

`base` is where the registry answers — k7's backend, the one registry (`apps/k7-daw-backend/docs/engine/shared.md`):
`serviceUrl(…)`, which is k7's origin, where its preferences are too. It decides both the registry's routes
and what the probe composes.
Passed separately to two functions it is two places for one fact, and nothing would force an app
to keep them in step. The one that drifts most quietly is the probe's, and it is what decides
whether the **relay** is offered: a service can then list its engines correctly and reach none of
them remotely, with no type noticing.

```ts
export const engines = engineAccess<PublicEngine, K7Prefs>(ORIGIN, json, { hostedFiles: true });
```

## `find()` is the sequence, not the pieces

List, read the preference, measure, apply the policy. Written per surface that sequence
diverges: filtering on `wsUrl` alone offers an engine that answered without a token and cannot
then be selected, and raises a picker in front of a single reachable engine.

It answers `ranked` for the picker, `chosen` for starting, `ambiguous` for whether there is a
question to ask, and `rememberedId` for ticking the chosen row — including when that choice did
not answer, which a picker needs and a start does not.

`preferId` takes precedence over the stored preference: the same value, but already current when
somebody changes engine from a neighbouring header.

## Who is taken unasked

Registration is open — a machine registers with no account (`apps/k7-daw-backend/docs/engine/heartbeat.md`).
The list holds the engines this account may use (those of this device) and the
UNOWNED ones (`apps/k7-daw-backend/docs/engine/ownership.md`) — and an unowned engine is anybody's
machine until somebody opens it directly, with any performance score it likes to claim. Taking one without a click would bind the account's workspaces to a stranger's
tunnel. So `chooseEngine` and `bestEngine` only ever take an engine that passes `mayAutoPick`
(`choose.ts`), and offer every other one in the picker:

1. the engine the account **remembered** — an explicit pick, saved by the picker or the header's
   engine menu;
2. an engine reached **directly** (`reachedDirectly`, `probe.ts`): the probe's winning socket is
   one of the engine's own loopback or private addresses, dialled from this browser, and its
   `whoami` answered there with the id and a token. That engine is on the user's machine or the
   user's network. A public address, the relay does not count, since what answers there
   is whatever the stranger registered.

A first open from an HTTPS page on a machine with no local engine therefore lands on the picker
until the user clicks one, which remembers it. The server repeats the check on its side: `select`
refuses an engine that is another account's (`ClaimLost`, reason `refused`, which the lease does not
retry), and a route that reaches an engine for an account requires that account to hold a claim on
it (`apps/k7-daw-backend/docs/engine/claim.md`).

An unowned engine comes with no relay ticket, so only a direct probe can reach it, and the relay
strips the token from a `whoami` for an account that may not use the engine. A reply without a
token ranks below one with it and leaves `token` null, so no chooser takes it.

**A silent registry is not an error.** `find()` swallows both requests' failure and answers an
empty list — the honest state, and a screen, where a rejected promise inside a gate's effect is
an eternal spinner.

## A probe is a snapshot, so it is re-run

"Unreachable" is a state of its own — `probeEngines` answers `latency: null` when no candidate
replied in time — so without re-measuring one unlucky race marks an engine unreachable until the
page reloads.

`relook.ts` answers a counter for the dependencies of the effect calling `find()`. It advances
every 15 s, aligned on `REGISTRY_TTL_MS` — longer buys nothing, an engine that stopped beating
having left the list; shorter probes faster than the information can change — and when the tab
returns to the foreground, which is precisely when "unreachable" is read.

Where to wire it differs, and the difference is a choice:

| surface | when |
|---|---|
| k7 `EngineGate` | never: the bar's menu looks again for an engine only when it has none |

The probe's timeout is **3 s**. Candidates are measured in parallel, so an absent engine costs
one timeout rather than one per address; and on any machine but the engine's the only workable
route is the relay, which a loopback round-trip's budget would declare unreachable for being
merely longer.

## The probe offers only what can succeed

An HTTPS page does not open `ws://` to a LAN address. Listing them anyway works — the browser
discards them — but each discarded attempt counts as **mixed content**, and that is what makes a
page non-installable. The surfaces menu could then not be added to a home screen, which is the
device it exists for (`apps/k7-daw-frontend/docs/external.md`).

`candidatesFor` therefore offers loopback — potentially trustworthy by definition, dialable from
HTTPS — and the relay. LAN addresses stay listed over plain HTTP, where they work. There is no WebRTC
data channel: the browser reaches an engine on its own network or through the relay.

One more is dropped instead of raced, because it cannot answer: a loopback address that refused is left alone for two minutes: the
page's own machine is the one thing it can learn without asking the server, and the same question 15
seconds later has the same answer. It is offered again once that window closes — the price being that
a tray started meanwhile is found within the two minutes rather than immediately.

Every candidate is raced, but the timings decide only among equals: a **direct** socket that
answered with a token beats the relay however fast those measured (`orderHits`). The
direct path is the shorter one for the audio, and it is the only one `reachedDirectly` trusts, so a
relay that merely won the clock would turn the one engine on this machine into one the gate may not
take unasked. The clock is not worth trusting there anyway: each round trip ends when the page's main
thread reads the reply, and a page still mounting easily delays one answer more than the other.

## Hosted files are opt-in, per app

When the registry's server hosts the workspaces (`apps/k7-daw-backend/docs/files/hub.md`), an app
built with `hostedFiles: true` sends every file call there instead of to the engine's machine:
`fileBase` becomes the hub, and `fileToken` a ticket the hub minted from the session, with the
chosen engine's id after a `~`. The ticket rides in the header the file service's gate already
reads, so none of the calls that build that header change, and none of them need a cookie
across origins. `token` stays the engine's own, since the claim is checked against it.

`hostedFiles()` answers the same hub with no engine in the ticket, for a page that lists or edits
files and plays nothing — k7's home page, whose workspaces exist whether or not an engine runs.

k7 opts in: it opens a project, and a project is the server's. A host that sends the
engine a document of its own does not: the server's files would name paths the engine cannot open.

## Following a scan claims nothing

`watchPluginScan` (`scanWatch.ts`) holds one socket to one engine and hears its `plugin-scan`
frames: it asks once, then listens to what the engine broadcasts while a scan runs. It sends no
session and no claim, so the relay passes it as it passes a probe (`plugin-scan` is among the
probe's verbs and answers, `apps/k7-daw-backend/packages/k7-engine/src/relay/gate/`), and a page that drives nothing
— k7's home page — can say a scan is running without taking the engine from whoever plays on it.
Its URL is asked on every dial, since a relayed one carries a ticket that ages.

## Outside this object

- **`EngineClient`**: opening the socket with the service's session
  (`sessionFor(SESSIONS.k7, …)`), which each app holds and only one of them must —
  `@studio/engine-protocol/docs/sessions.md`.
- **The file service**: `fileBase` goes from the probe into `@studio/project-shell/fileApi`, and
  **travels with the address that answered**. A relayed socket beside a direct `http://` rebuilds
  the mixed-content wall on the other half (`apps/k7-daw-backend/docs/engine/proxy.md`).
- **`bestEngine`** (`choose.ts`) ranks the trusted candidates by compatibility with the project
  (`compat.ts`), then the engine the account last chose, then the one on THIS machine
  (`onThisMachine`), then the latency class and the performance score. k7 does not use it: its
  gate takes the engine on this machine and offers no choice (`apps/k7-daw-frontend/docs/shell.md`).
