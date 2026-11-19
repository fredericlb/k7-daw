# The file hub: workspaces and libraries

The file hub is k7's Rust front's (`apps/k7/backend-rs/docs/files.md`), and the only place workspaces
live. It hands each account to its own [file-service](./36-file-service.md) runtime, held in the
front, and owns what is shared between accounts or devices: the libraries.

The routes are `apps/k7/backend-rs/crates/k7-front/src/routes/files/` (the hub's routes in `hub/`); tickets, the environment allow-list and
the engine bindings are `apps/k7/backend-rs/crates/k7-hub`, the history rooms
`crates/k7-sync`, the libraries `crates/k7-libraries`.

## STUDIO_FILES_ROOT

The front answers the file hub only with `$STUDIO_FILES_ROOT` set; unset or blank, it says so at start
and answers none of it. There is no fallback, because a default would put every account's
workspaces somewhere nobody chose. Under it:

```text
$STUDIO_FILES_ROOT/
  libraries.json                  the account libraries' list
  libraries/<name>/1/ 2/ … published versions (each with library.k7x)
  <account-slug>/                 the account's STUDIO_HOME
    workspace/                    its workspaces
    .history/  .renders/  models/ undo journals, automatic renders, downloaded models
```

The account slug is `accountKey(email)` (`apps/k7/backend-rs/crates/k7-hub/src/account.rs`): a slug of the email plus
ten hex characters of its sha256, so two emails that slug alike stay apart.

Housekeeping: `apps/k7/backend-rs/crates/k7-hub/src/stale.rs` purges `.renders` and `.history` entries unused for 30 days;

## One runtime per account, in the front

The hub starts no process. An account's runtime is made lazily on its first request (or by the
warm-up at sign-in) and let go after the idle time once nothing holds it. One runtime per account, never one shared,
keeps an account's state apart and lets an idle one cost memory only.

- **Environment:** an allow-list of the front's (`crates/k7-hub/src/env.rs`) overlaid by the host's
  settings, plus the account's own `STUDIO_HOME=<root>/<slug>`, `STUDIO_LIBRARIES_HOME=<root>/libraries`,
  `STUDIO_RENDER_JOBS_URL`
  when renders are on, and the shares' `STUDIO_MEDIATEK` (the instance's data folder).
  The front's secrets are not on the list.
- **Crash:** nothing to restart; a runtime is memory in the front.

**Routing** (`crates/k7-front/src/routes/files/hub/proxy.rs`): the user comes from a signed ticket
(`x-studio-engine-token` or `?token=`; HMAC over `{u, exp, a?}`, 12 h, minted by `GET /api/files-hub`)
or from the session. `/account/libraries…` goes to the libraries; everything else to the account's
runtime. A ticket may carry `~<engineId>` to bind that engine to the account first.

## Routes

| Route | Auth | Purpose |
|---|---|---|
| `GET /api/files-hub` | session | Mint a ticket: `{ path, ticket }`. |
| `/api/files/*` | ticket or session | The account's file service. |
| `GET /api/files/account/libraries` | ticket or session | The libraries list. |
| `/api/files-hub/internal/*` | the account's secret | Called by an account's runtime: `libraries/sync`. |

## Libraries

`crates/k7-libraries` (`apps/k7/backend-rs/docs/libraries.md`):

- A library is a folder of **versions**: `1/`, `2/`… frozen, each with a `library.k7x`
  (`{ name, version, requires, notes? }`). A version is never rewritten.
- A listed library missing on disk is installed from its seed (`$STUDIO_LIBRARY_SEED`: a folder of
  library folders, or `name=folder,…`), copied to a dot-named sibling then renamed into place, under
  the version its `library.k7x` declares; with no seed it starts empty. A seed declaring a version
  the machine lacks is added beside the others, at the next free number when its own is taken.
- A version folder holding no file but dot-files is no version: `ensure` removes it.
- `publish` copies the draft to `max + 1` with a new manifest. `requires` (the lowest
  `LIBRARY_LEVEL` that runs it) defaults to the previous version's.
- Which version a workspace uses is the workspace's `_library-pins.k7x`, read by the `song` crate;
  the hub does not pin.

Distribution: a library is frozen to the seed the app bundles; it changes with an app update and
nothing is fetched.

## Environment

| Variable | Read by | Meaning |
|---|---|---|
| `STUDIO_FILES_ROOT` | the front | Required data root. |
| `STUDIO_LOCAL_USER` | the front | `1` makes the front a device's own. |
| `STUDIO_LIBRARY_SEED` | `k7-libraries` | Where new libraries are seeded from. |

## Tests

The Rust crates hold their own tests (`apps/k7/backend-rs/crates/k7-front/tests/files_hub.rs`, `all/files_sync.rs`, `k7-hub/tests/`, `k7-sync/tests/`,
`k7-libraries/tests/`), checked against the committed vectors in `apps/k7/backend-rs/fixtures/`; the
conformance specs (`e2e/specs/conformance/files*`, `sync*`) drive the whole family through the front.
