# Where the backend reads its configuration

## `STUDIO_FILES_ROOT` is required, `STUDIO_MEDIATEK` is not

**`STUDIO_FILES_ROOT`** — where the accounts' workspaces live — has no default, and the Rust front
answers no file hub without it (`apps/k7/backend-rs/crates/k7-front/src/routes/files/mod.rs`): this is
the only place a workspace lives, and a default would put every account's workspaces somewhere nobody
chose (`apps/k7/backend-rs/docs/files/hub.md`). The backend itself reads it no longer.

**`STUDIO_MEDIATEK`** — the instance's data folder (`.auth`, `.shares`, `.engines`) — is optional, like everywhere else: absence means
`~/.k7/mediatek` (`mediatekRoot()` in `packages/song/src/mediatekRoot.ts`). Refusing to start
without it would only guard a misconfigured production deployment, and the Dockerfile sets
`ENV STUDIO_MEDIATEK=/library` there; in dev the fallback is already the right answer. The Rust front
reads the same variable; this backend reads it for the token stores' folder (`.auth`) and the root the file services and the
engine jobs share.

## The loader writes into `process.env`

`apps/k7/backend/src/server.ts` loads its section of the root `env.yaml` — `k7-backend`, which
holds the keys of the front beside the session's — with `loadServiceEnv` and assigns the result into
`process.env` rather than keeping it in a constant.

The path is not passed from hand to hand: every consumer resolves it itself by reading the
environment, starting with `mediatekRoot()` (`packages/song/src/mediatekRoot.ts`), which
`@studio/song` and the file service lean on. A loader that merely
returned the value — `defineEnv`, which validates and answers an object without touching
`process.env` — would leave every one of those on the `<state>/mediatek` fallback while the
backend saw its own value and passed its own guard. The result is a service that starts,
declares itself healthy, and serves the wrong directory.

Import order does not matter, and must not be made to: nothing in the import graph reads
`STUDIO_MEDIATEK` at import time, `mediatekRoot()` being a function called at run time. A
formatter reordering imports would otherwise break this silently.

## Precedence

The yaml wins wherever it declares a key (`@studio/env`): explicit, per-service dev
configuration, so an ambient variable cannot annex a backend in passing. In production there is
no `env.yaml` — the images exclude it — and the Dockerfile sets `ENV STUDIO_MEDIATEK=/library`.

## What else changes behaviour

The front's own variables — `FFMPEG_BIN`, the dot
directories `STUDIO_PROJECTS_DIR` and `STUDIO_SHARES_DIR` —
are the front's, read from the same section (`ENV_KEYS`, `apps/k7/backend-rs/docs/front.md`). What
this backend still reads:

- `SESSION_SECRET` — the seed of the API token secret when `API_TOKEN_SECRET` is not set
  (`docs/tokens.md`).
- `STUDIO_PREFS_DIR` — the one preferences document per person (`engineId`, `commands`,
  `playbackTarget`), which is k7's and not the volume's: `~/.studio/prefs/k7` by default,
  `/data/prefs` in the image.
- `STUDIO_FILES_WATCH=1` — set by `npm run dev` only: the account file services it starts restart
  on a source change, which this process's own watch cannot see.

That one is set by `apps/k7/backend/scripts/with-env.mjs`, which `dev` goes through: npm hands a
script to `cmd.exe` on Windows, and a leading `VAR=1` is shell syntax it reads as the command name.

## One side effect

`loadServiceEnv` merges `common` with the service's section, by design. Real `OIDC_*` values in
`common` therefore swing the session in dev — the front reads the same keys — from the "Dev User"
stub to a real identity provider. Keeping the stub means keeping those keys out of `common`.
