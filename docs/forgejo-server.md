# A Forgejo server for k7 workspaces

A self-hosted Forgejo is a place where k7 **workspaces** (projects) sync. It hosts nothing else: the app's own
repository and `k7.rocks` are on GitHub, and the site is served by GitHub Pages ([site.md](site.md)).

`deploy/forgejo/` is the server kit: `compose.yaml`, `Caddyfile` and `.env.example`.

## Choices

| choice | why |
|---|---|
| LFS on | k7 workspaces keep audio and other large files in LFS |
| `ROOT_URL` is the public HTTPS address, reachable from every device | Forgejo 17 has no LFS over SSH: `git-lfs-authenticate` hands back `<ROOT_URL>/<owner>/<repo>.git/info/lfs`, and the data travels over HTTPS to it; a device that reaches SSH only cannot sync large files |
| Caddy in front, with Let's Encrypt | one process holds the certificate and renews it; Forgejo itself speaks plain HTTP behind it |
| SSH on its own port (2222 by default) | git runs over SSH with the device's key; the host keeps 22 for its own sshd |
| sign-up closed, the administrator created by hand | the server is one person's or one group's; an open registration is an open disk |
| SQLite | one small server; the whole state is the data folder |
| backups are the data folder (`DATA_DIR`) | it holds the database, the repositories and the LFS objects; copy it while Forgejo is stopped, or snapshot its volume |
| images pinned to exact versions, each at least two weeks old | an update is a reviewed change to `compose.yaml`, never a pull that happened to land |

The pinned versions: Forgejo `17.0.1`, Caddy `2.11.8`.

## Setting up the server

These steps belong to the owner of the server and the domain; nothing in the repository performs them. The host
needs Docker with the compose plugin, and ports 80, 443 and 2222 open.

1. Copy `deploy/forgejo/` to the server, then `cp .env.example .env` and fill in `GIT_HOST` (for example
   `git.example.org`) and `ACME_EMAIL`.
2. DNS, at the domain's registrar: an `A` (and `AAAA`) record from `GIT_HOST` to the server.
3. `docker compose up -d`, then create the administrator:
   `docker compose exec -u git forgejo forgejo admin user create --admin --username <owner> --email <mail> --random-password`
   (it prints the password; change it at the first sign-in at `https://<GIT_HOST>`).

## How k7 uses it

In k7, **Settings, Services, Add a Forgejo server** takes the server's address, checks it (version, LFS on, `ROOT_URL`
answering), and walks through the sign-in; Connect and Clone then list the server's repositories beside GitHub's.
The steps are on the user page [02-start-share-workspace.md](../apps/k7-daw-frontend/user-docs/02-start-share-workspace.md).

## A local trial

`GIT_HOST=localhost`, `SCHEME=http` and `BIND_ADDRESS=127.0.0.1` run the kit on one machine. It covers the server
and the sign-in; a device on the network, and the HTTPS certificate, need the real host and DNS.
