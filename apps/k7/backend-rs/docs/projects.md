# Projects — the backend holds none

**The backend knows nothing about git.** It keeps no list of an account's repositories, no deploy
key and no libraries pointer: all three belong to the machine that holds the working copies.

| question | who answers |
|---|---|
| which projects does this account have | the file service, from the workspaces on its machine — a project is a folder there |
| how is a project addressed | `/@local/<espace de travail>/<projet>`, naming that folder (`packages/project-shell/docs/launch.md`) |
| the device's libraries | the file hub's libraries' service: `GET /account/libraries`, one list beside the account folders, frozen to the bundled seed (`apps/k7/backend-rs/docs/libraries.md`) |

## Why the file service, and not the backend

A register here would have to be consulted before a project could open, would give an address that
only resolves while a server answers, and would send a private key between two services to make
either work. With the machine holding all of it, a machine with no server is not a degraded one: an
imported repository is the same thing as a project made locally — a folder, with an `origin` if it
has one. `apps/k7/frontend/docs/projects.md` is the page for the screen that lists them.

## The account's service starts before its first list

A file service is a node process per account, and its cold start — parsing the bundle, then
readying its workspaces — is what the first list of projects would otherwise wait for. So it is
started EARLIER, at a moment that already says a list is coming:

- **On a phone or the desktop app, the boot.** The machine has one person, the local user, whose service
  starts once the front listens — and is never stopped for being idle (no `idle` in
  `apps/k7/backend-rs/crates/k7-front/src/routes/files/hub/spawn.rs`): reaping only frees memory for another account, and there is none, while
  a screen left off for ten minutes would otherwise pay a cold start on the next open.

