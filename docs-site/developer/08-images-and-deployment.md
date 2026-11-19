# Images, deployment and versioning

The server side of k7 ships as one Docker image built from this repository, run by
`docker/docker-compose.yml` behind an external Traefik. k7 itself runs on devices, so the image only
serves `k7.rocks` (the root site, the downloads page and the update feeds) and `docs.k7.rocks` (this
documentation).

| Image | Dockerfile | Registry name |
|---|---|---|
| k7 web app: the backend and the built frontend | `apps/k7/docker/Dockerfile` | `dev.frediver.se/frediverse/k7/k7-webapp` |

It builds with the repository root as context:

```bash
docker build -f apps/k7/docker/Dockerfile --build-arg APP_VERSION=dev .
```

`.dockerignore` keeps `node_modules`, `dist`, `.git`, `.claude`, `env.yaml`, logs, Storybook output
and the native build folders (`engine/k7-runtime/build`, `engine/k7-supervisor/build`) out of the
build context.

## The k7 web app image

The stages:

| Stage | Base | Does |
|---|---|---|
| `manifests` | `node:24-alpine` | Copies the repository and extracts only every `package.json`, `package-lock.json` and `.npmrc`, so the install layers depend on manifests alone. |
| `toolchain` | `node:24-alpine` | Adds `python3`, `make`, `g++` (for `ssh2`'s optional native crypto binding) and the manifests. |
| `deps` | `toolchain` | `npm ci` of the whole workspace, with an npm cache mount. |
| `frontend-build` | `deps` | Copies the sources, sets `VITE_APP_VERSION` from the `APP_VERSION` build argument, runs `vite build` for `k7-frontend`. No typecheck here: CI typechecks separately. |
| `backend-deps` | `toolchain` | `npm ci --workspace k7-backend --omit=dev`: the backend's production dependency closure only. |
| `runtime` | `node:24-alpine` | Adds `ffmpeg`; copies the backend's `node_modules`, **the whole repository**, and the built frontend into `apps/k7/frontend/dist`. |

The whole repository is copied because the `@studio/*` entries in
`node_modules` are symlinks to workspace source folders, consumed as TypeScript: a package left out
is a crash on first import.

Runtime settings:

| Setting | Value |
|---|---|
| Command | `tsx src/server.ts` in `apps/k7/backend` |
| `PORT` | `8789` (exposed) |
| Volumes | `/data` (`STUDIO_PREFS_DIR=/data/prefs`), `/library` (`STUDIO_MEDIATEK`), `/workspaces` |

The image starts from a plain Node image and has no nginx: the backend serves
the SPA itself.

## The compose file

`docker/docker-compose.yml` runs two services on an external `traefik` network: `k7` and `autoheal`,
which restarts `k7` when its healthcheck says unhealthy. Copy `docker/.env.example` to `docker/.env`,
fill it in, and run `docker compose up -d`; later deploys go through `docker/deploy.sh <APP_VERSION>`.

| Variable | Purpose |
|---|---|
| `SESSION_SECRET` | Required: compose refuses to start without it. At least 32 characters. |
| `OIDC_ISSUER`, `OIDC_CLIENT_ID` | Required, though nobody signs in on this host: the front refuses to start without them. |
| `APP_RELEASE_TOKEN` | The bearer a developer publishes the apps' builds with; empty refuses every upload. |
| `DOCS_DOMAIN`, `SITE_DOMAIN`, `TRAEFIK_CERTRESOLVER` | The two Traefik routers: the documentation host, and the root domain with `www.`. |

The `data` volume holds the published builds and the prefs.

## The version and the stamp

The release version is the root `package.json`'s `version` (for example `0.1.0`), copied into every
private workspace's `package.json`; `npm run lint:version` (`scripts/check-version.mjs`) checks the
copies and `node scripts/check-version.mjs --set X.Y.Z` writes them all. See `docs/versioning.md`.

The image's stamp is computed as `YY.MMDDHHmm` in the Europe/Paris time
zone, for example `26.10161432`. It is:

- passed as `--build-arg APP_VERSION=<stamp>` to the image, which sets `VITE_APP_VERSION` for the
  frontend build and `APP_VERSION` in the runtime stage. `apps/k7/frontend/src/version.ts` exports
  `VERSION` as `<version> (<stamp>)` (`"dev"` for the stamp when unset), and the home sidebar shows
  it as the tooltip of the `k7` wordmark (`apps/k7/frontend/src/projects/HomeSidebar.tsx`);
  `/api/health` answers `{ ok, version, build }`;
- a tag the `publish` job adds to every image.

Images are first pushed under the commit hash only. The stamp and `:latest` are added once
`k7.yml` has passed for that commit (and the publish fails if it never ran), so nothing deployable
points at an untested commit. A `vX.Y.Z` tag must equal the root version; its run adds the stamp
and `:X.Y.Z` and leaves `:latest` to `main`. A deploy sets `APP_VERSION` in `docker/.env` to a
version, a stamp or a commit hash — compose refuses to start without it — and it applies to the image.

The backend serves the frontend shell with `Cache-Control: no-cache` and the hashed files under
`assets/` as `public, max-age=31536000, immutable`, so a new deployment is picked up on the next page
load. A request for a hashed asset that no longer exists returns 404 rather than the shell.

Engine, tray and phone releases use a different stamp: see [Releases and publishing](./09-releases-and-publishing.md).
