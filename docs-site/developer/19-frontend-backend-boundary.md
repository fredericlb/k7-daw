# Frontend and backend boundary

The k7 editor in the browser talks to three servers, and each request has exactly one right destination. Getting it wrong shows up as a 401 that explains nothing or as a file op that silently lands on the wrong machine.

| | k7 backend | account file service | engine |
|---|---|---|---|
| Code | `apps/k7/backend-rs` (`k7-front`) | `apps/k7/backend-rs/crates/k7-file-service`, held by the Rust front | `engine/k7-runtime` |
| Runs | the device itself | in the Rust front, one runtime for the local account | the engine's machine |
| Owns | `/auth/*`, the device's preferences, AI settings and engine registry | the account's workspaces: every file op, the change set the verbs commit, compile, revision, undo, conflicts | playback, transport, meters, live params |
| Reached | same origin, `/api/*` and `/auth/*` | through the backend's hub, at the base URL the engine probe returns; never directly | WebSocket, direct when allowed, else relayed through the origin |
| Auth | the `k7.sid` cookie | a hub ticket in headers (`<ticket>~<engineId>`), minted from the token; the cookie is never sent | engine token |

## In the frontend

`apps/k7/frontend/src/api.ts` is the one typed client:

- auth: `/auth/me` — spelled at the root, since k7 is the service with no prefix;
- `engines` (`engineAccess` from `@studio/engine-browser/access`): the registry, probe, lease and relay, plus the account preferences (`prefs`, `patchPrefs`) at k7's own root;
- `api.*`: the file service, built on `@studio/project-shell`'s `projectApi` and `fget`/`fsend`, whose base and headers are set once by the gate;
- `aiOrganize`: `/api/ai/*`.

The history (`src/syncHistory.ts`: list, checkpoints, diff, restore) goes to the account's **file service** (`/history/*`), which keeps each workspace's ordered log on the device.

## The file service routes the shell uses most

All in the route table `apps/k7/backend-rs/crates/k7-front/src/routes/files/service/routes.rs`:

| Route | For |
|---|---|
| `GET /workspace` | The workspace, the active project, the last take, engine sync and load progress (polled) |
| `POST /open` | Hand a project to the engine (the ▶ in the tree) |
| `GET /tree`, `GET/PUT /file`, `/folder`, `/rename`, `/move`, `/entry/delete` | The file tree and editor |
| `POST /changes`, `GET /verb-bundle`, `/verb-host/*` | What the verbs Worker commits and reads ([the commands package](./12-commands-package.md)) |
| `GET /revision` | The change token every view keys its reloads on |
| `GET /preferences`, `PUT /preferences/vim` | The **machine's** preferences |

## Two kinds of preferences

| Kept by | Holds | Why there |
|---|---|---|
| k7 backend `/api/preferences` (per account) | the remembered engine id, command prefs (names, icons, keys, menus, order), playback target | follows the person to every browser |
| k7 backend `/api/ai/settings` (per account) | Claude key and model | the key must be write-only from the browser |
| file service `/preferences` (per machine) | vim mode, audio device, channel labels | belongs to the rig, not the person |
| browser `localStorage` | interface language, recent commands, viewport window placement, profiling switch | per-browser conveniences |

## Sessions and tickets

The hub mints the file service's ticket from the session; it ages (twelve hours), and `fileApi.ts` renews it once on a 401 through `engines.freshTicket()`. The engine WebSocket URL is a function rather than a string because a relayed URL carries a one-minute ticket. Logging out anywhere logs out of every studio app, and the backend being down means nobody can sign in to any studio app.

## Where to add something

- A new **verb** on project files: `packages/commands`, never a new route ([Adding a command](./13-adding-a-command.md)).
- A new **file-shaped** read the views need: a route in the Rust file service (`apps/k7/backend-rs/crates/k7-front/src/routes/files/service/`), a function in `@studio/project-shell` if an app outside this repository needs it too, else in `apps/k7/frontend/src/api.ts`.
- A new **account-level** setting: k7's front (`prefs_schema.rs`) and `K7Prefs` in `api.ts`, together.
- Anything about **live audio**: the engine protocol (Engines section).
