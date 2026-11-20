# The machine's engine, run by its own backend

Under the desktop app (`apps/k7-daw-desktop`), k7's front runs on the person's computer and runs that computer's
supervisor, and so its engine (`apps/k7-daw-backend/packages/k7-local`, routed by `apps/k7-daw-backend/packages/k7-front/src/routes/local/`). It is
off everywhere else: the desktop app turns it on (`K7_LOCAL_ENGINE=front`, the supervisor named by
`K7_SUPERVISOR_BIN`); a server never runs one, and a phone's app hosts its own (`supervisorHost.md`).
How the front keeps these promises is `local.md`.

| choice | why |
|---|---|
| the front hosts the supervisor, over stdio | the supervisor listens on no port, so no page in the browser can reach it (`apps/k7-daw-engine/k7-supervisor/docs/hostProtocol.md`); k7's pages reach it through the front's routes |
| `/api/local/engine` behind the session, on a device's own backend only | such a backend answers loopback only and refuses cross-site writes (`standalone.md`); the session is what any other route of it takes |
| an allow-list of commands | a page starts, stops and restarts the engine, ranks the audio interfaces, rescans plugins, fetches or drops instruments, lists a registry or drops it, and enables, disables or removes a slot; `shutdown`, the pairing verbs and plugin windows are not the page's |
| a slot saved through `POST /api/local/engine/slots`, staged in a file the supervisor takes (`slot.save`) | a compiled chain is too big for a one-line command, and on this machine the supervisor reads the same disk (`apps/k7-daw-engine/k7-supervisor/docs/slots.md`) |
| removing a server on the engine page unlists the engine from that registry alone | the account keeps syncing with it; removing the server in the settings lets go of the sync |
| the snapshot pushed as Server-Sent Events | a whole picture every time, as the supervisor emits it; the browser reconnects by itself across a restart |
| this machine's own k7 pinned first in the supervisor's `engine-host.json` before each start (`apps/k7-daw-backend/packages/k7-local/src/settings.rs`) | `$STUDIO_REGISTRY_URL` counts only while the file lists no registry, and the page finds the engine in this machine's registry |
| every other loopback registry dropped when pinning | the supervisor reaches any loopback registry at k7's one port, which `STUDIO_PORTS` makes this front's (`apps/k7-daw-engine/k7-supervisor/src/Url.cpp`): another loopback entry is this front spelt otherwise, and two hold two tunnels that displace each other. A second k7 on the machine (the dev session's) is listed by its public name |
| the engine opens the machine's own folders by path (`STUDIO_ENGINE_ROOTS`) | a project here is a folder on this disk; nothing is copied through the tunnel |
| the supervisor stopped before the rest | it deregisters from this front, which must still be up to hear it |
| this registry lists this machine's engine alone | the account's other machines play their own projects; none of them heartbeats here or at a server |
| `/api/local` answered on every device, Android included, with `engine: false` where the front runs no engine | its answering at all is how a page knows it is on a machine's own backend (`apps/k7-daw-frontend/docs/localApp.md`), and `engine` whether that backend drives the machine's engine |

Under the desktop app the front is linked into the desktop app's process, so the desktop app's quit is the front's stop,
which stops the supervisor first (`local.md`).
