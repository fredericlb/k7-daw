# The routes the front owns

A route is the front's once the conformance suite (`e2e/docs/conformance.md`) passes against it.

| route | owned since | what it declines |
|---|---|---|
| `GET /api/health` | phase 1 | nothing |
| `GET`, `PUT /api/preferences` | phase 1 | a method it does not own |
| `GET /api/cameras`, `POST /api/cameras/ticket`, `WS /api/cameras/ws` | phase 1 | the same as the preferences, for all three at once; a request to the socket's path that is no upgrade |
| `GET`, `PUT /api/ai/settings`, `GET /api/ai/organize`, `POST /api/ai/organize/turn` | phase 3 | the same as the preferences |
| `GET /auth/me` | phase 3 | the same as the preferences |
| `GET /api/local` | phase 4a; on every device since 6.1 (`local.md`) | off a device, where the backend answers 404 |
| `GET /api/local/engine[/events]`, `POST /api/local/engine/{cmd,slots}`, `GET /api/internal/engine/local` | phase 4a, behind `K7_LOCAL_ENGINE=front` (`local.md`) | all of it unless a device's host turned the switch on |
| `GET /api/registry/hello`, `POST …/heartbeat`, `POST …/deregister`, `GET /api/engines`, `POST /api/engine/{select,release}`, `POST /api/engine/<id>/rtc`, `WS /api/engine/<id>/ws`, `WS /api/engine-tunnel[/local/<key>]`, `GET /api/internal/engine/engines[/<id>]`, `GET …/hub`, `POST`, `DELETE …/bindings` | phase 4a, behind `K7_ENGINE_CORE=front` (`engine.md`) | all of it unless the switch is on; every other method there, a request to a socket's path that is no upgrade, the root domain's hosts, the seam's local half |

The front also answers `/api/engine-slots`, on every host (`engine/slots.md`). A path of that family no route
answers is declined.

What a route declines (a method it does not own) ends in the last routes (`last.md`): the seam's
the built page and its fallback, and
the 404 and the `/api` guard's 401 for any other path.

Nobody signs in against an identity provider: `/auth/login`, `/auth/callback` and `/auth/handoff/*` are no route, and answer what a missing route does.

## What every owned route goes through

`src/web.rs` applies the backend's own chain, in its order: the security headers, a device's
loopback lock, CORS and the cross-site write refusal. `src/session.rs` is `requireAuth`: the token cookie (`sessions.md`).

| choice | why |
|---|---|
| a request the front would answer differently is declined BEFORE that chain (`claim` in `src/lib.rs`): a method it does not own | the last routes apply the chain once, and running it twice would send `Vary` and the CORS headers twice, which a browser refuses |
| the preferences schema is copied (`src/prefs_schema.rs`): zod's key order, nested strangers stripped, an unknown top-level key refused | a stored document must still pass the gate on its next save; a field added to the schema is added there (`src/prefs_schema.rs`) |

## The remote cameras

The hub (`src/routes/cameras/`) and its rules, its limits, its frames and its ping, are [cameras.md](cameras.md).

| choice | why |
|---|---|
| the list, the ticket and the socket are owned or proxied together, by `claim`'s one rule | a ticket opens only the hub of the process that minted it, and a camera is listed only by the hub holding its socket |
| axum's `ws` feature (tokio-tungstenite) for the socket | it is the server half of the crate already serving; its frame cap and auto-answered pings are what the hub needs |
| a socket is admitted, named and counted before the 101 goes out, and released if the upgrade fails | the caps hold under concurrent upgrades, and the `hello` is the first frame the socket carries |
| the hub's state is behind one lock never held across an await; each socket's task writes only its own socket, and others reach it through a queue | a frame for another socket never waits on this one's network |
| shared vectors, `fixtures/camera-vectors.json`, written by the TypeScript helpers (`makeCameraVectors.ts`) | the browser keeps `@studio/web-protocol/cameras`; its test (`apps/k7-daw-frontend/packages/web-protocol/cameraVectors.test.ts`) fails when the helpers change, the Rust one when Rust disagrees: name cleaning, uniqueness, matching and frame reading; `K7_CAMERA_ICE_SERVERS`' cases are kept as committed, the TypeScript hub being gone |

Regenerate those vectors only on purpose: `npx tsx apps/k7-daw-backend/fixtures/makeCameraVectors.ts`.

## The leaves

Each reads its state from disk on every request, from files the previous release reads after a rollback.

| choice | why |
|---|---|
| zod's messages are written out by hand for the three bodies these routes check | the browser shows `error` as it is; the vectors hold every message |

## The session

`src/routes/auth.rs` is `GET /auth/me`, which hands a page the install's token and says who the one user is;
the token and its cookie are `sessions.md`. There is no provider, no sign-out and no `/auth/local`.

| choice | why |
|---|---|
| a signed-in `/auth/me` warms the account's hub in the background | the files are warm by the time the page asks (`files.md`) |
| the outgoing HTTPS client is `src/https.rs`: `hyper-rustls`, `rustls` on `ring`, and `webpki-roots` | `ring` is already in the desktop app's build, where `aws-lc-rs` compiles C with CMake and NASM on Windows and for every phone target |
| TLS trusts Mozilla's roots (`webpki-roots`) and the host's `K7_EXTRA_CA_CERTS` (`front.md`), not the OS store | it is one list on the desktop app and the phones, and needs no JNI start on Android |

## Known and accepted

| hazard | why it is accepted for now |
|---|---|
| any process of the machine that reaches loopback and names a loopback `Host` is given the token: on Android any app | the device has one person; what stops a web page is the token, and what stops the network is the loopback bind (`standalone.md`) |
| loopback origins are credentialed CORS origins (`web.rs`): a page on another loopback port the browser opens can read the answers | the desktop app's own page is a loopback origin |

## Where the front answers differently, on purpose

| difference | why it is accepted |
|---|---|
| no `ETag` on a JSON or HTML answer (the preferences, the cameras' list, a share's manifest and comments), so never a 304 | those answers are a few kilobytes at most, and none is cached; a downloaded FILE has the backend's validator, and its 304 |
| no `ETag` on a share's document either | it is served `immutable` for a year, so a browser never asks again |
| an AI key hint cut inside an emoji writes U+FFFD where JavaScript writes half a surrogate pair | no key holds one; a lone surrogate cannot be in a Rust string |
| a sealed secret whose nonce is not 12 bytes or whose tag is not 16 does not open | node accepts other lengths, which only a hand-edited file holds; it counts as no secret, as any secret that does not open |
| a settings file holding JSON that is not an object reads as empty | nothing ever writes one; a hand-broken file is no 500 |
| comments are capped per share inside one write transaction | the ceiling holds against concurrent posts |
| outside `/auth` and the token routes, a session refused for its stamps gets a 401 without the cleared cookie the backend also sends | the browser keeps a cookie that keeps failing the same way; nothing reads the clearing |
| a token label cut inside an emoji loses the whole emoji | as a device name |

| the wording of a 400 for invalid JSON, or for a path segment that is not valid percent-encoding | a client shows `error` as it is; none parses it |
| a camera socket's upgrade passes the owned chain, so a device's loopback lock applies to it | the desktop app and the phones listen on loopback, so only a page naming another host is refused |
| a device name capped in the middle of an emoji loses the whole emoji | JavaScript's `slice` keeps half of it, which no page can show |
| a frame past 64 KiB, or a malformed one, ends the socket without a 1009 close frame | the camera and the viewer reconnect on any close; the code changes nothing they do |
| a stopping front sends 1001 to every camera socket, but does not wait for the close handshake | the process is ending; the devices reconnect either way |
