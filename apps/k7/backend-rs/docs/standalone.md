# k7's backend on the user's own machine

Two apps run this backend on the user's device rather than on a server: the tray on a computer
(`apps/k7-tray/docs/tray.md`) and the phone (`engine/k7-runtime/docs/android.md`). Both
run the **same bundle** and start it from the **same table**, so the two cannot drift apart.

## The table: `standalone.json`

The port, the environment and the state folders, read by the tray (`apps/k7-tray/src-tauri/src/backend.rs`) and by
`BackendHost.kt` (the phone's, through a `BuildConfig` constant for the port and the copy inside its
`backend.zip` for the rest). What each host adds is only what is its own: the state root, the
session secret it keeps there, `$STUDIO_ENGINE` where an engine binary exists, and the file
service's entry.

| value | why |
|---|---|
| `port` 18789 | FIXED, on the computer as on the phone, because a page's storage is keyed on its origin, port included |
| no `OIDC_*` | a device signs no one in: the front starts with `STUDIO_LOCAL_USER=1` and no provider, which `k7-local`'s `front.rs` runs |
| `STUDIO_LOCAL_USER=1` | signed out is a local user, never a dead end: nothing needs a network to start |
| `STUDIO_LISTEN_HOST` `127.0.0.1` | the backend answers its own machine only — below |
| no `COOKIE_DOMAIN` | a domain would make the session cookie `Secure`, and the origin is plain http on loopback |
| `dirs` | the folders under the host's state root, named once so a backup or a support answer reads the same on both |
| `library` | the `k7-core` folder the bundle carries, handed to the hub as its seed (`STUDIO_LIBRARY_SEED`): the local user is never an admin and could not add one. `bundle.mjs` fills it with the repo's `k7-core`, and an app update is how a library changes, so a new install starts on a real version number (`../../backend-rs/scripts/libraryArchive.mjs`) |

## Ready means the library is there

**On a standalone the front holds the paths that create workspaces until the hub's first library
ensure has settled**: `/api/files-hub` and `/api/files` answer 503 `k7 is starting: its libraries are
being installed` with `Retry-After: 1` until the libraries' service says its first `ensure` settled, or
45 s have passed (`apps/k7/backend-rs/docs/files.md`, "The libraries' service"). The backend itself
listens at once: it holds no library. A workspace created before the bundled `k7-core` is installed has
no library to resolve its refs against, and the local user is no admin who could add one; so the copy
gates the workspaces, and neither a person nor a test reaches "create a workspace" before it.

| case | what happens |
|---|---|
| the copy fails | the libraries' service logs why; creating a workspace answers 503 with that reason, not the admin message (`apps/k7/backend-rs/crates/k7-libraries/src/lib.rs`, `libraryRefusal`). Everything that needs no new workspace still works |
| the copy is still running after 45 s | the gate opens anyway, under the hosts' own 60 s for a first answer; a workspace asked for meanwhile WAITS, since its request queues behind the copy in the libraries' service |

## It answers its own machine only

`/auth/local` hands a session to whoever asks, so this backend must be unreachable by anyone but the
person at the machine — a venue's Wi-Fi is full of strangers. Three locks, each for a different way in:

| lock | what it stops |
|---|---|
| `STUDIO_LISTEN_HOST=127.0.0.1` — `server.ts` binds there instead of every interface | a device on the same network opening `http://<ip>:<port>/auth/local` |
| `loopbackOnly` (`src/loopback.ts`) on every route while `STUDIO_LOCAL_USER=1`: the peer address AND the `Host` header must be loopback | DNS rebinding: a page whose name resolves to `127.0.0.1` reaches the port from the user's own browser, and only its `Host` gives it away |
| `/auth/local` checks both again (the Rust front's `src/routes/auth/local.rs`) | the same, should a host ever start the local user without the other two |

Nothing on another device needs this backend. A device on the network drives the **engine**, on
the engine's own port and once paired (`engine/k7-runtime/docs/pairing.md`). The QR codes the page
draws rebase onto `lanUrls`, which a machine's own backend leaves empty (`apps/k7/frontend/src/App.tsx`), so they show no code rather than
a link into this port (`apps/k7/frontend/docs/shell.md`); serving k7's pages to a tablet from here
would mean lifting all three locks, and a login of its own for the tablet. The supervisor, the file
services and the page all reach this backend on `127.0.0.1`. The locks cover HTTP routes; a
WebSocket upgrade is answered by its own listener and is that listener's to check.

## Memory after the start

Nothing forces a collection once the start is over. The start allocates far more than it keeps (the
library's capture, the parse cache, the first requests), and V8's memory reducer hands that heap back
on its own once allocation stops: measured on a desktop, about 280 MB private down to 110 MB within 10
to 20 s of the first answer. A `gc()` forced at 20 s, through `--expose-gc` set at run time, came after
it and changed nothing.

## The bundle: `scripts/bundle.mjs`

The backend, one ESM file, plus the files they read at run time —
the built frontend, the song's `meta/`, `commands.yaml`, the demos, the video sample — at their
REPO paths. A source's `import.meta` is rewritten to its original path
under that tree, so `new URL("../meta", import.meta.url)` finds the same file it finds in a checkout.
A data file a source starts reading is added to `DATA`; `--list` prints every source the rewrite touched.

| choice | why |
|---|---|
| a bundle, not source under tsx | tsx needs esbuild's native binary, which the phone's node cannot load; and source with its `node_modules` weighs about 150 MB against an 8 MB bundle |
| `cpu-features` and `*.node` left out | ssh2's native helpers, optional: it falls back to JS, and no addon could target both the tray's node and Termux's |
| `playwright-core` left out | only the server's picture render reaches it, through a lazy `import()`, and it does not bundle: its optional `chromium-bidi` imports are not installed. The server image has it in `node_modules`; the tray and the phone render no picture |
| `node24` | the node the tray pins and ships beside it (`apps/k7-tray/scripts/stage.mjs`) |
| every non-ASCII character escaped after the build (`asciiOnly`) | V8 holds a script's source for as long as it runs, and one non-ASCII character anywhere makes it hold all of it as UTF-16: the backend's 8.7 MB was 17 MB of heap. esbuild escapes strings by itself, but not regex literals or the comments it keeps; a `\uXXXX` means the same character in both |
| `--no-frontend`, and `K7_DEV_FRONTEND` at run time | the dev tray's page is `k7-frontend`'s Vite, streamed by the backend for HMR on 18789 (`../src/devFrontend.ts`, `../../../../docs/dev.md`); a release always carries the built one, since nothing sets the variable there |
