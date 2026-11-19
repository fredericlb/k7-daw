# Versioning

What a build is called, and how that name reaches the screen. Two names answer two questions:
**which release is this?** — the VERSION, `0.1.0`, one for the whole repository — and **which
build of it?** — the STAMP, `snapshot-9f2c1ab` or the tag itself for a tray, engine or phone build.

## One version, in the root package.json

The root `package.json`'s `version` is the release version of everything this repository ships:
the tray, the command-line engine, the APK, the iPhone app. A `vX.Y.Z` tag is a release of exactly that version, and `scripts/check-version.mjs --tag`
refuses any other tag. So a release is two steps: `node scripts/check-version.mjs --set X.Y.Z`
in a commit on `main`, then the tag on that commit.

| choice | why |
|---|---|
| the root package.json, not a `VERSION` file | npm already requires a `version` in every manifest, so a separate file would be one more copy, not one fewer; node, Gradle and Tauri all read JSON |
| every private workspace carries the same version | a manifest must hold some version, and a stale one is read as a claim; `npm run lint:version`, a blocking step of `k7.yml`, refuses a copy that differs |
| the four published packages keep their own | their consumers outside this repository pin them by semver, and their version moves with their API, not with a release ([publishing.md](publishing.md)) |
| `--set` writes every copy, the lockfile and the iOS project included | a bump is one command; nobody edits sixty manifests by hand |

Each artefact derives its name from it:

| artefact | release | snapshot |
|---|---|---|
| `/api/health` | `version` (the backend's package.json copy) and `build` (`$APP_VERSION`, `dev` when unset) | same |
| the SPA's tooltip | `0.1.0 (dev)`, the stamp being `$VITE_APP_VERSION` when a build sets it | same |
| engine and supervisor `build` | `v0.1.0` | `0.1.0-snapshot-9f2c1ab`; `0.1.0-dev` locally |
| tray bundle version | `0.1.0` | `0.1.0-snapshot-9f2c1ab`, a semver prerelease |
| Android `versionName` / iOS `MARKETING_VERSION` | `0.1.0` | `0.1.0-snapshot-9f2c1ab` / `0.1.0` |
| Android `versionCode` / iOS build number | the commit count | the commit count |

**The phone's build number is the commit count** (`git rev-list --count HEAD`). Android refuses an
update whose `versionCode` is lower than the installed one, so the number must grow with every
build that may replace another. A count of ancestors grows with every descendant commit, is the
same for a commit wherever it is built, and needs no counter stored anywhere; a CI run number would
differ between the workflow that built a snapshot and the one that built its release. The cost is
a full history: the jobs that build a phone app check out with `fetch-depth: 0` and no blobs, and
Gradle refuses a shallow clone rather than count one commit.

## The stamp in the page

`apps/k7/frontend/src/version.ts` reads `import.meta.env.VITE_APP_VERSION ?? "dev"`, and the front's
`/api/health` reads `$APP_VERSION` the same way. `??` and not `||`: an explicitly empty
`VITE_APP_VERSION=` renders as an empty string rather than `dev`, which is a real if harmless edge. The
variable keeps the `VITE_` prefix, which is Vite's default `envPrefix`: a name without it is not inlined, and the build
ships `dev` with no warning. Both layers exist so the stamp can never render `undefined`.

## Where it shows

| app | SPA | render site |
|---|---|---|
| k7 | `k7-frontend` | `apps/k7/frontend/src/projects/HomeSidebar.tsx` — tooltip on the sidebar's title, on every home page |

`VERSION` is read at module scope so it constant-folds into the bundle. In k7's home sidebar the version and the stamp are a
tooltip on the app's name and never visible text: nobody needs it in the moment, and it is one hover
away for whoever asks which build this is.

## The shell is asked again on every load, its assets never

The front serves the built `frontend/dist` (`apps/k7/backend-rs/docs/last.md`), with two
cache rules. `index.html` — and every other file outside `assets/`, the manifest and the icons —
goes out `no-cache`: a browser may keep it, but asks the server before each use, so the first load
after an update is the new build. Files under `assets/` go out `immutable` for a year: Vite names
each one after a hash of its content, so a name never comes back with other bytes, and a new
`index.html` names the new ones. Express's default (`max-age=0`) would have had the browser
re-check every chunk on every load for nothing.

`no-cache` and not `max-age=0`: the two read the same until the server is unreachable, where some
browsers serve a stale `max-age=0` copy and a `no-cache` one is not served unchecked.

A **service worker** is the other cache, and it ignores these headers for what it precached. k7
registers one (`apps/k7/frontend/src/appMain.tsx`, `packages/vite-config/docs/pwa.md` for how
an update reaches an open tab). An app on an iPhone's home screen can still be RESUMED
from memory without any request at all; closing it in the app switcher is what makes it load.
