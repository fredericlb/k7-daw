# k7 served by a machine's own backend

The same bundle is served by K7 DAW on a computer (`http://127.0.0.1:18789`), shown in its window and in the browser, and by the phone apps;
the hosted image serves only the downloads and docs sites. On the desktop app's backend, the page is the whole app: there is no shell around it, no
preload, and the browser's address bar is right there. What differs is asked of the backend, never of a build
flag (`src/localApp.ts`).

| choice | why |
|---|---|
| `GET /api/local`, answered by a machine's own backend only | the backend knows what it is; the answer also says whether it runs the engine and which release the desktop app ships |
| the page holds the backend's token, handed over by `GET /auth/me` (`AuthGate`) | the machine works signed out, one account folder holds its workspaces, and a page on another site gets no token (`apps/k7/backend-rs/docs/sessions.md`) |
| the user menu's heading is the identity's name from Settings, "Local" while none is set (`src/projects/menuAccount.ts`) | the machine's own user is nobody's login, so the person names themselves once, for the commits and the menu |
| the engine page over HTTP (`src/settings/desktopEngine.ts`) | k7's front runs the supervisor (`apps/k7/backend-rs/docs/localEngine.md`); the page reads its snapshot as events and sends allowed commands |
| the engine list is this machine's own registry's | a project plays on the engine of the machine that holds it: a device's engine is listed at no server |

The service worker the hosted build registers serves in a browser on the desktop app's backend too: it makes the page installable, and the
backend being on the same machine is what makes it work offline. A phone app's view and K7 DAW's window (`window.k7Desktop`, `src/appShell.ts`) register none
(`src/appMain.tsx`) and drop any they hold: its files already sit on the device, served from loopback, and a
precache would copy them to flash again at every first start and every update, while the app starts. The
phone shell is what is installed, so nothing is lost.
