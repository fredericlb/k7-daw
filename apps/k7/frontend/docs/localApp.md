# k7 served by a machine's own backend

The same bundle is served by the tray on a computer (`http://127.0.0.1:18789`) and by the phone apps;
the hosted image serves only the downloads and docs sites. On the tray's backend, the page is the whole app: there is no shell around it, no
preload, and the browser's address bar is right there. What differs is asked of the backend, never of a build
flag (`src/localApp.ts`).

| choice | why |
|---|---|
| `GET /api/local`, answered by a machine's own backend only | the backend knows what it is; the answer also says whether it runs the engine and which release the tray ships |
| the machine's session is ALWAYS its local user (`AuthGate` → `/auth/local`; the backend turns any other into it) | the machine works signed out, and one account folder holds its workspaces |
| the user menu's heading is the identity's name from Settings, "Local" while none is set (`src/projects/menuAccount.ts`) | the machine's own user is nobody's login, so the person names themselves once, for the commits and the menu |
| the engine page over HTTP (`src/settings/desktopEngine.ts`) | k7's front runs the supervisor (`apps/k7/backend-rs/docs/localEngine.md`); the page reads its snapshot as events and sends allowed commands |
| the engine list is this machine's own registry's | a project plays on the engine of the machine that holds it: a device's engine is listed at no server |

The service worker the hosted build registers serves on the tray too: it makes the page installable, and the
backend being on the same machine is what makes it work offline. A phone app's view registers none
(`src/appMain.tsx`) and drops any it holds: its files already sit on the device, served from loopback, and a
precache would copy them to flash again at every first start and every update, while the app starts. The
phone shell is what is installed, so nothing is lost.
