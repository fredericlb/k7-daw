# The session cookie, in two languages

`studio.sid` is minted by `@studio/oidc` (`packages/oidc/session.ts`) and verified by
every studio service, phare's included, which stays TypeScript. So the cookie's bytes are a
**contract between languages**: `k7-auth` reads and mints it exactly as cookie-session and Keygrip
do, and the TypeScript stays the source of truth.

## The format

- `studio.sid` is the base64 (standard alphabet, padded) of `JSON.stringify({user, iat, exp})`,
  plus `_expire` when a lifetime was given.
- `studio.sid.sig` is Keygrip's default signature of the string `studio.sid=<value>`: HMAC-SHA1
  with the secret, base64 with `+/` written `-_`, without padding. The cookie's NAME is signed too.
- A signature proves nothing about time. A session is valid only while `exp` and `_expire` are
  ahead, its `iat` is not in the future beyond a minute of skew, its groups are under 12 hours old,
  and its subject's revocation epoch is not after `iat` (`packages/oidc/docs/session.md`).

## The choices

| choice | why |
|---|---|
| shared vectors, `apps/k7/backend-rs/fixtures/session-vectors.json`, written by @studio/oidc's own `sessionCookieHeader` (`makeSessionVectors.ts`) | the TypeScript test (`apps/k7/backend/src/sessionVectors.test.ts`) fails when @studio/oidc changes the format, and the Rust test (`crates/k7-auth/tests/all/vectors.rs`) fails when Rust disagrees with it: a format change cannot reach one side silently |
| the vectors include non-ASCII, quotes, a control character, nested values and a 300-character subject | those are where a JSON serializer differs from `JSON.stringify` |
| `serde_json` with `preserve_order` | a minted cookie must list its keys in the order `JSON.stringify` writes them, or its bytes, and so its signature, differ |
| the payload is decoded as leniently as node's `Buffer.from(…, "base64")`: either alphabet, padding optional | a cookie node accepts must not be refused here |
| the signature is compared in constant time | as Keygrip's `verify` |
| the `Set-Cookie` lines are cookie-session's too (`crates/k7-auth/src/cookie_session.rs`), with no `Domain` | the front mints and clears the device's session; when and how a cookie is rewritten is what a browser keeps (`docs/routes.md`, the session) |
| the API tokens have vectors of their own, `fixtures/auth-vectors.json` | the same contract as the cookie's, for the other thing signed with `SESSION_SECRET` |
| every host's start refuses what node's `authGuard.ts` refuses, word for word (`crates/k7-front/src/auth_guard.rs`): no provider, or a secret shorter than 32 characters or @studio/oidc's public default, unless `STUDIO_DEV_AUTH=1` or `STUDIO_LOCAL_USER=1`. The binary exits 2, `k7_front_start` answers -5 | without a provider the dev stub signs ANY visitor in as one shared "Dev User", and a known secret lets anyone mint a cookie; once the front starts alone, node's guard no longer runs before it |
| @studio/oidc's default secret is read only where the stub may run; elsewhere an unset secret is no secret, the front then claims no session route, and the key derived from it (the API tokens') is random per process and the recorder stays unclaimed | a front started past the guard (a test, a host that forgot it) must fail closed, never sign with a literal anyone can read here |

Regenerate the vectors only on purpose, since they are what the two sides agree on:
`npx tsx apps/k7/backend-rs/fixtures/makeSessionVectors.ts`.
