# The static site, k7.rocks

`k7.rocks` is a static site built from this repository and served by GitHub Pages: a front page, and the
documentation under `k7.rocks/docs`. No server runs: k7 itself runs on devices, mobile apps come from the
stores, and desktop builds are attached to the GitHub release page.
A self-hosted Forgejo can serve the same build instead, with git-pages ([forgejo-pages.md](forgejo-pages.md)); the DNS
records decide which host answers.

## Choices

| choice | why |
|---|---|
| GitHub Pages, built by `.github/workflows/pages.yml` on a push to `main` that touches the site | the site changes only with the docs, and a host that is free and has nothing to patch or watch keeps no server to retire |
| the front page is one static file, `site/index.html`, with the splash drawing and three links | the page says where to get k7; it fetches nothing and needs no build |
| the documentation is its own Vite bundle, `apps/k7-daw-frontend/vite.docs.config.ts`, base `/docs/` | the editor's bundle never carries the guides and the guides never load the editor (`apps/k7-daw-frontend/docs/docsSite.md`) |
| `scripts/buildSite.mjs` writes an `index.html` for every route, and a `404.html` | Pages has no SPA fallback: a deep link needs its own file, and a mistyped one needs an answer that is the app's own "No page here" |
| `CNAME` and `.nojekyll` are written by the build | the custom domain survives every deploy, and Jekyll would otherwise drop the `_`-prefixed files |
| no download page and no update feed | the stores update the phone apps, and a person fetches a desktop build from the release page: no app asks a server whether it is current |
| `docs.k7.rocks` does not exist | one host, one certificate; the guides live at `k7.rocks/docs` |

## Building it

```bash
node scripts/buildSite.mjs dist-site   # the front page, then the guides, into dist-site/
```

The script builds the bundle with `npx vite build --config vite.docs.config.ts` from `apps/k7-daw-frontend`, so the
repository's dependencies must be installed. Serve `dist-site/` from any static file server to read it.

## Turning it on

These steps belong to the owner of the repository and the domain; nothing in the workflow performs them.

1. Repository settings, Pages: set the source to **GitHub Actions**.
2. Run the `pages` workflow once (Actions, `pages`, Run workflow), or push a change to the site on `main`.
3. Point `k7.rocks` at GitHub Pages: an `A` and `AAAA` set of records for the apex (`185.199.108.153`,
   `185.199.109.153`, `185.199.110.153`, `185.199.111.153`; `2606:50c0:8000::153` to `2606:50c0:8003::153`)
   and a `CNAME` of `www` to `<owner>.github.io`. GitHub's own custom-domain page holds the current list.
4. In Pages settings, name `k7.rocks` as the custom domain and turn on **Enforce HTTPS** once the certificate is issued.
5. Retire the old server and its `docs.k7.rocks` records.
