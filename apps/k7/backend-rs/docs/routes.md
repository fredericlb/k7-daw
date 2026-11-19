# The routes the front owns

A route moves to Rust when the conformance suite (`e2e/docs/conformance.md`) passes against the
TypeScript backend and through the front, and, for a route that writes, when each implementation
reads what the other wrote (`CONFORMANCE_UPSTREAM`).

| route | owned since | what still goes to the backend |
|---|---|---|
| `GET /api/health` | phase 1 | nothing |
| `GET`, `PUT /api/preferences` | phase 1 | the root domain's hosts; any front that cannot find the session revocations |
| `PUT /api/app-releases[/tray]/<stamp>/<file>`, `POST …/<stamp>/publish`, `GET /app_downloads[/tray]/…` | phase 1 | every other method on the upload paths, which the backend's bearer check answers; the whole family on a device (`STUDIO_LOCAL_USER`), where the backend does not mount it |
| `GET /api/cameras`, `POST /api/cameras/ticket`, `WS /api/cameras/ws` | phase 1 | the same as the preferences, for all three at once; a request to the socket's path that is no upgrade |
| `GET`, `PUT /api/ai/settings`, `GET /api/ai/organize`, `POST /api/ai/organize/turn` | phase 3 | any request with a bearer, which the backend's `/api` guard answers and no route behind it; the same as the preferences |
| `POST /auth/logout`, `GET /auth/me` | phase 3 | the same as the preferences |
| `GET /auth/local` on a device | phase 3 | off a device, where the backend does not mount it either; the same as the preferences |
| `GET /api/local`, `GET /api/local/tray` | phase 4a; on every device since 6.1 (`local.md`) | off a device, where the backend answers 404 |
| `GET /api/local/engine[/events]`, `POST /api/local/engine/{cmd,slots}`, `GET /api/internal/engine/local` | phase 4a, behind `K7_LOCAL_ENGINE=front` (`local.md`) | all of it unless a device's host turned the switch on |
| `GET /api/registry/hello`, `POST …/heartbeat`, `POST …/deregister`, `GET /api/engines`, `GET /api/engines/owners`, `GET`, `PUT /api/engines/<id>/shares`, `DELETE …/owner`, `POST /api/engine/{select,release}`, `POST /api/engine/<id>/rtc`, `WS /api/engine/<id>/ws`, `WS /api/engine-tunnel[/local/<key>]`, `GET /api/internal/engine/engines[/<id>]`, `GET …/hub`, `POST`, `DELETE …/bindings` | phase 4a, behind `K7_ENGINE_CORE=front` (`engine.md`) | all of it unless the switch is on; every other method there, a request to a socket's path that is no upgrade, the root domain's hosts, the seam's local half |

The front also answers `/api/engine-slots`, on every host (`engine/slots.md`). A path of that family no route
answers goes on to the backend, which holds none of it.

With `K7_LAST_ROUTES=front` (`last.md`) the front answers what it would otherwise send the backend: the seam's
`verify-token`, a bearer on `/api/preferences`, `/api/ai/*`, the root
domain's hosts, the built page and its fallback, and
Express's 404 and the `/api` guard's 401 for any other path. Nothing goes to the backend then.

The backend keeps no copy of the app releases, the cameras, the session but `/auth/me`, the
bearer tokens' routes: what the front sends
it on those paths gets its 404. Nobody signs in against an identity
provider: `/auth/login`, `/auth/callback` and `/auth/handoff/*` are on no side, and answer what a missing route does.

## What every owned route goes through

`src/web.rs` applies the backend's own chain, in its order: the security headers, a device's
loopback lock, CORS and the cross-site write refusal. `src/session.rs` is `requireAuth`.

| choice | why |
|---|---|
| a request the front would answer differently goes to the backend BEFORE that chain (`claim` in `src/lib.rs`): a method it does not own, a root-domain host, a session route without known revocations | the backend applies the chain itself, and running it twice would send `Vary` and the CORS headers twice, which a browser refuses |
| the revocations file is read again whenever its inode, modification time or size changes, and a revocation re-reads it and keeps each subject's later epoch (`k7-auth`'s `revocations.rs`, as @studio/oidc's `session.ts`) | a logout everywhere is written by either process; a front that cached the file would keep honouring a revoked session, and a whole-file write from a stale copy would undo the other's revocation |
| no auth folder known (neither `STUDIO_AUTH_DIR` nor `STUDIO_MEDIATEK`) means no session route is owned | a front that cannot see revocations must not decide who is signed in |
| on a device, any valid session that is not the local user's is answered as the local user, with a new local session cookie | as `server.ts` rewrites it, so a page signed in at a server sees the same account either way |
| the preferences schema is copied (`src/prefs_schema.rs`): zod's key order, nested strangers stripped, an unknown top-level key refused | the backend's other routes read the same file, so its bytes must be the backend's; a field added to `prefsSchema` in `apps/k7/backend/src/server.ts` must be added there too, until the TypeScript copy is deleted |

## The apps' downloads

| choice | why |
|---|---|
| the store's folder layout, pointers and file names are the previous release's to the byte (`src/routes/release_store.rs`), with no lock | a rollback serves the same `STUDIO_APP_RELEASES_DIR`; one CI run publishes a stamp at a time |
| `publishedAt` is the `current` pointer's time with its milliseconds rounded, where the file's `ETag` floors them | the feed reads as the previous release's did, which a rollback serves from the same folder |
| the page is rendered byte for byte, its Paris dates in English and French written out by hand (`src/routes/release_page.rs`) | no ICU or time-zone crate for one line of text |
| the family is answered on the root domain's hosts | the backend's site page answers the root domain's other paths, so `k7.rocks/app_downloads` is the same page |
| an upload is written as its frames arrive, and a download read as the client takes it (tokio's `fs` feature, no new crate) | a file reaches 2 GB; a 300 MB round trip leaves the front at about 10 MB |

## The remote cameras

The hub (`src/routes/cameras/`) and its rules, its limits, its frames and its ping, are [cameras.md](cameras.md).

| choice | why |
|---|---|
| the list, the ticket and the socket are owned or proxied together, by `claim`'s one rule | a ticket opens only the hub of the process that minted it, and a camera is listed only by the hub holding its socket |
| axum's `ws` feature (tokio-tungstenite) for the socket | it is the server half of the crate already serving; its frame cap and auto-answered pings are what the hub needs |
| a socket is admitted, named and counted before the 101 goes out, and released if the upgrade fails | the caps hold under concurrent upgrades, and the `hello` is the first frame the socket carries |
| the hub's state is behind one lock never held across an await; each socket's task writes only its own socket, and others reach it through a queue | a frame for another socket never waits on this one's network |
| shared vectors, `fixtures/camera-vectors.json`, written by the TypeScript helpers (`makeCameraVectors.ts`) | the browser keeps `@studio/web-protocol/cameras`; its test (`packages/web-protocol/cameraVectors.test.ts`) fails when the helpers change, the Rust one when Rust disagrees: name cleaning, uniqueness, matching and frame reading; `K7_CAMERA_ICE_SERVERS`' cases are kept as committed, the TypeScript hub being gone |

Regenerate those vectors only on purpose: `npx tsx apps/k7/backend-rs/fixtures/makeCameraVectors.ts`.

## The leaves

Each reads its state from disk on every request, from files the previous release reads after a rollback.

| choice | why |
|---|---|
| a request to `/api/preferences` or `/api/ai/*` with a bearer goes to the backend | the backend's `/api` guard checks a bearer against tokens the backend holds, and refuses a bad one even beside a valid cookie; behind it the backend has the preferences only, so a bearer on `/api/ai/*` hears the guard or a 404 |
| zod's messages are written out by hand for the three bodies these routes check | the browser shows `error` as it is; the vectors hold every message |
| shared vectors, `fixtures/leaf-vectors.json`, written by the TypeScript these routes replaced and kept as committed: the AI settings' patch and key hint | the AI patch and key hint; the file is the reference, and `src/vectors.rs` fails when Rust disagrees |

## The session

`src/routes/auth/` holds the device's session: `/auth/me`, `/auth/logout` and `/auth/local`, which signs the
machine's own user in. `src/routes/tokens.rs` mints the bearer tokens. `/auth/{logout,me}` come before a device's
rewrite to its local user, so `/auth/me` there names the account the cookie holds, and `/auth/local` and the token
routes after it. There is no provider: a machine has one account, and a cookie is only ever minted for it.

| choice | why |
|---|---|
| the session cookie is written as cookie-session 2 and the `cookies` library write it (`k7-auth`'s `cookie_session.rs`): found by their pattern in the raw header, rewritten only when it changed, a refused user dropped and the rest kept, a cleared session sent as an empty cookie dated 1970 with a real `.sig`, a bad signature clearing `.sig`, `SameSite=Lax`, `Secure` only behind a trusted https edge | a browser keeps whatever the `Set-Cookie` lines say, so they are part of the contract; the conformance suite compares them |
| the cookie is host-only: no `Domain`, no trusted sibling origin | a session shared across subdomains served a hosted instance; a device's page is one loopback origin |
| the front keeps `AUTH_ADMIN_GROUPS` and `session::is_admin` | the engine registry's ownership and the file hub's admin answers still ask it; the engine ownership model and the hub's accounts are the next simplification, not this one |
| a return target is a path on this origin, anything else is `/` | `/auth/local` is reachable from a link, and a redirect off the machine would hand it a session cookie's page |
| a signed-in `/auth/me` is asked of the backend again in the background, its answer dropped (`src/routes/auth/warm.rs`) | the front warms its own hub on it (`files.md`); a previous release's backend, the rollback, starts the account's file service on the same question, and repeating it costs one loopback request |
| the outgoing HTTPS client is `src/https.rs`: `hyper-rustls`, `rustls` on `ring`, and `webpki-roots` | `ring` is already in the tray's build, where `aws-lc-rs` compiles C with CMake and NASM on Windows and for every phone target |
| TLS trusts Mozilla's roots (`webpki-roots`) and the host's `K7_EXTRA_CA_CERTS` (`front.md`), not the OS store | it is one list on the tray and the phones, and needs no JNI start on Android |
| shared vectors, `fixtures/auth-vectors.json`, written by @studio/oidc's `createApiTokens` (`makeAuthVectors.ts`) | `apps/k7/backend/src/authVectors.test.ts` fails when @studio/oidc changes, `k7-auth`'s `tests/all/auth_vectors.rs` when Rust mints or opens other bytes |

Regenerate those vectors only on purpose: `npx tsx apps/k7/backend-rs/fixtures/makeAuthVectors.ts`.

## Known and accepted

Hazards the front has, as the TypeScript backend had them, kept for now:

| hazard | why it is accepted for now |
|---|---|
| on a device, `/auth/local` gives a full session to any process of the machine that can reach loopback and names a loopback `Host`: on Android any app | the device has one person, and the tray and the phones open their page this way; the fix is a per-install secret the app's page carries, which every host would have to hand its page first |
| loopback origins are credentialed CORS origins (`web.rs`): any page a browser on the machine opens can drive the tray | the tray's own page is a loopback origin; the fix is the same per-install secret |

## Where the front answers differently, on purpose

| difference | why it is accepted |
|---|---|
| no `ETag` on a JSON or HTML answer (the preferences, the cameras' list, a release feed, the downloads page, a share's manifest and comments), so never a 304 | those answers are a few kilobytes at most, and none is cached; a downloaded FILE has the backend's validator, and its 304 |
| no `ETag` on a share's document either | it is served `immutable` for a year, so a browser never asks again |
| an AI key hint cut inside an emoji writes U+FFFD where JavaScript writes half a surrogate pair | no key holds one; a lone surrogate cannot be in a Rust string |
| a sealed secret whose nonce is not 12 bytes or whose tag is not 16 does not open | node accepts other lengths, which only a hand-edited file holds; it counts as no secret, as any secret that does not open |
| a settings file holding JSON that is not an object reads as empty | nothing ever writes one; a hand-broken file is no 500 |
| comments are capped per share inside one write transaction | the ceiling holds against concurrent posts |
| `/app_downloads/` redirects with a plain-text body | the body of a 301 is never shown |
| outside `/auth` and the token routes, a session refused for its stamps gets a 401 without the cleared cookie the backend also sends | the browser keeps a cookie that keeps failing the same way; nothing reads the clearing |
| a token label cut inside an emoji loses the whole emoji | as a device name |

| the wording of a 400 for invalid JSON, or for a path segment that is not valid percent-encoding | a client shows `error` as it is; none parses it |
| a camera socket's upgrade passes the owned chain, so a device's loopback lock applies to it | the tray and the phones listen on loopback, so only a page naming another host is refused |
| a device name capped in the middle of an emoji loses the whole emoji | JavaScript's `slice` keeps half of it, which no page can show |
| a frame past 64 KiB, or a malformed one, ends the socket without a 1009 close frame | the camera and the viewer reconnect on any close; the code changes nothing they do |
| a stopping front sends 1001 to every camera socket, but does not wait for the close handshake | the process is ending; the devices reconnect either way |
