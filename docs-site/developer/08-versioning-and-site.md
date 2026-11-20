# Versioning and the static site

k7 runs on devices, so this repository deploys no server. `k7.rocks` is a static site built from it and
served by GitHub Pages: a front page, and this documentation under `k7.rocks/docs`.

## The site

`.github/workflows/pages.yml` runs `node scripts/buildSite.mjs dist-site` and deploys the result with
`actions/upload-pages-artifact` and `actions/deploy-pages`. The script builds the documentation as its own Vite
bundle (`apps/k7-daw-frontend/vite.docs.config.ts`, base `/docs/`), copies `site/` (the front page) beside it, writes an
`index.html` for every documentation page, a `404.html`, a `CNAME` of `k7.rocks` and a `.nojekyll`.

| Piece | Where |
|---|---|
| the front page: the stores and the release page | `site/index.html` |
| the documentation bundle | `apps/k7-daw-frontend/docs-entry/`, `src/docs/` |
| the pages, user and developer guides | `docs-site/` |
| the assembly and the workflow | `scripts/buildSite.mjs`, `.github/workflows/pages.yml` |

Mobile apps come from the stores, and desktop builds from the repository's GitHub release page: no app updates itself and
no server publishes a build. `docs/site.md` has the steps that turn Pages on for the repository and point the domain at it.

## The version and the stamp

The release version is the root `package.json`'s `version` (for example `0.1.0`), copied into every
private workspace's `package.json`; `npm run lint:version` (`scripts/check-version.mjs`) checks the
copies and `node scripts/check-version.mjs --set X.Y.Z` writes them all. See `docs/versioning.md`.

Every build takes its version from the root `package.json`. `apps/k7-daw-frontend/src/version.ts` exports `VERSION`,
and the About section of the app settings shows it (`apps/k7-daw-frontend/src/about/AboutSection.tsx`);
`/api/health` answers `{ ok, version, build }`, both the front's.

The front serves the frontend shell with `Cache-Control: no-cache` and the hashed files under
`assets/` as `public, max-age=31536000, immutable`, so a new build is picked up on the next page
load. A request for a hashed asset that no longer exists returns 404 rather than the shell.

Engine, desktop and phone release files are named for the same version: see [Releases and publishing](./09-releases-and-publishing.md).
