# The heartbeat — reading one without letting one odd field unlist an engine

An engine's supervisor POSTs `/registry/heartbeat` every 5 s; an entry that has not had one in
15 s (`REGISTRY_TTL_MS`) is gone. So a heartbeat REFUSED is an engine unlisted — and one that was
listed a moment ago stays in a browser's cached list, whose next `select` then fails.

## Only three fields can refuse one

`read_heartbeat` (`crates/k7-engine/src/registry/heartbeat.rs`) requires what there is no entry without: the **id** (at most 128 characters of
`[A-Za-z0-9._-]`, since it travels in paths and log lines), the **token** (at most 256), and
**`addresses.port`**, where the engine answers. `tunnelSecret` is optional, for a supervisor older
than it, but a malformed one refuses the heartbeat rather than being cleaned away: dropping it would
turn a bound engine into a legacy one (`token.md`). A heartbeat missing one is refused with a 400 whose
body names the field (`heartbeat refused — token: …`), and the supervisor writes that body into its
`.studio.log`.

Everything else is CLEANED, and each cleaning is said:

| field | when it is odd | what is kept |
|---|---|---|
| `hostname`, `machineFingerprint` | not a string | empty (a hostname is cut at 255) |
| `addresses.loopback` | not IPv4 | `127.0.0.1` |
| `addresses.lanIps` | an entry not IPv4 | the others |
| `performance` | any figure out of its bounds | no performance at all — the picker shows no rating |
| `libraryLevel` | anything but an integer from 1 | no level — read as 1, what a supervisor older than the field runs |
| `protocol` | anything but an integer from 1 | no version — read as 1, what a supervisor older than the field speaks (`packages/engine-protocol/docs/version.md`) |
| `build` | not 1 to 64 of `[A-Za-z0-9._-]` | no stamp |
| `safeMode` | not a boolean | not in safe mode |
| `chains` | not an object, or a name that is not a string | that chain left out; no chains, as an older supervisor sends |

The cleanings come back in the answer (`{ ok: true, cleaned: [...] }`) and go into this server's
log. The bounds hold — the route is open, and a rating is a hint for a picker, never a number to
trust blindly — but a figure out of them costs the rating, not the listing.

One strict schema would refuse a machine on every beat for a figure its first heartbeat did not
carry (performance is measured after the engine starts): the entry lapses, and a play answers
"unknown engine or bad token" with nothing in either log to say why.

## A heartbeat proves nothing

The heartbeat is open: a device's engine lists itself at its own backend, and a machine on the network may
list itself too. Neither proves anything, and a listed engine is not usable for that: only one whose heartbeats
come from loopback is the device's to use (`ownership.md`).

## What is logged here

`[registry] …` on the front's stderr, at most once a minute per engine and per complaint
(`crates/k7-engine/src/log.rs`), since a refused machine retries every 5 s:

- a heartbeat refused, and why;
- a heartbeat taken with fields cleaned, and which;
- a token mismatch (409) — another process registered this unbound id with another token; the id
  is freed 15 s after that one's last heartbeat;
- a secret mismatch (403) — the id is bound to another machine's tunnel secret (`token.md`);
- a tunnel refused for the same reasons;
- the list full (503, `MAX_ENGINES`), or too many ids NEW to it from one address in a minute (429,
  `NEW_IDS_PER_IP_PER_MINUTE`): the route is open, so what it may grow is bounded.
- a `select` refused.

## `select` tells its two refusals apart

`404 engine gone` when there is no live entry — the engine's machine has not had a heartbeat
accepted in 15 s, and its `.studio.log` says why. `403 bad token` when there is one and the token
the browser got from the engine's `whoami` is not the one it registered with. Two answers and not
one 403, because a single one sends the reader to the token when the cause is the heartbeat. A
third, `403 unowned`, is ownership's and not the heartbeat's: the engine does not run on this device
([ownership.md](ownership.md)).
