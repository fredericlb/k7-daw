# The file hub — a device's workspaces, a server's libraries

A workspace lives on the device that edits it and travels between devices through git
(`git.md`). A **server** edits none. So the front has two modes, told by `STUDIO_LOCAL_USER`:

| | a device (the tray, a phone) | a server |
|---|---|---|
| `/api/files/*` | answered by the user's account runtime in the front | **410**, with one sentence naming the tray and the phone apps |
| `/api/files/account/libraries*` | the origin's libraries | the same |
| `/api/files-hub` (the hello) | a ticket for the local user | a ticket, which the libraries' admin calls carry |
| an account runtime | one, for the local user | **none**, whoever signs in |

The 410 is answered before the ticket or the session is read: a server has nothing to hide behind it.
`$STUDIO_FILES_ROOT` is still required, since the libraries live in it.

## An account's runtime is the front's own, on a device

The hub runs no process. The local user's files are answered by that account's runtime inside the front
(`../service.md`), made by its first request or by the hub's own warm-up, with the account's folder as its
`$STUDIO_HOME`. Nothing starts node for an account: a host that still names `K7_NODE` or a service entry
(`STUDIO_FILE_SERVICE_ENTRY`, `STUDIO_LIBRARY_SERVICE_ENTRY`, `STUDIO_MAX_FILE_SERVICES`,
`STUDIO_FILES_WATCH`) starts all the same and the front says once that it no longer reads them.

One person and no account competing for memory: the runtime is never reaped.
`State::is_device` is the one test: `warm` does nothing on a server, so a sign-in there makes no runtime.

An account's folder is `accountKey(email)` (`k7_hub::account`): the readable slug **and** a hash
of the exact email, since a slug alone folds `a.b@` and `a_b@` into one folder.

## A server's cut resolves in the libraries' context

The médiathèque's stem cut and an engine's slots ask the hub for a chain (`POST /api/internal/files/cold-run`). On a device that is the
account's runtime, with its chain setting and, for an admin, the libraries' drafts. On a server it is `Services::cut`, a
context of the libraries alone (`$STUDIO_FILES_ROOT/_cut`, no prefs, published versions only), and the roots the
engine's machine is shown are the médiathèque (writable, for the stems), the libraries and the model's own folder.

| choice | why |
|---|---|
| the account's `stemsChain` and `instrumentalChain` do not apply on a server; a cut names its chain, else the engine's | an account's own chains are local to each device and never synced (decision 3106 of `docs/webdav-removal-brief.md`) |
| no admin draft overlay on a cut | a draft reaches no cut until it is published |
| the account's workspace and models are no root of a cut | a server holds no account runtime to resolve a project against |
| the engine slots' compile (`/api/internal/files/slot`) takes the same context on a server: a chain of the libraries, shared with the machine as `libs` and the médiathèque, both read-only | a slot of an account's own chain is a device's |

### The environment is what the host adds, not the front's

An account's environment is `k7_hub::env`'s allow-list taken from the process and the host's own settings,
then the account's values on top (`k7_hub::account::Names`). The front's environment holds the origin's
secrets — the session and token signing keys, the OIDC client secret, the render key — and an account's
runtime needs none of them. The hazard runs
one way: a variable a program starts reading and nobody adds to the list is silently unset there.

### `/api/ready` keeps node's figures

The readiness reports `running`, `starting`, `cap`, `oldestStartMs` and `lastStartError` because the image's
healthcheck and the conformance suite read that shape. `running` is the accounts' runtimes held now; nothing
starts out of process, so the other figures are fixed (`starting` and `oldestStartMs` zero, no failed start,
`cap` the largest figure the shape ever held). A full hub is no longer a state, and a start can no longer be stuck.

## A ticket says who is calling

The browser's identity headers are **dropped**, not trusted: the account served is the ticket's or the
session's, whatever `x-studio-user`, the hub secret or the engine token a client sends. On a server shared
by several accounts, trusting those would let anyone open anyone's workspace.

Who it is comes from a **ticket**: `/api/files-hub`, behind the session, answers one — the
account's email and an expiry, signed with a key derived from the session secret, so a deploy does
not invalidate an open page. The browser sends it in the header the files gate reads, which spares every
call a cookie across origins; `/api/files` itself has no session guard, the ticket being its gate, with
the session as a fallback for a same-origin caller. Twelve hours, and the browser asks again after six.

**Whether the caller is an admin rides the same way.** The front checks the session's OIDC `groups`
against `AUTH_ADMIN_GROUPS`, and an empty list admits nobody. The hello answers `admin`, the ticket
signs it, and the front takes the flag from the ticket or the session alone. It guards what reaches every
account at once: the libraries list and every write into a library (`../libraries.md`), which follow the
stamp request by request, so a new session's ticket makes an account an admin with no restart. A ticket
outlives a group change by up to twelve hours, the same as it outlives the session.

## A refusal on the headers alone reaches the caller

A request is answered by the account's runtime as soon as its headers are in, and its body streams behind
them, never buffered. An upload declaring more than the ceiling is a 413 in words before a byte is read, and
a client that declares a length and waits for that answer sends nothing until it comes: a hub that read the
body first would never answer it.

## The libraries are the front's own

The origin's list of libraries and their folders are one runtime for the whole origin, never an account's, made
when the hub launches and always kept (`../libraries.md`). Without a code root, or when it could not be made, the
libraries answer a 503 saying so, and a standalone's writes wait for its first `ensure` to settle, or 45 s
(`../files.md`, "Ready").

## An engine is bound by a CLAIM

A request naming an engine (`x-studio-engine-id`) binds the account to it only when that account
**holds a claim** on it in the registry (`k7_hub::binding`). The registry lists every engine to
everyone, and a header is not permission; the claim is (`../engine/claim.md`).
A binding that fails leaves the files reachable and says why in the log.

The binding is the ACCOUNT's, held by the hub (`Bindings`), not by the runtime serving it: the runtime
is told it before each request (`../service.md`). Its two loopback paths are kept under their keys while
the account has a runtime and re-established when the tunnel hub restarts.

A device's bound engine sees exactly five roots of the account: its `workspace/` folder, writable, so a take
comes back; its render cache `.renders/`, writable, so an automatic VST render comes back
(`packages/song/docs/autoFreeze.md`); its undo cache `.history/`, writable, since the machine KEEPS
an undo's audio and sends it back up for an undo (`files/history.md`); its
`models/`, read-only; and the instance's data folder, read-only.
**Never the rest of `$STUDIO_HOME`**, which holds the
account's own state: an engine's machine may be a shared studio box.

A binding is two loopback paths to the same machine: the engine's, and its **jobs** — where every
separation the account asks for runs, since cutting stems belongs to the engine's machine
(`packages/stem-separation/docs/stemSplit.md`). The account's `models/` is shared too, read-only,
so a model downloaded once here reaches that machine once. A cut also shares the folder of the model it resolved when no root holds it
(`$STUDIO_MODELS` moves it off `models/`), read-only: the machine refuses a path outside its copies.

The engine's plugin list is fetched over the tunnel at binding time and written as
`<home>/exts.json`, the scan file's own shape, so everything that reads the engine's catalogue
works unchanged. When the engine answers that its scan is still running — it scans at every
start — the hub keeps the channel open, for fifteen minutes at most, and writes the list again
from the one the scan ends on (`engine/k7-runtime/docs/plugins.md`). A rescan asked later reaches
the server at the next binding, or through the account's runtime when a project waits for it. Both writers leave a copy that already says the same
untouched, since its mtime is in every project's revision token.

## An engine serves one account at a time

An engine is shared, never concurrently: the account holding its claim drives it. When the claim
moves on, the previous account **lets go** of the engine at its next request, before
anything else, so a tab left open cannot push its project into an engine someone else is playing.
Each account's copy on the engine's machine stays in its own folder, `ws-<accountKey>`, and is
simply there again when that account comes back. The libraries travel beside it, read-only, as
one root, `libs`: the libraries' folders, the same for every account. A module's files the engine
loads — an amp's model, its samples — live in one, and no workspace holds a copy
(`../libraries.md`).

## What stays on the engine's machine

What drives or needs the machine's own hardware, for every account alike: its audio devices, its
plugins, the separations its CPU runs, and playing a song on its speakers. No file runtime runs
there: the server asks for that work as jobs over the tunnel (`packages/engine-tunnel/docs/jobs.md`).
