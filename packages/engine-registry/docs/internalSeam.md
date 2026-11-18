# The internal seam

The registry, the tunnel hub, the machine's local engine and the file hub are the Rust front's
(`apps/k7/backend-rs/docs/engine.md`, `local.md`, `files.md`). Whatever needs them — the front's own
families, which call it at the front's own port so a test bed sees the calls a host makes, and the
TypeScript that `seamClient.ts` serves — asks a loopback HTTP API under `/api/internal`. This page is
that API's contract. The front serves the engine half
(`apps/k7/backend-rs/crates/k7-front/src/routes/engine/internal.rs`), the local half
(`apps/k7/backend-rs/crates/k7-front/src/routes/local/internal.rs`); the backend answers what is left
of the file half (`apps/k7/backend/src/media/internalFiles.ts`). `seam.ts` holds the paths, the shapes and
the gate.

## The choices

| choice | why |
|---|---|
| plain HTTP on loopback, JSON both ways | every caller is already `async`, and the front speaks HTTP/1.1 to the backend anyway; a second protocol buys nothing |
| a channel is the hub's loopback local path (`packages/engine-tunnel/docs/tunnel.md`), never a channel API over HTTP | the file services already dial engines that way; one protocol for every TypeScript consumer, which the hub implements once |
| a one-shot binding for a route's channel: bind, upgrade, unbind | the hub reads a binding at the upgrade only, so dropping it after leaves the socket working and no key to leak |
| the registry is read per request, never pushed | a lapsed claim is seen at the file service's next request (`releaseIfUnclaimed`), which is when it matters |
| a view per engine and account (`EngineView`), not the registry's records | the callers need five facts; the records hold the token and the claims' secrets, which no other process should read |
| the gate is a loopback peer AND the secret | loopback alone admits any local process, and behind the front any page's request arrives on loopback; the secret alone would admit a remote request that guessed it |
| the secret is `K7_FRONT_SECRET`, else drawn per process | both processes already share it; a backend with no front calls itself with a secret nobody else holds |
| the TypeScript calls the front's port when `K7_FRONT_SECRET` and `K7_FRONT_PORT` are both set, its own otherwise | behind a front every half but `verify-token` and the three `runs` answers is Rust's, and those are proxied back to the backend unchanged; a backend with no front answers its own engine half with a 404, which every caller reads as no engine there |
| the file half's gate mounted in the backend before the loopback lock and CORS, the engine and local halves outside the front's route chain | a server-to-server call has no `Origin`, and the seam's own gate is stricter than either |

**A kept binding outlives a restart of the hub's process.** The hub holds its bindings in memory,
and the front that holds it restarts on its own: in the dev session, or after a crash on any host.

| choice | why |
|---|---|
| the hub draws an `epoch` when it starts, answered by `GET /hub` and with every binding | one cheap read tells a client whether the hub it bound on is still the one answering, whatever it holds |
| the client re-establishes its bindings under the keys it was given, not new ones | the file services were handed those URLs, the render path in their environment at start: the same key needs no word to them and no restart, and their engine socket's redial finds it bound again |
| the file hub does it (`k7_hub::seam`), as the seam client does (`EngineSeam.rebind`) for a TypeScript caller, for every binding `bind` answered and `unbind` has not dropped | the file hub is the one caller that keeps bindings; a one-shot channel's binding is dropped at once and never re-established |
| a caller-chosen key only through the gated seam, and at least 32 characters of the drawn alphabet | only a process holding the secret on loopback can name one, and a key it re-establishes is one a hub drew; the length keeps a short guessable key out |
| a key already bound is bound again, not refused | re-establishing is then idempotent, and every caller holding the secret is the same trust |

## Authentication

Every request carries `x-k7-internal: <secret>`. In order:

| refusal | status | body |
|---|---|---|
| the peer is not loopback: `peerAddress(req)` from `@studio/web-protocol/services`, so behind the front the peer it vouched for, and `front-unverified` is not loopback | 403 | `{ "error": "the internal API answers this machine only" }` |
| the header is missing, or differs from the secret (compared in constant time) | 401 | `{ "error": "the internal API needs this process's secret" }` |

A passed request is answered with `Cache-Control: no-store`. Every error body is `{ "error": string }`;
its wording is not part of the contract, its status is. A body that is not JSON is a 400.

## The engine half: `/api/internal/engine`

Served by the front, which holds the registry and the hub, while its engine core is on
(`K7_ENGINE_CORE=front`, `apps/k7/backend-rs/docs/engine.md`); off, every path is a 404.

### `GET /engines/<id>?holder=<account>`

Engine `<id>` as the account sees it. `holder` is `claimHolder` of the caller's session (its email,
else its subject); absent or empty is no account.

`200`:

```json
{ "id": "e1", "hostname": "rig", "role": "owner", "holds": false, "tunnel": true, "renderer": false, "score": 700 }
```

| field | meaning |
|---|---|
| `role` | the engine's role for `holder`, the render machine not counted (`apps/k7/backend-rs/docs/engine/ownership.md`): `"owner"`, `"shared"`, `"unowned"` or `"none"`; never `"server"`, which `renderer` says instead |
| `holds` | the account holds a live claim on it, lapsed claims pruned first |
| `tunnel` | its tunnel is up now |
| `renderer` | its last tunnel presented the render key, up or down (`Hub::is_renderer`) |
| `score` | its machine's `performance.score`, `null` when absent |

`404` when no live engine has that id (no heartbeat within `REGISTRY_TTL_MS`).

### `GET /engines?holder=<account>`

`200` `{ "engines": EngineView[] }`: every live engine, as above, in the registry's order.

### `GET /hub`

`200` `{ "rendersEnabled": boolean, "renderer": string | null, "epoch": string }`: whether a render
key is configured (`STUDIO_RENDER_KEY`), the render machine's id while its tunnel is up, the most
recent one if several hold the key, and the hub's epoch: `[A-Za-z0-9_-]+`, the same for the life of
the process holding the hub, another after it restarts.

### `POST /bindings`

```json
{ "engineId": "e1", "roots": [{ "remote": "/srv/ws", "name": "ws-a", "writable": true, "perChild": false }], "kind": "jobs", "synced": true, "key": "<a key held before>" }
```

| field | |
|---|---|
| `engineId` | an engine id, or `null` for the render machine, resolved when a client CONNECTS |
| `roots` | at most 64 `TunnelRoot`s: `remote` and `name` non-empty strings, `writable` boolean, `perChild` optional boolean |
| `kind` | `"engine"` or `"jobs"` |
| `synced` | optional, default false; ignored for the render machine |
| `key` | optional: bind under this key, matching `[A-Za-z0-9_-]{32,128}`, rather than a drawn one; a key already bound is bound again |

`201` `{ "path": "/api/engine-tunnel/local/<key>", "epoch": string }`, `<key>` matching `[A-Za-z0-9_-]+`
(32 characters when drawn), `epoch` as `GET /hub` answers it. `400` for any other shape. The binding is not checked against the registry: the caller asked
`holds` or `role` first.

The path is then dialled as a WebSocket at the seam's own address (`ws://127.0.0.1:<port><path>`),
and the hub answers it as every bound path (`Hub::check_local`): `403` for a peer that is not loopback or a
key not bound, `503` when the engine (or the render machine) has no tunnel, else `101` and the
frames piped both ways. A `jobs` binding's text frames are respelt to the tunnel's spelling on the
way to the machine (`packages/engine-tunnel/docs/jobs.md`). What the client sends before the
engine's channel opens is held, up to 256 frames and 8 MiB (`HeldFrames`). A dead tunnel closes the
client.

### `DELETE /bindings/<key>`

`204`, whether or not `<key>` was bound. `400` for a key outside `[A-Za-z0-9_-]+`. A socket already
upgraded through the binding keeps working.

### `GET /local`

`200` `{ "answered": boolean }`: whether the machine's supervisor has sent its first snapshot since
it started. `404` when the front runs no local engine (anything but the tray): the request reaches
the backend, which answers none of this half.

## The file half: `/api/internal/files`

Served by the front, which holds the file hub, and called by its registry; `verify-token` and `hub` are the backend's, which
the front proxies with `x-k7-internal` passed through until `K7_LAST_ROUTES=front`
hands those two to the front as well (`apps/k7/backend-rs/docs/last.md`): `verify-token` from its own ephemeral
bearers and the token file, `hub` as below.

### `POST /verify-token`

`{ "token": "<bearer>" }` → `200` `{ "sub": string | null }`: the subject the bearer verifies
as under the `/api` guard's own check, listed and ephemeral tokens alike, `null` when it does not.
`400` for a missing or empty token. The token is in the body, never the query string, so no access log keeps it.

### `GET /hub`

`200` `{ "runs": false }`, always: the backend runs no file hub, the front does (`apps/k7/backend-rs/docs/files.md`).
A `404` is a previous release's backend, which runs its own; the front then leaves it the family.

### `POST /cold-run`, `POST /slot`

`{ "user", "chain", "want" }` → `200` `{ "model", "outputs", "roots" }`, and `{ "user", "ref" }` →
`200` `{ "slot": { "id", … }, "roots" }`: a cut's chain and a slot compiled by the account's runtime on a device, in the libraries' context on a server
(`apps/k7/backend-rs/crates/k7-front/src/routes/files/hub/answers.rs`, `apps/k7/backend-rs/docs/files/hub.md`); an empty `chain` is the engine's own `system stems`. A refusal of the file hub's is `422` `{ "error" }` with its
words, which the caller passes on; `400` for another shape.

## Who calls what

| caller | calls |
|---|---|
| the front's `/api/engine-slots`, without the engine core in the front (`apps/k7/backend-rs/docs/engine/reach.md`) | `GET /engines`, `GET /engines/<id>` (`role` must be `owner` for a slot), then a one-shot `jobs` binding |
| the front's file hub binding an account (`k7_hub::binding`, `apps/k7/backend-rs/docs/files/hub.md`) | `GET /engines/<id>` per forwarded request of a bound account, a `synced` `engine` and a `jobs` binding kept while the account is bound and served, `DELETE` when let go, and a one-shot `engine` binding for the plugin list |
| the front's file hub, for each account's render target (`k7_hub::seam`) | `GET /hub`, then a `jobs` binding to the render machine kept per account |
| the seam client, while it holds a kept binding (`seamClient.ts`) | `GET /hub` every 2 s, and `POST /bindings` with its `key` for each one bound on another epoch |

The tests: `seam.test.ts` (the gate), `seamCore.test.ts` (every engine endpoint, and a kept binding
re-established on another hub, against the front's testbed), `internalFiles.test.ts` (the backend's part of
the file half), `apps/k7/backend-rs/crates/k7-front/tests/files_hub.rs` (the front's),
and each caller's suite, which reaches the seam the testbed serves.
