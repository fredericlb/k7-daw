# The session cookie

`studio.sid` is minted and verified by `k7-auth` in the cookie-session and Keygrip format, and the suite's
helper (`e2e/src/sessionCookie.ts`) mints the same bytes. The cookie's bytes are a **contract**: the vectors in
`apps/k7/backend-rs/fixtures/session-vectors.json` freeze it.

## The format

- `studio.sid` is the base64 (standard alphabet, padded) of `JSON.stringify({user, iat, exp})`,
  plus `_expire` when a lifetime was given.
- `studio.sid.sig` is Keygrip's default signature of the string `studio.sid=<value>`: HMAC-SHA1
  with the secret, base64 with `+/` written `-_`, without padding. The cookie's NAME is signed too.
- A signature proves nothing about time. A session is valid only while `exp` and `_expire` are
  ahead, its `iat` is not in the future beyond a minute of skew, its groups are under 12 hours old,
  and its subject's revocation epoch is not after `iat`.

## The choices

| choice | why |
|---|---|
| frozen vectors, `apps/k7/backend-rs/fixtures/session-vectors.json` | the Rust test (`crates/k7-auth/tests/all/vectors.rs`) fails when Rust disagrees with the stored cookies: a format change is a deliberate edit of the vectors |
| the vectors include non-ASCII, quotes, a control character, nested values and a 300-character subject | those are where a JSON serializer differs from `JSON.stringify` |
| `serde_json` with `preserve_order` | a minted cookie must list its keys in the order `JSON.stringify` writes them, or its bytes, and so its signature, differ |
| the payload is decoded as leniently as node's `Buffer.from(…, "base64")`: either alphabet, padding optional | a cookie node accepts must not be refused here |
| the signature is compared in constant time | as Keygrip's `verify` |
| the `Set-Cookie` lines are cookie-session's too (`crates/k7-auth/src/cookie_session.rs`), with no `Domain` | the front mints and clears the device's session; when and how a cookie is rewritten is what a browser keeps (`docs/routes.md`, the session) |
| every host's start refuses a session secret shorter than 32 characters or the public development default, unless `STUDIO_DEV_AUTH=1` or `STUDIO_LOCAL_USER=1` (`crates/k7-front/src/auth_guard.rs`). The binary exits 2, `k7_front_start` answers -5 | a known secret lets anyone mint a cookie |
| the public default secret is read only where the dev stub may run; elsewhere an unset secret is no secret, the front then claims no session route, and the recorder stays unclaimed | a front started past the guard (a test, a host that forgot it) must fail closed, never sign with a literal anyone can read here |

The vectors are written once, not generated: a change to the format edits them on purpose.
