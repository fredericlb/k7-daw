# Sessions and tokens: where k7 keeps what makes them revocable

The rules — stamps, lifetimes, revocation epochs, token ids — are `@studio/oidc`'s
(`packages/oidc/docs/session.md`, `packages/oidc/docs/apiTokens.md`). This page is
what k7 chooses on top.

## One folder, in the instance's data folder

`src/authState.ts` puts that state under `$STUDIO_AUTH_DIR`, else `.auth/` in the instance's data folder
(`mediatekDir()`): it survives a redeploy with that folder.

| file | holds |
|---|---|
| `session-revocations.json` | each account's "not before" epoch, written by `POST /auth/logout?everywhere=1` |
| `library-tokens.json` | the bearer tokens the backend still checks; nothing mints them any more |

The Rust front writes the revocations, and this backend writes a token's `lastUsed` when it checks a bearer;
each re-reads the file before it writes (`apps/k7/backend-rs/docs/routes.md`, the sign-in).

## Lifetimes

k7 takes the package's defaults: a session lives 7 days, and 12 hours when it carries groups — the
bound on how long an admin removed at the IdP keeps admin rights here. A token lives 90 days.

## What a deploy of these rules does

Every session minted before the stamps existed is refused, so everyone signs in once. Every token in
the old `{sub, iat}` format is refused: a CLI or an app holding one mints a new one (the pairing
for an app).

## Tokens handed over in code

The token each account's file service is started with is **ephemeral**: held in this process's memory, never listed, and dead when it
stops — which is also when that holder stops.

## A machine's session is its local user

Under `STUDIO_LOCAL_USER` (the tray, the phones) every session is the machine's local user: the backend
turns any other one into it (`src/server.ts`). A session that expires, or a cookie left from another sign-in, would otherwise move the
machine to another account folder, and its workspaces with it. The machine keeps no
provider token: nothing on it speaks for the person at the provider.

## The token routes

They are the Rust front's.

| route | does |
|---|---|
| `POST /api/token` | mint a bearer token, `{label}` in the body |
| `GET /api/token` | the owner's bearer tokens, without the tokens themselves |
| `DELETE /api/token/:id` | revoke one of them |

All of them take the session only. A POST or DELETE from a site outside the origin allowlist is
refused before it reaches them (`cors.md`).
