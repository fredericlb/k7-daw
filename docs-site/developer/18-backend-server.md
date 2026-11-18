# The k7 backend server

`apps/k7/backend/src/server.ts` is one Express process (run under `tsx` in dev, as a bundle under Node 24 in the image). It sits behind k7's Rust front: it reads the session the front mints, stores each account's preferences, verifies the bearer tokens for the front, and serves the built frontend — the image ships no nginx. The engine core, the shares, the file hub that starts each account's file service, and the AI settings and organize are the Rust front's (`apps/k7/backend-rs/docs/`). Dev port 8790 (`PORT`), behind the front on 8789.

It has **no file routes**. Every file and project operation belongs to the account's file service (`apps/k7/backend-rs/crates/k7-file-service`), which the browser reaches through the front's hub; see [Frontend and backend boundary](./19-frontend-backend-boundary.md).

## Order of the middleware

The order is load-bearing; `server.ts` mounts, top to bottom:

1. **Refusal to start** (`apps/k7/backend/src/authGuard.ts`): unless `STUDIO_LOCAL_USER=1` (a device's own backend) or `STUDIO_DEV_AUTH=1` (a local dev session), the process exits when `OIDC_ISSUER`/`OIDC_CLIENT_ID` are missing (every visitor would be signed in as the dev stub) or when `SESSION_SECRET` is the public default or shorter than 32 characters.
2. **Security headers** (`apps/k7/backend/src/hardening.ts`): `X-Content-Type-Options: nosniff`, a strict referrer policy; Express's `x-powered-by` is off.
3. **Loopback only** (`apps/k7/backend/src/loopback.ts`) when `STUDIO_LOCAL_USER=1`: the tray's and phone's backend answer their own machine only, by peer address **and** `Host` header (a rebound DNS name reaching 127.0.0.1 is refused).
4. **CORS** (`apps/k7/backend/src/cors.ts`, `studioCors(originPolicyFromEnv())`): credentialed calls are answered only for origins under `COOKIE_DOMAIN`, those in `TRUSTED_ORIGINS`, and loopback pages on a user's own machine; a cross-site write from any other origin is refused.
5. One shared JSON parser (`apps/k7/backend/src/bodyLimits.ts`) at 256 kB.
6. **Session** (`setupAuth` from `@studio/oidc`, `/auth/me` alone, code kept for a later rework in `docs/inventaire.md`): the session cookie `studio.sid` is read here, while the logout, `/auth/local` and the bearer tokens are the Rust front's (`apps/k7/backend-rs/docs/routes.md`).
7. **The bearer tokens** (`mountMedia`, `apps/k7/backend/src/media/mount.ts`): the tokens' store, which guards every `/api` route after it, and the internal seam's `verify-token`; the file hub is the Rust front's.
8. **Preferences**: `/api/preferences` behind `requireAuth`.
9. **Aliases**: on a host starting `play.` or `k7.`, every page (not `/api`, `/auth`) is redirected 301 to k7's home (`/`) on k7's root origin.
10. **The SPA**: precompressed static files from the frontend's build output (`dist` under `apps/k7/frontend`) — hashed assets cached for a year as immutable, the shell `no-cache` and only framable by the same origin — then `index.html` for every path except `/api`, `/auth` and `/assets/` (a missing chunk must 404, not return HTML).
11. **`jsonErrorHandler`**, last: a JSON body for every error, never a 5xx's message or stack.

`STUDIO_LISTEN_HOST` binds one interface (the tray and the phone set `127.0.0.1`). Requests may run 30 minutes (large uploads). On `SIGTERM`/`SIGINT` the server drains within 4 s of Docker's 10.

## Preferences

`packages/user-prefs` is the store: one small JSON file per user, named by a slug and a hash of the session's user, written beside then renamed so a crash never leaves a file that reads as empty. `PREFS_DIR` is `STUDIO_PREFS_DIR` or `~/.studio/prefs/k7`. `preferencesRouter` (`packages/user-prefs/router.ts`) serves `GET` and `PUT /preferences`; a PUT replaces the document wholesale (so a field can be cleared).

The **zod schema in `server.ts` is the gate**, not a description:

```ts
const prefsSchema = z.object({
  engineId: z.string().optional(),
  commands: z.record(z.string(), commandPrefSchema).optional(),
});
```

A field the browser sends and the schema does not declare is refused with a 400 naming it (`droppedKeys`), rather than silently stripped. Widening `K7Prefs` in `apps/k7/frontend/src/api.ts` is therefore only half of adding a preference; the other half is this schema. `engineId` is an id and never an address, since an engine's address changes between networks.

These are the **account's** preferences. The machine's own (vim mode, the audio device, channel labels) are the file service's `/preferences`, on the engine's machine.

## AI settings and AI organize, in the front

k7's Rust front keeps each account's Claude API key and model in a second prefs store (`<PREFS_DIR>/ai`). `GET /api/ai/settings` answers the model, the default model and a **hint** of the key (`sk-ant-…wxyz`), never the key. `PUT` is a patch: an absent field is kept, `null` or `""` clears it. This is why the key is not a field of the preferences document, which the browser reads and writes whole.

`apps/k7/backend-rs/crates/k7-front/src/routes/ai/turn.rs` is the module builder's AI organize, the server's half: `POST /api/ai/organize/turn` makes one Claude turn with the account's key and model and streams it as NDJSON with a heartbeat. The conversation is the page's: it builds the prompt, checks the answer with the builder's own rules (`@studio/panel-rules/ai-organize-loop`, React-free), and hands the model its mistakes once (two rounds at most) as a second turn. The system prompt is cached. With no key on the account, the route is off and the builder shows no button (`apps/k7/frontend/docs/aiOrganize.md`).

## Running and checking

`npm run dev` at the repository root starts it with the frontend and the rest; alone, `npm run dev --workspace=k7-backend`. The Vite dev server proxies `/api` and `/auth` to it. `npm run test` and `npm run typecheck` in `apps/k7/backend`.
