# The documentation site

`k7.rocks/docs` is the documentation SITE: the user guide and the developer guide in `docs-site/` at the
repository root, readable by someone who has no account and no engine. It is a static bundle on GitHub
Pages (`docs/site.md` at the repository root); no server answers it.

## A bundle of its own

The site is its own Vite entry, not a mode of the app: `docs-entry/index.html` loads `src/docs/mountSite.tsx`, and
`vite.docs.config.ts` builds it with `base: "/docs/"`, no service worker and no public folder. The editor's
bundle never carries the documentation, and the documentation never loads the editor. `src/docs/site/pages.ts`
globs `docs-site/` at build time, so the site needs no server of its own and no copy of the pages.

The page mounts no `AuthGate`, no router and no engine gate. Its one link out is to the front page of the
site, `k7.rocks`, where the stores and the release page are listed.

## Base-aware addresses

The pages route on the pathname themselves, with no router. A route is a path inside `docs-site/`;
`hrefOf` puts the base in front of it and `routeOfPath` takes it off, so the same code serves `/docs/user/…`
on Pages and `/user/…` under a base of `/`. Every link a page renders goes through `hrefOf`.

GitHub Pages answers a path it has no file for with its `404.html`, not with the app, so
`scripts/buildSite.mjs` writes an `index.html` for every route and copies the entry to `404.html`: a deep
link opens, and a mistyped one lands on the app's own "No page here".

## Not the in-app help

The `help` command opens `HelpPage`, which renders `apps/k7/docs/` — the commands, the file
formats, the generated command reference — beside an open project. The site serves the
guides instead: they cover the whole of k7 for a reader who is not inside it yet, and the in-app
page's command reference comes from a file service the site has none of.

## Pages are addressed by their file

A page's address is its path in `docs-site/` without `.md`, and a guide's `00-index.md` is the
guide's own address (`/user`, `/developer`). The files link each other with relative `.md` links,
so they read the same on GitHub; the site rewrites those links into addresses as it renders, and
gives every heading GitHub's anchor id, which is what their `#…` links were written against.

A guide's sidebar is read off its `00-index.md` — each `##` a group, each list link an entry — so
adding a page means listing it there, and there is no second manifest to forget.

`src/docs/site/pages.test.ts` follows every link between the pages and every anchor, and counts
the pages, since an empty glob would otherwise leave the site blank and the link check vacuous.
