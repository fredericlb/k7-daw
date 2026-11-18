# Deploying the front

The deployment is **hosted**: k7's deployed server owns the accounts' working copies. It is not a
service of its own: the routes are k7's Rust front's, in the same container as the backend
that serves the built SPA, with no nginx (`apps/k7/docker/Dockerfile`, `docs/versioning.md`).
Three things belong to that image.

## 1. It is stateful. The volume IS the state.

The container owns everyone's tokens under the instance's data folder, and, under `/workspaces`, the
accounts' working copies (`apps/k7/backend-rs/docs/files/hub.md`).

`VOLUME ["/library"]` is declared so a `docker run` with no `-v` still gets an anonymous
volume instead of writing into the container layer, where a token would survive exactly
until the next deploy — and would look like it had worked. The image sets
`STUDIO_MEDIATEK=/library`, and `STUDIO_FILES_ROOT` is required at boot
([config.md](../../backend/docs/config.md)).

There is **no database**: what the front keeps is files, one per person.

## 2. It answers two hosts, and one of them only for its API

The front answers on k7's own host, `/api/*` for the routes.
`play.st0.app` is routed to the same container and is **permanent**: every installed
supervisor has `https://play.st0.app` saved as its registry, and every installed supervisor names it. On
that host `/api/*` and `/auth/*` answer as on k7's, and any other path is a 301 to the home
at k7's origin, `app.k7.rocks` (`apps/k7/backend/src/server.ts`).
`packages/web-protocol/docs/addresses.md` is the page for the address side.

## 3. It moves large files, so nothing in front may buffer or cut it

The front streams in both directions itself: a file goes out answering `206` to every Range request
a player makes, and an upload comes in straight to a file on the
volume. A large
upload over a slow link is a long request doing exactly what it should. Whatever fronts the container — Traefik here —
must stream rather than spool, and must not cut a long request short, or the big files are the
ones that fail and the front's own limit and message never run.

The engine relay is a WebSocket on the same process
(`apps/k7/backend-rs/docs/engine/proxy.md`), under `/api/engine/<id>/ws`. A driving socket is idle
for long stretches by design, so the edge must not time an idle WebSocket out either. Its HTTP
half — `/api/engine/<id>/files` — is an ordinary request; what it needs is that **nothing consumes
the body before it**.

## What the runtime stage must carry

`ffmpeg` for the files the waveform module cannot open
(`packages/audio-analysis/docs/analysis.md`), and `git`, `git-lfs` and `openssh-client` for the file hub's clones.

## What cannot be checked from a dev machine

The image build itself: CI is where it runs.

The assumption most likely to be wrong IS checkable here: the server boots with nothing but
`STUDIO_MEDIATEK`, `STUDIO_FILES_ROOT` and `PORT`, on two empty directories.

And on the deployed side: `SESSION_SECRET` and the `OIDC_*` triple must be set, or the service
comes up in **dev auth mode with a stub 'Dev User'** — it says so on stderr, and on a public
origin that line is the difference between a library and a free-for-all.
