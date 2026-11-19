# Tokens

`npm run tokens` gives k7's front its own `API_TOKEN_SECRET` in `env.yaml`, and does nothing
else. It is idempotent — a secret already there is left alone — so it is safe on every checkout.

```
npm run tokens              # generate k7-front.API_TOKEN_SECRET if it is missing
npm run tokens -- --dry-run # say what it would do
```

## Why no service token is minted

A token is a signed `{jti, sub, iat, exp}` (`apps/k7/backend-rs/crates/k7-auth/src/api_token.rs`). Every service that calls k7's API with one runs inside k7, so k7 mints its token in code,
and nobody pastes one anywhere (`apps/k7/backend-rs/docs/sessions.md`):

| caller | `sub` | minted by |
|---|---|---|
| an account's file service | the account | the front's hub, when it makes the account (`apps/k7/backend-rs/crates/k7-front/src/routes/files/hub/spawn.rs`) |

Nothing reaches the library over a loopback address typed into a config file, and nothing has to
be pasted into `env.yaml`.

## Why the secret is still generated

Left unset, the front signs with `${SESSION_SECRET}:pl8`. That works, and is what nobody should rely on: rotating the session
secret would then log everyone out *and* revoke every machine's sync in one move. Restart k7's
front after the secret is written; every token signed with the previous one is void.

## Rotation and revocation

A token is valid only while its id is listed in the token store, so its owner revokes one alone,
and it expires after 90 days (`apps/k7/backend-rs/docs/sessions.md`). Rotating a secret still kills
every token signed with it at once. A token of the `{sub, iat}` format, with no id, is refused.

## The URLs are not configuration

`packages/web-protocol/services.ts` holds the one table, every address derived from k7's origin
— `https://app.k7.rocks` in production (`packages/web-protocol/docs/addresses.md`). `play.st0.app`
and `k7.st0.app` are former hosts read as that origin. **`/auth/*` belongs to
k7**, on its origin.
