# The k7 image — why its Dockerfile is staged the way it is

One image is k7 — the editor —, `dev.frediver.se/frediverse/k7/k7-webapp`. Its Dockerfile builds
from the repo root (`apps/k7/docker/Dockerfile`) and pays for two expensive things: an `npm ci`
over the workspace, and the C toolchain `ssh2` builds its optional crypto binding with.

The server's engine is a second image, `dev.frediver.se/frediverse/k7/k7-runtime`, apart
because it compiles C++ (`docs/server-engine.md`), and the Raspberry Pi's a third,
`k7-runtime-pi` (`docs/pi-engine.md`).

## What the image serves

The image is the server of two public hosts and nothing else: `k7.rocks` (the root site, `/app_downloads`
and the update feeds the tray and the Android app read, `docs/app-downloads.md`) and `docs.k7.rocks`
(the documentation, `apps/k7/frontend/docs/docsSite.md`). k7 itself runs on devices, so the image
answers no sign-in, no account's files, no share and no application page.

| choice | why |
|---|---|
| the front, the Express backend and the compose file stay as they are | the publishing of builds and feeds works unchanged (`APP_RELEASE_TOKEN`), and the image is the one place the hosts are served from |
| the Express `@studio/oidc` sign-in code stays in the backend | it is deferred code, listed in `docs/inventaire.md` (D2); the compose file's `OIDC_*` variables feed it |

## One Node version: 24

The image's stages, GitHub's checks (through `.nvmrc`), OneDev's and the bundles' esbuild target all
name Node 24, the active LTS: a test run on one version and a server on another is a bug found in
production. Its `node:sqlite`, the shares' comments store, needs no `--experimental-sqlite`, and
it has an alpine image. A move to the next LTS changes every `node:24-*` image tag, `.nvmrc`, the
root `engines` and `bundle.mjs`'s `target` in the same commit.

## An install depends on the manifests, never on the sources

The stage that runs `npm ci` sees the lockfile, `.npmrc` and every `package.json`, and nothing
else: the `manifests` stage cuts the build context down to those files. BuildKit keys a
`COPY` on file content, not on timestamps. So a push that touches no manifest finds every install
layer in the cache, and only the frontend build and the final `COPY . .` run.

The hazard this avoids: with `COPY . .` before `npm ci`, any edit to any file invalidates
the install, and every deploy pays for 1,200 packages and their native builds.

## `deps` installs the whole workspace

`manifests`, `toolchain` and `deps` are the first three stages, and `deps` installs the WHOLE
workspace rather than the frontend's closure: one install layer serves the frontend build and
stays valid whatever the frontend imports next.

The production install (`backend-deps`) is separate, because the runtime image ships only its
backend's closure. It still sees only the manifests, so it is cached across deploys like the others.

## The server runs a bundle, not source under tsx

The `frontend-build` stage also bundles the backend
(`apps/k7/backend/scripts/bundle.mjs server-dist --in-tree`), and the image runs
`node /app/server-dist/backend.mjs`. Under tsx, the process transpiled the whole package
graph on start and kept an esbuild child beside it: that cost was paid at every deploy.

`--in-tree` writes only the two entries, into a folder of the checkout, so each source's
`import.meta` points at the files the image already copies (`frontend/dist`, `song/meta`,
`commands.yaml`). The checkout stays in the image for that reason. Its target is node 24, like the desktop and phone
bundles.

`NODE_COMPILE_CACHE` points node's compile cache at `/home/node/.cache`, as the tray and the phone
do (`apps/k7-tray/docs/tray.md`): the bytecode of the bundle is compiled by the first start and read by every later one.
The folder is in the container's own layer, not on a volume: the cache is keyed by each file's
contents and node's version, so a new image could not use the old one's, and a volume would only
pile them up. A `dsp` child starts with an empty environment and so without it.

## tini is PID 1, in the image

The front answers every account's files, and it starts ffmpeg for a video or an
mp3, the engine for a render and chromium for a picture. A front that dies before its children — killed past its stop
grace — leaves them re-parented to PID 1. Node never reaps a child it did
not spawn, so with node as PID 1 each one becomes a zombie, and one still running burns CPU for
nobody. So the image's `ENTRYPOINT` is `tini`, which reaps every orphan and forwards the stop
signal; the `CMD` is `start.sh`, which passes that signal to node and to the front (below). It is in the
image and not `init: true` in the compose file because the image runs elsewhere too — a bare
`docker run`, another orchestrator — and an init that only one deployment supplies is one the
others lack. tini only reaps: killing what a dead front left running is not its job. The server's
engine image runs under tini for the same reason (`engine/docker/Dockerfile`).

## Two processes: the Rust front, and node behind it

The image runs k7's Rust front (`apps/k7/backend-rs/docs/front.md`) on 8789, the port Traefik and the server's
engine reach, and node's backend behind it on `127.0.0.1:8790`. `apps/k7/docker/start.sh` starts both under tini.

| choice | why |
|---|---|
| one container, not a sidecar | the two share a secret and a loopback; a second container would put the backend on a network every other service can reach, and a deployment could run one without the other |
| either process ending ends the container | the restart policy then brings both back together, as it brought node back alone; a front left alone would answer 502 until the healthcheck noticed |
| the one that ended is said, with its code or the signal that killed it (`start.sh: k7-front exited …`) | the front says its own panics and stops (`apps/k7/backend-rs/docs/front.md`), but a kill leaves only the status its parent reads |
| the stop is trapped and passed to both, then waited for | the front stops the programs it started (stdin closed, then SIGTERM, SIGKILL past 6 s); a shell that exited at once would end the container under them |
| the secret drawn at each start, unless `K7_FRONT_SECRET` is set | only these two processes need it, and a restart has no session to keep |
| `K7_ENGINE_CORE=front` and `K7_FILES=front`, whatever the deployment sets | the registry, the relay, the tunnel hub (`apps/k7/backend-rs/docs/engine.md`) and the file hub (`apps/k7/backend-rs/docs/files.md`) exist in Rust alone; their settings (`STUDIO_MEDIATEK`) reach the front from the container's environment, and the way back is the previous image |
| the front runs all fifteen native pieces answering; the image holds no engine, so `engine` and `runtime` degrade to the no-engine answers | one release runs every piece, and no variable switches one off |
| every account's file routes are the front's own (`apps/k7/backend-rs/docs/service.md`), the libraries too (`apps/k7/backend-rs/docs/libraries.md`); no node process starts | the image runs no JavaScript for an account |
| `K7_LAST_ROUTES=front` and `K7_FRONTEND_DIST=/app/apps/k7/frontend/dist`, whatever the deployment sets | node's last routes are the front's (`apps/k7/backend-rs/docs/last.md`), the page served from the image's built frontend; node keeps running behind the front until the image starts the front alone. The rollback is the previous image |
| the bundle is `backend.mjs` alone, and the image runs `backend.mjs` and `k7-front` alone: no node file service, no libraries' service, an image of some 2.21 GB | the relay's rooms, the blobs and the history are files under `STUDIO_FILES_ROOT`, the front their one writer |
| the front's compiled modules are in `/home/node/.cache/k7-front` (`K7_FRONT_CACHE`) | a compiled module is machine code, so its folder is the container user's own and on no volume |
| the front's code root is its own folder, `/usr/local/share/k7/code` (`K7_CODE_ROOT`), staged by `apps/k7/backend-rs/scripts/stageCodeRoot.mjs` in the frontend build, some 700 KB | the front's file service reads `@meta`, `commands.yaml`, the demos and the sample video there; the checkout copied into `/app` goes when node does (phase 6.6), this folder does not |
| `K7_FRONT_TRUST_HOPS=1` | Traefik is the one proxy before the front; the engine on the compose network sends no forwarded headers, so its own address is what is read. A container sharing Traefik's network can still name any non-loopback client address, which moves only the rate limits and the same-network listing: ownership and "this machine" read the socket (`apps/k7/backend-rs/docs/front.md`). A deployment that knows Traefik's address sets `K7_FRONT_TRUSTED_PROXIES` to it |
| a static musl binary from `rust:alpine`, built from `apps/k7/backend-rs` alone with `--locked` | the runtime is alpine, and a change anywhere else in the repo rebuilds nothing of it |

## The Rust front is built with thin LTO

`apps/k7/backend-rs`'s release profile is `lto = "thin"` at cargo's default 16 codegen units, as the
tray's is, not fat LTO at one unit. One unit makes every crate a single thread, and fat LTO adds a
serial pass over the whole binary — Wasmtime's Cranelift, chromiumoxide — at the
end of the build. What it changes is the COLD build, the one a new toolchain, a dependency bump or an
emptied cache mount pays: on twelve threads it is under half. With the dependencies cached, OneDev's
image build takes about as long either way (15 min 10 s fat, alone on the host; 16 min 22 s thin,
beside one `lint and test`), so the job's hour is lost to other jobs sharing the host, not to the profile.

| measured on Windows 11, 12 threads, `cargo build --release -p k7-front --timings` | fat, 1 unit | thin, 16 units |
|---|---|---|
| cold build | 38 min 55 s | 16 min 31 s |
| k7-front's own crate, then its binary and the LTO (what an image build with the cached dependencies pays) | 758 s + 1015 s | 167 s + 347 s |
| `k7-front.exe` | 52.5 MB | 59.5 MB |

The cost is the size, and Rust code that thin LTO inlines less across crates (not measured). The hot paths are not that code: a module run in Wasmtime is Cranelift's
output, not rustc's.

## A readiness healthcheck, memory limits and capped logs

`HEALTHCHECK` asks `/api/ready`, answered only on loopback. It is ready when each volume the
environment names takes a one-byte write: a full or read-only disk is a fault that persists until
somebody acts. The report keeps the shape the healthcheck and the conformance suite read, and
no account's runtime is out of process, so no start can be stuck (`apps/k7/backend-rs/docs/files/hub.md`).
`/api/health` stays as the liveness probe: no session, no disk, only that the event loop answers.
The server's engine waits for the healthcheck (`service_healthy`) rather than dialling a k7 still starting.

`docker/docker-compose.yml` caps k7 and the engine with separate `mem_limit`s, so a render or an
import on k7 cannot starve the engine's null sink. Inside k7's limit the front, the backend and the
programs the front starts share one budget, and an OOM kill there takes the whole container
down. Every service's json-file log is rotated (`LOG_MAX_SIZE`, `LOG_MAX_FILE`).

## `--mount=type=cache` keeps the npm cache when a manifest does change

A lockfile change invalidates the install layers, but the `/root/.npm` cache mount survives
between builds, so `npm ci` reinstalls from the local cache instead of the registry.

## The layers are kept in the registry, not only in the builder

The build exports its cache to `k7-cache:k7-webapp` (`mode=max`, so the install
stages are in it, not only the final image) and imports it on the next run. A builder that
starts empty, or one whose cache was pruned, still finds the installs. `image-manifest=true`
stores the cache as an ordinary OCI image, which a registry that refuses BuildKit's own cache
manifest still accepts.

## An image build checks no types

The frontends are built with `vite build` alone, not with their `build` script, whose second half
is `tsc`. `k7.yml` runs that `build` script, so a type error is still red;
the image build just doesn't find it a second time.

## No CI job builds or publishes an image

An image is built locally from its Dockerfile when a deployment needs one, and tagged by hand
with the version stamp (`docs/versioning.md`); `k7.yml` only checks the code.

## The engine compiles only on a source change

The engine's compile lives in cache mounts, which a registry cache does not carry and a fresh runner
starts without: a push that changes no input of the compile reuses the compiled layer whole, and one
that does compiles cold. So the engine Dockerfiles read a context of their own,
`engine/docker/Dockerfile.dockerignore` and `Dockerfile.pi.dockerignore`, which BuildKit takes in
place of the root `.dockerignore`: every `docs/` folder and every `.md` is left out, since the
engine folders' notes change about as often as their sources and each such push would otherwise
compile the whole engine for nothing. The two files repeat the root's build-output exclusions, which
they replace.

## The deployment pulls from OneDev's registry, logged in

`docker/docker-compose.yml` names `dev.frediver.se/frediverse/k7/k7-webapp` and `…/k7-runtime`,
the images pushed to OneDev's registry. That registry answers nobody anonymously, so the deploy host signs in once,
`docker login dev.frediver.se`, as a user who may read the project, with an access token for a
password; the login is kept in the host's Docker config and every later pull uses it.

## Deploying, and rolling back

A deploy is `docker/deploy.sh <APP_VERSION>`, run on the host beside the compose file and its `.env`.
It writes the version into `.env`, pulls k7 and the engine, restarts the stack, and waits up to
`READY_TIMEOUT` (180 s) for k7's `/api/ready`. If k7 does not get there, it writes the previous
`APP_VERSION` back, restarts on it, and exits non-zero with k7's last log lines.

| choice | why |
|---|---|
| `/api/ready`, not "the container is up" | a container that runs but cannot write its volumes, or whose file hub is wedged, serves errors — and with them `/app_downloads`, the apps' update feed, so a bad deploy also stalls every app's next update |
| the previous version kept in `.env`, the one file the stack reads it from | the rollback is the same `up -d` as the deploy; nothing to remember beside the files already there |
| `up -d --remove-orphans` on a deploy | a service the compose file stops naming leaves no container running beside the stack; its volumes stay, since a removed part's data is kept on the host |
| the images only, never the volumes | a build may rewrite what it stores when it starts, and nothing promises an older build reads it back; a rollback past such a change needs the volumes' backup, not a tag |

**Rolling back by hand**, when the script is not the one that deployed, or a problem shows only later:
read `docker compose ps` and k7's log, set `APP_VERSION` in `.env` to the last version that worked (the
script prints it as `was:`; `docker image ls dev.frediver.se/frediverse/k7/k7-webapp` lists those still on the host),
then `docker compose up -d` and `docker compose exec -T k7 wget -qO- http://127.0.0.1:8789/api/ready`.
Or simply `./deploy.sh <that version>`, which checks and reverts the same way.

**Between deploys**, `autoheal` (`willfarrell/autoheal`, pinned) restarts a container labelled
`autoheal=true` once its healthcheck says unhealthy: Docker's own `restart:` acts on an exit only, so a
backend wedged but alive stays wedged. Only k7 carries the label, the engine having no healthcheck; it
reaches the Docker socket, which is root on the host, and has no network.

## The process runs as `node`, and its volumes must be `node`'s

The runtime stage ends on `USER node`: the code under `/app` stays root's, so a flaw in a route
cannot rewrite the server it runs in. What the process writes is its three volumes — `/data`,
`/library`, `/workspaces` — which the image creates owned by `node` before
declaring them, so a FRESH volume inherits that owner. A volume that already holds files owned by
root keeps them: it needs a one-time `chown -R 1000:1000` of its contents (for instance
`docker run --rm -v <volume>:/v alpine chown -R 1000:1000 /v`) before this image can write to it.

