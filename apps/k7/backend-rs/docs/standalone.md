# k7's front on the user's own machine

Three apps run the front on the user's device rather than on a server: the tray on a computer
(`apps/k7-tray/docs/tray.md`), the phone (`engine/k7-runtime/docs/android.md`) and the iPhone
(`engine/k7-runtime/docs/ios.md`). All three start it from the **same table**, so they cannot drift apart.

## The table: `standalone.json`

The port, the environment and the state folders, read by the tray (`apps/k7-tray/src-tauri/src/backend.rs`), by
`BackendHost.kt` (the phone's, through a `BuildConfig` constant for the port and the copy inside its
`backend.zip` for the rest) and by `BackendHost.swift`. What each host adds is only what is its own: the state
root, the session secret it keeps there, `$STUDIO_ENGINE` where an engine binary exists, and the page's folder.

| value | why |
|---|---|
| `port` 18789 | FIXED, on the computer as on the phone, because a page's storage is keyed on its origin, port included |
| no `OIDC_*` | a device signs no one in: the front starts with `STUDIO_LOCAL_USER=1` and no provider |
| `STUDIO_LOCAL_USER=1` | signed out is a local user, never a dead end: nothing needs a network to start |
| no `COOKIE_DOMAIN` | a domain would make the session cookie `Secure`, and the origin is plain http on loopback |
| `dirs` | the folders under the host's state root, named once so a backup or a support answer reads the same on every host |
| `library` | the `k7-core` folder the bundle carries, handed to the hub as its seed (`STUDIO_LIBRARY_SEED`): the local user is never an admin and could not add one. `scripts/stageBundle.mjs` fills it with the repo's `k7-core`, and an app update is how a library changes, so a new install starts on a real version number (`scripts/libraryArchive.mjs`) |

## Ready means the library is there

**On a standalone the front holds the paths that create workspaces until the hub's first library
ensure has settled**: `/api/files-hub` and `/api/files` answer 503 `k7 is starting: its libraries are
being installed` with `Retry-After: 1` until the libraries' service says its first `ensure` settled, or
45 s have passed (`files.md`, "The libraries' service"). The front itself answers at once: it holds no
library of its own. A workspace created before the bundled `k7-core` is installed has
no library to resolve its refs against, and the local user is no admin who could add one; so the copy
gates the workspaces, and neither a person nor a test reaches "create a workspace" before it.

| case | what happens |
|---|---|
| the copy fails | the libraries' service logs why; creating a workspace answers 503 with that reason, not the admin message (`crates/k7-libraries/src/lib.rs`, `libraryRefusal`). Everything that needs no new workspace still works |
| the copy is still running after 45 s | the gate opens anyway, under the hosts' own 60 s for a first answer; a workspace asked for meanwhile WAITS, since its request queues behind the copy in the libraries' service |

## It answers its own machine only

`/auth/local` hands a session to whoever asks, so the front must be unreachable by anyone but the
person at the machine — a venue's Wi-Fi is full of strangers. Three locks, each for a different way in:

| lock | what it stops |
|---|---|
| each host binds `127.0.0.1` (`Config.listen`) instead of every interface | a device on the same network opening `http://<ip>:<port>/auth/local` |
| the loopback lock (`src/web.rs`) on every route while `STUDIO_LOCAL_USER=1`: the peer address AND the `Host` header must be loopback | DNS rebinding: a page whose name resolves to `127.0.0.1` reaches the port from the user's own browser, and only its `Host` gives it away |
| `/auth/local` checks both again (`src/routes/auth/local.rs`) | the same, should a host ever start the local user without the other two |

Nothing on another device needs this front. A device on the network drives the **engine**, on
the engine's own port and once paired (`engine/k7-runtime/docs/pairing.md`). The QR codes the page
draws rebase onto `lanUrls`, which a machine's own front leaves empty (`apps/k7/frontend/src/App.tsx`), so they show no code rather than
a link into this port (`apps/k7/frontend/docs/shell.md`); serving k7's pages to a tablet from here
would mean lifting all three locks, and a login of its own for the tablet. The supervisor, the file
services and the page all reach the front on `127.0.0.1`. The locks cover HTTP routes; a
WebSocket upgrade is answered by its own listener and is that listener's to check.

## The bundle: `scripts/stageBundle.mjs`

What a device's front reads beside itself: the built frontend at its repo path (`apps/k7/frontend/dist`, named to the front as
`K7_FRONTEND_DIST`) and the `k7-core` archive. The front's code root (the song's `meta/`, `commands.yaml`, the demos and the
video sample) is `scripts/stageCodeRoot.mjs`'s, staged beside it.

| choice | why |
|---|---|
| a folder, not a JavaScript bundle | the front is Rust: it needs files to read, no runtime to run them |
| `--no-frontend`, and `K7_DEV_FRONTEND` at run time | the dev tray's page is `k7-frontend`'s Vite, streamed by the front for HMR on 18789 (`routes/last/dev_frontend.rs`, `docs/dev.md`); a release always carries the built one, since nothing sets the variable there |
