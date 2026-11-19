# The k7 front

`apps/k7/backend-rs/crates/k7-front` is k7's one backend process: a Rust server that reads and mints the session, stores each account's preferences, answers the file hub that starts each account's file service, the engine core, the shares, the AI settings and organize, and serves the built frontend — the image ships no nginx (`apps/k7/backend-rs/docs/front.md`). Dev port 8789.

It has **no file routes of its own**. Every file and project operation belongs to the account's file service (`apps/k7/backend-rs/crates/k7-file-service`), which the browser reaches through the front's hub; see [Frontend and backend boundary](./19-frontend-backend-boundary.md).

## Order of the chain

The order is load-bearing; `web.rs` applies, top to bottom, to every route the front owns:

1. **Refusal to start** (`crates/k7-front/src/auth_guard.rs`): unless `STUDIO_LOCAL_USER=1` (a device) or `STUDIO_DEV_AUTH=1` (a local dev session), the process refuses to start when `SESSION_SECRET` is the public default or shorter than 32 characters.
2. **Security headers**: `X-Content-Type-Options: nosniff`, a strict referrer policy.
3. **Loopback only** when `STUDIO_LOCAL_USER=1`: the tray's and phone's front answer their own machine only, by peer address **and** `Host` header (a rebound DNS name reaching 127.0.0.1 is refused).
4. **CORS**: credentialed calls are answered only for loopback pages on a user's own machine; a cross-site write from any other origin is refused.
5. One shared JSON parser at 256 kB.
6. **Session**: the session cookie `studio.sid` is read and minted (`apps/k7/backend-rs/docs/sessions.md`); `/auth/me`, `/auth/logout` and `/auth/local` are routes of the front (`apps/k7/backend-rs/docs/routes.md`).
7. **The `/api` guard** (`routes/last/api.rs`): a request without a session is refused.
8. **Preferences**: `/api/preferences` behind the session.
9. **The SPA**: precompressed static files from the frontend's build output (`dist` under `apps/k7/frontend`) — hashed assets cached for a year as immutable, the shell `no-cache` and only framable by the same origin — then `index.html` for every path except `/api`, `/auth` and `/assets/` (a missing chunk must 404, not return HTML).
10. The 404 page, last.

The front's address is the tray's or phone's loopback port, or `K7_FRONT_LISTEN`. Requests may run 30 minutes (large uploads). On `SIGTERM`/`SIGINT` the front stops what it started and drains.

## Preferences

One small JSON file per user, named by a slug and a hash of the session's user, written beside then renamed so a crash never leaves a file that reads as empty. The folder is `STUDIO_PREFS_DIR` or `~/.studio/prefs/k7`. `GET` and `PUT /api/preferences` serve it; a PUT replaces the document wholesale (so a field can be cleared).

The **schema in `crates/k7-front/src/prefs_schema.rs` is the gate**, not a description: it declares `engineId` and `commands` (and two legacy fields kept so a stored document still passes).

A field the browser sends and the schema does not declare is refused with a 400 naming it, rather than silently stripped. Widening `K7Prefs` in `apps/k7/frontend/src/api.ts` is therefore only half of adding a preference; the other half is this schema. `engineId` is an id and never an address, since an engine's address changes between networks.

These are the **account's** preferences. The machine's own (vim mode, the audio device, channel labels) are the file service's `/preferences`, on the engine's machine.

## AI settings and AI organize, in the front

k7's Rust front keeps each account's Claude API key and model in a second prefs store (`<PREFS_DIR>/ai`). `GET /api/ai/settings` answers the model, the default model and a **hint** of the key (`sk-ant-…wxyz`), never the key. `PUT` is a patch: an absent field is kept, `null` or `""` clears it. This is why the key is not a field of the preferences document, which the browser reads and writes whole.

`apps/k7/backend-rs/crates/k7-front/src/routes/ai/turn.rs` is the module builder's AI organize, the front's half: `POST /api/ai/organize/turn` makes one Claude turn with the account's key and model and streams it as NDJSON with a heartbeat. The conversation is the page's: it builds the prompt, checks the answer with the builder's own rules (`@studio/panel-rules/ai-organize-loop`, React-free), and hands the model its mistakes once (two rounds at most) as a second turn. The system prompt is cached. With no key on the account, the route is off and the builder shows no button (`apps/k7/frontend/docs/aiOrganize.md`).

## Running and checking

`npm run dev` at the repository root starts the front (a cargo build, the Tiltfile's `k7-front`) with the frontend and the rest. The Vite dev server proxies `/api` and `/auth` to it. `cargo test -p k7-front` in `apps/k7/backend-rs`.
