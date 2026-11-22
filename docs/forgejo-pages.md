# k7.rocks from a self-hosted Forgejo

`k7.rocks` can be served by a Forgejo server of our own instead of GitHub Pages: the same site, built by the same
script, published by Forgejo Actions to **git-pages**, which serves it. GitHub Pages stays as it is
([site.md](site.md)); the two run side by side, and the DNS records decide which one answers.

The pieces in this repository:

- `.forgejo/workflows/pages.yml`: the site's build (`node scripts/buildSite.mjs dist-site`, as in
  `.github/workflows/pages.yml`), then the upload to git-pages. Forgejo reads `.forgejo/workflows/` and ignores
  `.github/workflows/` when the first exists; GitHub never reads `.forgejo/`.
- `deploy/forgejo/`: the server kit, `compose.yaml`, `Caddyfile` and `.env.example`.

## Choices

| choice | why |
|---|---|
| git-pages serves the site | it is the static server Forgejo's own Pages are built on; Forgejo itself serves no site |
| Forgejo Actions publishes it, with the `actions/git-pages` action pinned to a commit | Forgejo 17 has no git-pages webhook, and Actions is on by default; the action uploads the built folder, so the build stays the one GitHub runs |
| the job's own token, accepted through a DNS record | git-pages lets a repository publish on a domain whose `_git-pages-forge-allowlist` TXT record names that repository, checking the token's push right with Forgejo; no secret is stored anywhere |
| the site's address is the repository variable `PAGES_SITE`, `https://k7.rocks/` when unset | a trial server publishes to its own domain from the same workflow file |
| Caddy in front, with Let's Encrypt | one process holds every certificate and renews it, for the named hosts only |
| Caddy routes `git.<DOMAIN>`, `<DOMAIN>` and `www.<DOMAIN>`, nothing else | git-pages serves any domain whose DNS authorizes an upload; with only our hosts routed to it, nobody else's domain can be published on this server |
| no `*.pages.<DOMAIN>` wildcard | one site needs one certificate; a wildcard would need a DNS-01 challenge or on-demand certificates, and a wildcard record |
| the runner's jobs run in their own Docker (`docker:dind`), never the host's | a job, which runs whatever a pushed workflow says, cannot reach the server's containers or their data; the price is a privileged container |
| the runner registers with a shared secret, and its config is written from `.env` at start | the registration is two commands, and the runner's token never enters the repository |
| the runner's connection is Forgejo's public address | the runner hands that address to its jobs, which clone from it |
| SQLite, LFS on, SSH on its own port (2222 by default) | one person's server; LFS is what k7 workspaces use; the host keeps 22 for its own sshd |
| images pinned to exact versions, each at least two weeks old | an update is a reviewed change to `compose.yaml`, never a pull that happened to land, and a release has had time to show its regressions |

The pinned versions: Forgejo `17.0.1`, Forgejo runner `13.2.1`, Docker `29.9.2-dind`, git-pages `0.10.1`, Caddy
`2.11.8`; the jobs run in `node:24.22.2-bookworm`, and the workflow pins `actions/checkout` v7.0.1,
`actions/setup-node` v7.1.0 and `actions/git-pages` v2.2.0 by commit.

## Setting up the server

These steps belong to the owner of the server and the domain; nothing in the repository performs them. The host
needs Docker with the compose plugin, and ports 80, 443 and 2222 open.

1. Copy `deploy/forgejo/` to the server, then `cp .env.example .env` and fill in `DOMAIN` (`k7.rocks`) and
   `ACME_EMAIL`.
2. DNS, at the domain's registrar:
   - `git.k7.rocks`: an `A` (and `AAAA`) record to the server.
   - `_git-pages-forge-allowlist.k7.rocks`: a `TXT` record `https://git.k7.rocks/<owner>/k7-daw.git`, the
     repository that may publish the site.
   - `k7.rocks` and `www.k7.rocks` stay on GitHub Pages until the switch below.
3. `docker compose up -d forgejo caddy pages docker`, then create the administrator:
   `docker compose exec -u git forgejo forgejo admin user create --admin --username <owner> --email <mail> --random-password`
   (it prints the password; change it at the first sign-in at `https://git.k7.rocks`).
4. The runner: `openssl rand -hex 20` is its secret; register it with
   `docker compose exec -u git forgejo forgejo forgejo-cli actions register --name k7-runner --secret <secret>`,
   which prints its UUID. Put the UUID in `RUNNER_UUID` and the secret in `RUNNER_TOKEN` in `.env`, then
   `docker compose up -d runner`. (Site administration, Actions, Runners, **Create new runner** gives a UUID and a
   token too; either goes in the same two lines.) The runner shows as idle on that page.
5. Push this repository to `<owner>/k7-daw` on the server (a new repository, then `git push` of `main`).
6. Run the `pages` workflow once (the repository's Actions tab, `pages`, Run workflow), or push a change to the site.

## Switching k7.rocks to Forgejo, and back

Before the switch the server holds the site but `k7.rocks` still points at GitHub; the upload works anyway, since
git-pages checks the TXT record, not where the domain points. Caddy can only get the certificate for `k7.rocks`
once the domain points at the server, so the first visits after the switch wait for it (seconds).

- **To Forgejo:** replace the apex's GitHub `A`/`AAAA` records with the server's, and point `www` (a `CNAME` to
  `git.k7.rocks`, or the same `A`/`AAAA`). In GitHub's Pages settings, remove the custom domain so GitHub stops
  claiming it, and disable the `pages` workflow there if GitHub should stop building.
- **Back to GitHub:** put back the records [site.md](site.md) lists and the custom domain in GitHub's Pages
  settings. The Forgejo side keeps publishing to git-pages, harmlessly, until its workflow is disabled.

`docs.k7.rocks` is not served here either ([site.md](site.md)); a host Caddy does not route gets an empty answer.

## A local trial

The kit runs on a Mac (OrbStack) with `DOMAIN=k7.test`, `SCHEME=http` and `BIND_ADDRESS=127.0.0.1`, plus a compose
override that adds a CoreDNS answering `*.k7.test` and the allowlist TXT record, set as the `dns` of the `pages`,
`runner` and `docker` services (and `--dns` of the inner `dockerd`), and the repository variable
`PAGES_SITE=http://k7.test/`. That trial covers the runner's registration, the workflow's build from a fresh clone,
git-pages' DNS-allowlist check of the job's token, and the site served at `k7.test` and `k7.test/docs/`.

Only the real server and DNS cover the Let's Encrypt certificates (`SCHEME=https`), the public TXT lookup, and the
jobs reaching `https://git.k7.rocks` from inside the runner's Docker through the server's public address.
