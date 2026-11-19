# The address of a project

`projects.ts` states two contracts: which studio app owns a folder, decided by the folder's NAME,
and the address that names one, `/@local/<workspace>/<project>`. Both sit between the side that
WRITES an address — k7's home page — and the apps that read one back, which is why they are stated
once, browser-safe, and nowhere else. See `apps/k7/frontend/docs/projects.md` and
`packages/project-shell/docs/launch.md`.

## Where each service answers

`services.ts` states where every service is, derived from ONE origin, k7's. k7 itself runs on devices, at their own loopback address, so no host serves the app: the `app.` rows are the names the code derives, and the docs host is the one the image serves:

| service | where | dev | production |
|---|---|---|---|
| k7 | the origin itself | `app.priv.k7.rocks` | `app.k7.rocks` |
| pl8 | k7's backend routes (the tokens), served BY k7 — k7's origin | `app.priv.k7.rocks` | `app.k7.rocks` |
| docs | k7's documentation, served BY k7 on a host of its own | `docs.priv.k7.rocks` | `docs.k7.rocks` |

`serviceUrl(origin, service)` answers from any studio URL — k7's or a sibling's, whose label it
strips first — so a page never spells a host, and a deployment on a domain of its own needs nothing
configured. On a bare loopback host (`localhost`, a dev run) services are told apart by PORT instead.

An app outside this repository reads this table through the published package
(`docs/publishing.md`).

What you land on is your own WORK: the editor's home page, at k7's origin, lists the workspaces,
the projects and the libraries. The
workspaces, the libraries and the deploy key are the file service's.

### k7 is `app.`, and its documentation is `docs.` beside it

k7's origin carries the `app.` label so the documentation can have a host of its own on the same
domain. `docs.<domain>` is routed to k7's container and served the same SPA, which, seeing that
host (`isDocsHost`), mounts the help page alone: no gate, no engine, no session
(`apps/k7/frontend/docs/docsSite.md`). Every service resolved from it is k7's: `docs.` reads as
`app.` in `serviceUrl`, so the page's Back lands on the app's origin.

### The former hosts — `st0.app`, `play.` and `k7.` — are permanent

`st0.app` is k7's former origin, `play.st0.app` a former host, and `k7.st0.app`
another former host of k7. All three are still routed to k7's container, because things
outside this repo's reach name them and cannot be rewritten:

- every installed supervisor has `https://play.st0.app` saved as its registry URL;
- every link sent and every PWA installed from `k7.` names it.

`serviceUrl` reads each of them as the current origin: the `play.` and `k7.` labels are stripped
(`ALIAS_LABELS`), and what is left is looked up in `LEGACY_HOSTS`, which names each former host's
successor — so a supervisor's saved registry heartbeats `app.k7.rocks` directly, and so does the
supervisor's `Url.cpp`, its twin. Dropping one of them is a breakage of those links, not a cleanup.

A label is stripped only when a real domain is left behind it: `k7.rocks` is a domain, not the
`k7.` alias of `rocks`. The labels stay pattern-based otherwise, so a deployment on a domain of
its own gets the same aliases with nothing configured.

### The ports are the other half of the same table

A SERVER calling another studio service uses `SERVICE_PORTS` rather than the public URL, and the
reason is not performance: in a checkout everything is behind a private CA, whose local CA node
does not trust, so `https://app.priv.k7.rocks` fails with `UNABLE_TO_GET_ISSUER_CERT` from
a backend while working perfectly in the browser beside it.

On a loopback host the same table is what a PAGE uses too, and a host may move it:
`STUDIO_PORTS=k7=18789` (the tray publishing the port `apps/k7/backend-rs/standalone.json` fixes) in the environment of the front and the supervisor, whose
`Url.cpp` reads the same variable — or `globalThis.__STUDIO_PORTS__` for a page, set before its
scripts run (`servicePorts`). `pl8` and `k7` have no port of their own: they are k7's. The tray's port is apart from the dev session's, so the two run side by side
(`apps/k7-tray/docs/tray.md`). Unset, the table is `SERVICE_PORTS`.

## One suffix, and every shape inside it

`PROJECT_SUFFIXES` has one entry, `.k7`, because one app opens a project. Everything an k7 project
can BE is a shape *inside* `.k7` rather than a folder kind of its own: a song carries
picture lanes and a `viewport` surface, a set carries `songs.k7x` and a `turntables` surface
(`PROJECT_SHAPES` in `@studio/song`). See `packages/song-video/docs/picture.md` and
`packages/song/docs/setProject.md`. It stays a `Record<ProjectApp, string>`, so an app added to
`ProjectApp` fails to compile until its suffix is stated.

`projectApp` reads the NAME, not the contents, and the two are different questions: "who opens
this" and "can the engine be given it". Only the second needs a file opened, which is why the
first can be answered without a checkout at all. A `project.k7x` is what makes a project
COMPILE, and `@studio/song` asks that question its own way, on the machine's own clone.

k7 IS its service, so a project's address has no mount in front of it:
`<origin>/@local/<workspace>/<project>`.

## Parsing one back

k7 is mounted at the ORIGIN ROOT, so its `base` is `/` and the sentinel is the FIRST path segment.
`/api` and `/auth` are claimed by k7's backend; neither can be mistaken for a sentinel, since
every sentinel starts with `@`, so nothing collides.

**Where the project ends** is decided by the folder's own name, the same rule as `projectApp`: the
project is everything up to and including the LAST segment claiming THIS app. That is what lets a
project sit at a depth (`songs/winter.k7`), lets the app's own route follow it
(`…/winter.k7/timeline`), and lets a nested set be addressed by its own app without k7's suffix
winning. With no such segment the workspace itself is the project (`"."`) and everything after it
is the app's route — a view segment never ends in a project suffix, so the two cannot be confused.

`base` is where the app is mounted, as vite's `BASE_URL` spells it. An address outside it names
something else: null.

### `projectBasename` is a SLICE, never a rebuild

`projectBasename` is what keeps the launch out of tab-local storage. React Router only ever
writes BELOW its basename, so the project cannot be rewritten away by an app's own `routes.ts` —
which is the single thing such a memory would be for. A reload re-reads the same path, a bookmark
works, and a wrong project is fixed by editing the address instead of by clearing storage nobody
can see. It falls back to `base` when the address names no project, which leaves the app mounted
where it is and lets its gate send you to k7's home page.

React Router compares the string against `window.location.pathname` **verbatim** and renders
NOTHING when it does not prefix it. So any re-encoding here is a chance to disagree with the
address the browser actually holds, over a character with two legal spellings: `%40local` and
`@local` are the same address, and this function must answer with whichever one it was handed —
which slicing does by construction and spelling it out again cannot.

`encodeSegment` leaves `@` alone for the same family of reasons. `encodeURIComponent` escapes it
and RFC 3986 does not: `@` is a `pchar`, legal literally in a path segment, and a browser keeps it
that way in the address bar. Encoding it produces an address that does not match the one the
person is looking at — and the sentinels this file mints are exactly the names that would hit it.

## The three sentinels

`@` cannot begin a folder name git gives a clone, so a first segment starting with it can never
collide with a workspace. Three sentinels use it, and they are the only addresses a gate opens.

### `@local` — a project in a workspace on this machine

`/@local/<workspace>/<projet>.k7` names a folder the machine already holds: nothing to look up and
nothing to clone. Every project on k7's home page is linked this way — `Examples`, seeded with the
demos the first time a person is seen, a workspace made there with no repository, and a clone
brought in with **Importer** alike.

`parseLaunch` needs nothing added. `/@local/Examples/synth-demo.k7` reads as repo `@local`, project
`Examples/synth-demo.k7`: the project part ends at the last segment claiming the app, exactly as
everywhere else. `localTarget` is what splits the workspace off the front of it — a workspace that
IS a project has nothing after its name and the project is `"."`.

The price, stated where it is paid: this address names a machine's folder, so it is **not
portable** — a link pasted to somebody names a folder in yours. What it buys is that no service has
to answer before a project opens (`apps/k7/backend-rs/docs/projects.md`).

### `@banc` — the bench

The bench has no repository and wants none. It is generated, and deleted and generated again
whenever it is opened, so putting it under version control would version something with a lifetime
of one session. `ProjectGate` does no lookup and no clone: one call to the machine, which makes the
folder.

It answers **one** project, `live.k7`, preceded by the library chain's ref the bench is ON —
`/@banc/vocals/warm/live.k7` (or `/@banc/@k7-core/vocals/warm/live.k7`) seeds the rig with a copy of
that chain. The bare `/@banc/live.k7` and
a bare library ref, `/@banc/@k7-core/live.k7`, are refused by the service: a library is not edited on
a bench but by an admin from any project. `benchChainOf` is the whole rule — spelled
once, in this package, so the browser that builds the link and the service that reads it cannot
drift. Anything else under `@banc` names nothing and is a 404 whose sentence says where a chain and
a module are edited instead (`apps/k7/frontend/docs/sharedModules.md`).

The project part is safe to join onto a root, because it never is: the folder is the constant
`live.k7` whatever the address says, and the chain ref is only ever MATCHED against the chains the
library holds. A ref is folder names — no empty, dotted or `.k7` segment — so a traversal is not
even a candidate.

## One builder, one claim key

`localProjectUrl` and `externalUrl` both go through `launchUrl` rather than spelling the
address out beside it: these addresses are read back by `parseLaunch` and `projectBasename`, and a
second builder is a second place for the encoding to differ. `launchUrl` keeps only the origin of
what it is handed, so a caller can pass `window.location.href` without stripping anything first.

`projectClaimKey` is `<sentinel>/<projet>` — for `@local`, the workspace and the folder inside it —
stated here and not in each gate: two surfaces that spell one project differently would each
believe they had it, and the rule "one engine per project" would hold for neither. No app prefix,
because which app opens a folder is decided by its SUFFIX, so two folders with different suffixes
are already two different strings.

## A surface, opened on its own

A surface is a screen the project declares, and some of them are things you stand away from the
computer to use — a desk you want on a tablet by the decks, not on the laptop behind them. So
every surface has an address of its own, `<project>/@external/<surface>`, and opening it on another
device gives that surface and nothing else, driving the same engine as the tab that has the
project open.

`EXTERNAL_SEGMENT` sits BELOW the project, not beside it, and that is what makes it cost nothing:
`parseLaunch` ends a project at the last segment claiming the app's suffix, so everything after is
already the app's own route. The project parses exactly as it always does, the router's basename is
unchanged, and a reload or a pasted link re-reads the address. The `@` cannot be a surface id
either (`createSurfaceFile` allows letters, digits, `_` and `-` and nothing else), so the sentinel
can never collide with the thing it sits next to.

`parseExternal` reads it the same way `parseLaunch` reads the project — off the address and
nothing else — so the two cannot disagree about what a tab is on. It returns null for a bare
`@external` with no name: that is not "some surface", it is an address somebody truncated.

`externalOf` exists for the caller that did not CHOOSE the address: a lister has already built the
project's link (`projectHref`) by the time it wants the surface's. One encoder either way, which is
the point of it being here rather than a template string at the call site.

### `/external`, the menu

The page a second device opens first and, installed as a PWA, the one it starts on: every surface
this account can drive, across every project on the machine. It needs no project in the address,
which is why it sits at the app's mount rather than under one. `isExternalIndex` matches it —
exactly one segment, `external`, after the mount — and `main.tsx` reads it before `parseLaunch`.
Since every project address starts with a sentinel, the plain word collides with nothing.

`/libraries`, `/settings` and `/about` are more such pages — k7's list of the origin's libraries,
its settings, and its version, licence and third-party notices — reserved for the same reason:
none opens a project. `isHomePage` matches them, and every address BELOW one too —
`/settings/engine` is a route of the home like `/settings`, and reloading it or following a link
to it must land there rather than be read as a workspace. Taking the whole prefix costs nothing:
a project opens only under a sentinel (`@local`, `@banc`), so no bare word below the
mount is a workspace, and `isProjectAddress` already reads the head alone. `isHomePage` is read
BEFORE `parseLaunch`, which would otherwise take the word for a workspace and the
router would mount under it. Once loaded they are ROUTES of k7's home, the router's basename
being the mount, so going between the home and any of them reloads nothing. `/external` stays a
branch because it swaps the page's manifest before anything mounts.
