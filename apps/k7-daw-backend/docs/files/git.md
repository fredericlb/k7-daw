# The git routes (`/api/files/git/*`)

A workspace under git, its remote, its commits and its merges, answered by `apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/git.rs` (with `git_sync.rs`, `git_keys.rs` and the forge seam: `git_forge.rs`, `git_forge_routes.rs`, `git_forge_api.rs`, `git_github.rs`)
over `apps/k7-daw-backend/packages/k7-git` (`../git.md`). The choices of the repository, the transports, LFS and the merge are on that page; this one is the contract.

## The routes

| route | body or query | answer |
|---|---|---|
| `GET /git/state?workspace=` | | `{ mode: "none" \| "local" \| "connected", branch, remote, user, ahead, behind, changed, merge, lastFetch, remoteEmpty? }`; `lastFetch.at` is in milliseconds since the epoch (the fetch cadence keeps seconds); `remoteEmpty: true` when connected, a fetch worked and `origin/<branch>` does not exist (nobody has pushed), absent otherwise; `merge` is the number of conflicts without a choice, absent when no merge waits; a remote with no branch yet makes `ahead` the number of commits on `HEAD` and `behind` 0 |
| `GET /git/status?workspace=` | | `{ entries: [{ path, change, from?, size, binary, lfs, tooLarge }] }` |
| `GET /git/log?workspace=&limit=&before=` | | `{ commits: [{ id, subject, author, time, device, files_changed }] }` |
| `POST /git/connect` | `{ workspace, url, hostKey? }`; `url` is an ssh address, anything else is 400 `ssh-only` | `{ ok, remote, lfsWarning? }`; the repository is made when there is none, the device's key signs in and the sign-in is proved by a fetch through an anonymous remote before the workspace points at the address |
| `POST /git/clone` | `{ url, name, ...the same }` | `{ ok, name }`, a new workspace |
| `POST /git/commit` | `{ workspace, message, exclude? }`; `exclude` names files over 2 GiB to leave out (any other path is `bad-path`), and an unexcluded one is `too-big` | `{ ok, id }` |
| `POST /git/push` | `{ workspace }` | `{ ok }` |
| `POST /git/pull` | `{ workspace, message? }` | `{ ok, state: "up-to-date" \| "fast-forward" \| "merged" \| "conflicts" \| "empty-remote", conflicts? }` |
| `GET /git/merge?workspace=` | | `{ pending, theirs, conflicts: [{ id, path, kind, keyPath?, line?, base, mine, theirs, both, choice }] }` |
| `POST /git/merge/resolve` | `{ workspace, choices: [{ id, choice }] }` | `{ ok, id }` |
| `POST /git/merge/abort` | `{ workspace }` | `{ ok }` |
| `GET` / `POST /git/identity` | `{ name, email }` | `{ name, email }`, per device, the one identity of Settings (`apps/k7-daw-frontend/docs/settings.md`); nothing stored answers two empty fields, and a commit with an incomplete identity is refused `no-identity` |
| `GET /git/key` | | `{ publicKey, synced }`: the one ed25519 public key as an `authorized_keys` line, made on the first call, and whether its keychain item travels to the person's other Apple devices; the private half is never answered |
| `POST /git/forget-host` | `{ host, port? }` | `{ ok, forgotten }` |
| `POST /git/focus` | `{ workspace, front, opened? }` | `{ ok }`: the page says it is in front, which drives the fetch; the page repeats it with each state poll, and a page silent for 10 minutes (two fetch periods) is no longer in front |

### Forges: sign in with GitHub (`git_forge_routes.rs`)

A forge is one account's server (today GitHub only). Every route is `/git/forges/<id>/...`, `<id>` being the forge's host
(`github.com`); `/git/github/...` is the same route for GitHub under its first name, which the page and the e2e spec call.
The codes below are GitHub's: a forge's own prefix replaces `github`.

| route | body or query | answer |
|---|---|---|
| `GET /git/forges` | | `{ forges: [{ id, kind, name, available, login?, scopes[] }] }`, every account this build offers, signed in or not; no network call |
| `GET /git/forges/<id>` (`/git/github`) | | `{ available, login?, scopes[] }`; `available` is false only when the front runs with an empty `K7_GITHUB_CLIENT_ID`, and every other route here but `signout` then answers 404 `github-unavailable`; an id no account has is 404 `unknown-forge`; no network call |
| `POST …/start` | `{ scopes }` | `{ userCode, verificationUri, interval, expiresIn }`; the device code stays in the front |
| `POST …/poll` | | `{ state: "pending" \| "slow_down" \| "expired" \| "denied" \| "ok", interval, login?, keyNote? }`; on `ok` the token is kept, the device's key is uploaded and GitHub's ed25519 host key is trusted for `github.com:22` before the answer; `keyNote` is a sentence when the key could not be added (already used on another account) |
| `GET …/repos` | | `{ repos: [{ fullName, sshUrl, private, pushedAt, k7 }] }`, most recently pushed first, up to ten pages of a hundred; 403 `scope` without `repo`, 401 `github-signout` when GitHub refuses the token (it is dropped) |
| `POST …/repos` | `{ name, private? }` (private by default) | `{ sshUrl, fullName }`; the repository gets the `k7-workspace` topic; a refusal such as a taken name is 422 `invalid` with GitHub's sentence |
| `POST …/signout` | | `{ ok }`; the token goes, the uploaded key stays |

### The forge seam (`git_forge.rs`)

| choice | why |
|---|---|
| what differs per forge is data in a `Profile` (id, name, ssh host, keychain entry, REST base and media type, the repository list's first page and page count, the repository scope) plus a few `Forge` methods matched per variant: the sign-in (`start`, `poll`, a `Step` that ends with a token), the granted scopes, the host-key policy, the key list's address, the topic on a new and on an existing repository | forges differ there and nowhere else: GitHub has a device flow and publishes its host keys, while a self-hosted Forgejo has neither (an authorization code with PKCE, and the host key asked on the first connect); the keys, repositories and topics are the same calls |
| `Forge` is an enum, one variant and one file per forge (`git_github.rs`), rather than a trait object | the sign-in is async, and a new forge then touches the seam's matches, never another forge's file |
| every route over a forge is shared (`git_forge_routes.rs`): the end of a sign-in, the key, the repository pages over `Link`, creation, the topic after a connect, the sign-out | one place holds the token's rules: read from the keychain per call, never answered, a 401 drops it |
| the key is looked up by its `SHA256:` fingerprint before it is posted | Forgejo refuses a key already on any account and can filter its list by fingerprint; GitHub's list compares the same way |
| an account's token sits in its own keychain entry; GitHub keeps `github-oauth`, a new forge takes `forge:<host>` | the entry is never the synced one, and nobody signed in before the seam is signed out by it |

A refusal is `{ error, code }` with the codes of `k7-git`'s errors (`ssh-only`, `unauthorized`, `behind`, `uncommitted`, `unknown-host` with `fingerprint`,
`host-key-changed` with `expected` and `fingerprint`, `no-lfs`, `offline`, `unresolved`, `unparsable`, `merge-moved`, `bad-path`, ...).

## Choices

| choice | why |
|---|---|
| a body carries no secret: the device's own key signs in, and a trusted host key in a body is kept per device and never logged | a workspace folder is copied and shared; its secrets are not |
| a remote that is not an ssh address is refused before any network call, `https://` and `file://` alike, unless the front is started with `K7_GIT_ALLOW_FILE` | HTTPS is not offered: a token typed into a page is a secret one more place; the flag is the tests' and the e2e bed's seam |
| the routes run libgit2 on a blocking thread through `Net::blocking` | the front's runtime is never held by a clone |
| a pull with changes and no `message` is `uncommitted` (409); with one it commits first | "mine" is then always a commit |
| every route answers 403 `device-only` on a server front, a device's own front (`STUDIO_LOCAL_USER`; no médiathèque is needed) only, and a front started with `K7_GIT_ALLOW_FILE`, which is a test bed | a server would keep tokens and keys in a plain file and `connect` could make it call its own loopback services |
| the token is kept in the device's secrets store under its own entry, never the synced one, with GitHub's granted scopes read from `X-OAuth-Scopes`; no route answers it | a token is the person's whole account, so it never leaves the device nor reaches the page |
| the device flow, the key list and the repositories are called from the front with the front's own HTTPS client; the base addresses are `K7_GITHUB_WEB_URL` and `K7_GITHUB_API_URL`, honoured only when their host is 127.0.0.1, ::1 or localhost (a test's fake) and ignored with one log line otherwise; the client id is k7's registered OAuth App, compiled in, and `K7_GITHUB_CLIENT_ID` overrides it at run only (empty turns the path off), because a variable read at build would rebuild the front | the login endpoints have no CORS, and the client id is public; the variables are the tests' seam |
| only github.com's ed25519 key is trusted, and a different key already listed is never replaced | the trusted list holds one fingerprint per host, the ssh client meets ed25519 first, and `host-key-changed` keeps refusing as for any host |
| connecting an existing github.com repository adds the `k7-workspace` topic after the connection, on a task of its own, and ignores any failure | a repository the person cannot administer must still connect, and the connect never waits on GitHub's REST API |
| a connect or a clone that fails puts the trusted hosts back as they were, and a repository it made is removed | a mistyped address must not replace a connection that works |
| one lock per workspace root is held by connect, clone, commit, push, pull, resolve and abort; a second of them answers 409 `busy` at once, and the background fetch skips its turn without an error or a backoff | two operations over one repository fight over libgit2's ref locks and the merge's landing; the page disables its button, so waiting in line would only queue a double click |
| a merge that waits makes every write to the workspace answer 423 `merge-pending`, through `merge_refusal` (`native/git.rs`): the file routes, a history restore, a project's rename or recycle, a copy into the workspace and an open's repairs are all covered, and arming a take is refused too; reads, compiles, play and disarming are untouched | the engine plays both sides while the person decides |
| `commit` and a `pull` with a `message` answer 409 `merge-pending` while a merge waits, and `Repo::commit` refuses too | a commit moves `HEAD`, after which the waiting merge could only be aborted |
| a commit names a checkpoint of the local history with its subject, and a pull's or a merge's landing is one history entry `pull <short id>` (best effort: a workspace the account does not capture has none) | undoing a pull is one step, not one per file |
| pulled files land through the capture's landing when the account has one and the file is under 64 MiB | the ledger learns each file before the watcher sees it, so a pull is not a storm of edits; the landing takes bytes held in memory |
| the fetch runs at open, every 5 minutes while the page reports it is in front, and when it comes back; after a failure no more than every 30 minutes, said once; the fetching task wakes at once when a fetch is wanted instead of waiting out its 20-s look | cheap, and a train journey must not fill the log; a user who opens a workspace sees what is behind within a state poll, not after the look |
