# Work order: project sync, git by hand; iCloud carries the key and the list

The decisions are `project-sync-plan.md`; this page splits them into milestones a builder works from alone.
Rows are numbered 3600 to 3719. Every milestone ends with `bash scripts/agent/gate.sh --milestone` green, on
`webdav-icloud` or a branch made from it.

## The tracks

```
A  k7-git:    G1 repository ──> G2 transports ──> L1 LFS ──> G3 merge ──> G4 routes + fetch ──┐
B  frontend:                       (G4's contract, 3640) ──> G5 screens ───────────┤
C  phones:    G6 libgit2 on Android and iOS (after G2) ─────────────────────────────┤
D  removal:   G0 the WebDAV and SFTP share leaves (after W10f, W10g and the médiathèque's removal)
E  iCloud:    R the list of workspaces (after G4, G5)
```

A is the critical path and starts now: a new crate touches nothing W10 touches. G0 waits (plan, "Where it lands").
E needs a signing team (plan, "Apple's portal").

| milestone | rough size |
|---|---|
| G0 removal | M: mostly deletions, the moves of the keychain and SSH code |
| G1 repository | M |
| G2 transports | L: two subtransports and their test servers |
| L1 LFS | L: the client, the batch API over both transports, the test LFS server |
| G3 merge | L |
| G4 routes and fetch | M |
| G5 screens | L |
| G6 phones | M |
| R list | M |

## Rules every builder follows

- Work in the slot named at launch; `eval "$(node engine/scripts/sccache.mjs agent --export)"`, `--target-dir ~/.cache/k7-slot-<n>`.
- Narrow checks while building (`cargo test -p k7-git`, `npx vitest run <file>`), `docs/agent-self-check.md`, then `gate.sh --milestone`.
- No push, no merge, no `npm install` or `npm ci` (a new npm line goes into `package-lock.json` by hand).
- Dependencies this page names are approved: `git2` with `default-features = false` (it brings `libgit2-sys` and
  `libz-sys`). Any other new crate, npm package or Swift package is a stop-and-ask, shown with its strengths and weaknesses.
- Keep `.claude/state/<branch>.md` at every commit; the report is the short format (`agent-playbook.md`).
- Files under 500 lines, English, one-line body comments, headers of 10 lines at most; biome clean on every js/ts file touched.
- Every rule comes with the test that fails without it, shown by reverting the rule alone.
- A page describes the code AS IT IS: a removed part's page is deleted or rewritten, never annotated with its past.

## G1 — the repository (`crates/k7-git`, track A)

| # | row |
|---|---|
| 3610 | a new crate `crates/k7-git`, `git2` with no default features; `Repo::open(workdir, gitdir)` where `gitdir` is `<workdir>/.git` or a folder elsewhere (I3), opened with the work tree set and no `.git` file written in the latter case; `Repo::init` makes the branch `main` |
| 3611 | `ignore.rs`: k7's ignore list, written to `.gitignore` by `init` and always to `info/exclude`; each name confirmed against the code that writes it (renders, exports, waveform and analysis caches, stems and freezes per decision 4, atomic-write temp names, `.k7-git.json`, `.*.icloud`); a name not found in the code is a stop-and-ask |
| 3612 | `status()`: each changed path (added, modified, removed, renamed), its size, binary or not; a path the `Present` hook (I2) calls not downloaded is reported unchanged |
| 3613 | `commit(message, identity, device)`: the whole tree, the trailer `K7-Device:`; LFS-tracked files go through L1's clean step (until L1, every file is a plain blob) |
| 3614 | `log(limit, before)`: id, subject, author, time, device trailer, files changed; `ahead_behind()` against the remote-tracking ref |
| 3615 | the identity: name and email in the device's k7 settings (the builder finds where settings live), prefilled from the account when signed in |
| 3616 | gate: tests in a temp folder: init, ignore, status, commit, the size rule, log; a removed-but-ignored file never in a commit |

## G2 — the transports (`k7-git`, track A)

| # | row |
|---|---|
| 3620 | `transport/http.rs`: a smart-HTTP subtransport registered for `http` and `https` through `git2::transport::register`, once per process; `info/refs?service=`, then the `POST`s, over the front's `hyper` and `hyper-rustls` connector (ring) with the front's roots and `K7_EXTRA_CA_CERTS`; redirects followed on the same host only |
| 3621 | `http://` refused unless the host is loopback (the tests); a 401 asks the credential store, Basic `user:token`, once; a second 401 is `Error::Auth` |
| 3622 | `transport/ssh.rs`: a subtransport for `ssh` and `user@host:path`, over `russh` (the workspace's pin), exec `git-upload-pack '<path>'` or `git-receive-pack`; the host key checked against S4's trusted list before any credential; an unknown host is `UnknownHost { fingerprint }`, a changed one `HostKeyChanged`, as S4's dialog reads them |
| 3623 | `auth/`: `k7-capture/src/dav/secret.rs`, `secret_ios.rs`, `secret_map.rs`, `hosts.rs`, `keys.rs` and `k7-sftp/src/keys.rs` moved here with their tests (a copy while G0 has not run; G0 deletes the originals); the entry name becomes `k7 git`, `<tag>/<user>@<host>` |
| 3624 | libgit2 calls run on `spawn_blocking`; the subtransports reach the runtime through a `Handle` they are given, never `block_on` on a runtime thread |
| 3625 | `fetch`, `push` (fast-forward only: a non-fast-forward answer is `Error::Behind`), `clone(url, into)` into a dot-named sibling renamed into place, so a failed clone leaves no workspace |
| 3626 | `file://` accepted only when `K7_GIT_ALLOW_FILE` is set |
| 3627 | gate: fetch, push, clone against a bare repository (`file://`); the HTTPS case against an in-process hyper server running `git http-backend` as CGI; the SSH case against the russh fake moved from `k7-sftp/src/fake.rs`, running `git-upload-pack` and `git-receive-pack`; both skipped with one line when `git` is absent; a push behind is `Behind`; a changed host key sends no credential |

## L1 — LFS (`k7-git`, track A, after G2)

| # | row |
|---|---|
| 3700 | `lfs/attributes.rs`: the `.gitattributes` lines k7 writes (plan, "Large files: LFS"), the per-path line for another binary over 1 MB, the match of a path against the attributes as git reads them (patterns, last match wins) |
| 3701 | `lfs/pointer.rs`: parse and write the pointer text exactly as git-lfs's spec, a parse that refuses unknown keys before `oid` |
| 3702 | `lfs/store.rs`: `<gitdir>/lfs/objects/<aa>/<bb>/<oid>`, written through a temp name and checked by hash; `k7-lfs-stat.json`, the size-and-mtime cache of each tracked file's oid |
| 3703 | clean in `commit` (the pointer into the index with `add_frombuffer`), smudge in checkout (the object through the capture's landing), status of a tracked file by oid; over 2 GB refused at commit |
| 3704 | `lfs/batch.rs`: `POST objects/batch` over G2's HTTPS transport, the endpoint from the remote URL (`.git` added when absent) or `.lfsconfig`; the token to the endpoint's host only; an action's `href` on another host gets the action's `header` alone; `verify` called when given |
| 3705 | SSH remotes: `git-lfs-authenticate <path> upload\|download` on G2's russh connection, its `href` and `header` reused until `expires_at` |
| 3706 | push: the objects the pushed commits name, the batch's missing ones uploaded, then the ref; a host without LFS (404, 501, unknown command) is `Error::NoLfs { host }` before any upload; connect probes once and returns a warning |
| 3707 | pull and clone: the checked-out commit's objects downloaded before any file lands; an old version's object fetched on demand by restore and diff; a missing object offline is `Error::Offline` naming the file |
| 3708 | prune after push and pull: an object both on the remote and held by the work tree (same hash) is removed from the store |
| 3709 | gate: an in-process LFS server in `tests/` (hyper's server side and `sha2`, no new dependency): batch, PUT, GET, verify, a switch that answers 404 on the batch, one that names an `href` on a second host and records what reaches it; the SSH fake answers `git-lfs-authenticate`; cases: upload before the ref, eager download, prune, no LFS refused before any upload, a foreign host gets no token; `#[ignore]` interop with the `git-lfs` binary both ways |

## G3 — the merge (`k7-git`, track A)

| # | row |
|---|---|
| 3630 | `merge/plan.rs`: from `HEAD`, the fetched commit and their merge base, each path's case: unchanged, one side, both equal, both changed, added on one side and removed on the other |
| 3631 | `merge/k7x.rs`: `k7_sync::k7x::k7x_op_for(base, theirs)` settled against mine (`settle_k7x`); each op set aside is a conflict `{ path, keyPath, base, mine, theirs, both? }`, `both` offered for a whole item added on both sides under one key of a list keyed by a name or an `id` (the plan, "The merge, per file"); no ops: `k7_sync::diff3` on lines, a conflict per block |
| 3632 | `merge/text.rs` and binary: diff3 for other texts; a binary (an LFS file by oid) changed on both sides is a conflict with `mine`, `theirs`, `both` (theirs written beside under a free name, `<name> (<author>).<ext>`) |
| 3633 | the pending merge in `<gitdir>/k7-merge.json`: their commit, each path's merged bytes, blob id or LFS oid, each conflict and its choice; written atomically; `abort` deletes it and leaves `HEAD` and the files as they were |
| 3634 | `resolve(choices)`: every conflict chosen, each resulting `.k7x` parsed (a "both" that does not parse is refused with the line), the files written through the capture's landing (G0's move of `dav/landing.rs`, copied here until then), the index set, the merge commit with both parents |
| 3635 | a pull with uncommitted changes is refused with `Error::Uncommitted` (the route commits first, decision 3) |
| 3636 | gate: vectors in `k7-git/tests/merge-vectors.json` (base, mine, theirs, expected text and conflicts), including every case of `fixtures/settle-vectors.json` that has a base; a restart between plan and resolve keeps the choices; abort leaves the tree byte for byte |

## G4 — the routes and the fetch (`k7-front`, track A)

| # | row |
|---|---|
| 3640 | the contract, written first for track B, in a new page `git.md` in the backend's `docs/files/`: `GET /api/files/git/state?workspace=` (`none`, `local`, `connected`; remote URL, ahead, behind, last fetch and its error, pending merge or not, changed count), `GET …/status`, `POST …/connect { workspace, url, auth }`, `POST …/clone { url, name, auth }`, `POST …/commit { workspace, message }`, `POST …/push`, `POST …/pull { workspace, message? }`, `GET …/merge`, `POST …/merge/resolve`, `POST …/merge/abort`, `GET …/log`, `POST …/keygen`, `POST …/forget-host`; refusals `{ error, code }` with the codes of `k7-git`'s errors |
| 3641 | the routes in `k7-front/src/routes/files/service/native/git.rs` (beside `dav.rs`, which G0 removes), the account's workspaces only, as the dav routes check it |
| 3642 | the fetcher: one task per open workspace with a remote, a fetch at open, every 5 minutes while the page reports it in front, at the page's return; backoff to 30 minutes after a failure; no secret in a log line |
| 3643 | `pull` with uncommitted changes and a `message` commits first; without one, 409 `uncommitted` |
| 3644 | while a merge is pending, every write to the workspace answers 423 with a sentence, through `Dav::write_refusal` (`dav/readers.rs`), which every write path asks (the file routes, a history restore, a project's rename or recycle, a copy into the workspace, an open's repairs); arming a take is refused too; reads, compiles, play and disarming are untouched |
| 3645 | a commit names a checkpoint in the workspace's local history with the commit's subject; a pull's landing is one history entry `pull <short id>` |
| 3646 | gate: route tests against a bare repository; a write during a pending merge is 423; an uncommitted pull without a message is 409; the fetch's cadence with a fake clock |

## G5 — the screens (`apps/k7/frontend`, track B)

| # | row |
|---|---|
| 3650 | `src/git/` replaces `src/sharing/`: a workspace's bar shows the remote, "N to pull", "N to push", Commit, Push (or "Pull first"), Pull |
| 3651 | Connect: URL, HTTPS (user, token) or SSH (make a key, paste a key: `SftpKey`'s parts), the unknown-host and changed-key flow of S3/S4; Clone: the same, into a new workspace |
| 3652 | Commit: the changed files with sizes, LFS ones marked, the refusal over 2 GB, a message; a push to a host without LFS shows its sentence |
| 3653 | Conflicts: one card per conflict, the key path or the lines, mine / theirs / both where offered, "Finish merge" once all are chosen, "Abort"; editors read-only meanwhile, through `useLockedWorkspaces` and `lockedWriters` |
| 3654 | History: two lists, Edits (the local history, as it is) and Commits (the log) |
| 3655 | the identity fields in settings; en and fr strings |
| 3656 | pages: a new `git.md` in the frontend's `docs/` (replacing `sharing.md`), the user page `apps/k7/docs/02-start-share-workspace.md` rewritten for git |
| 3657 | gate: vitest on the state and conflict reducers; one e2e spec that makes a bare repository, connects, commits, pushes, clones into a second workspace, makes a conflicting edit on each and resolves it |

## G6 — libgit2 on the phones (track C)

| # | row |
|---|---|
| 3660 | Android: `libgit2-sys` and `libz-sys` build in `libk7_embed.so`'s cross build; the APK's size before and after in the state file |
| 3661 | iOS: the same in `libk7_embed.a` through `engine/k7-ios/scripts/stage.mjs`; the size before and after |
| 3662 | `android-check.sh` green; `ios.yml` started by hand on the branch (the user's push) |
| 3663 | the device checklist in `engine/k7-runtime/docs/ios.md` and `android.md`: clone over HTTPS and SSH, commit, push, pull with a conflict |

## G0 — the share leaves (track D, after W10f, W10g and the médiathèque's removal are on `webdav`)

| # | row |
|---|---|
| 3600 | crates `k7-dav`, `k7-remote`, `k7-sftp` removed, with their workspace members and `[workspace.dependencies]` lines; `russh-sftp` and `quick-xml` go if nothing else uses them |
| 3601 | `k7-capture/src/dav/` removed but what 3623 moved and `landing.rs`, which moves to `k7-capture/src/landing.rs`; `history/share.rs` and `mirror.rs` removed; `account/out.rs` and `shared.rs` lose the queue; W10a's import keeps `tree.rs`'s download with its server source only |
| 3602 | `k7-front`: `native/dav.rs`, `tests_dav*.rs`, the `/api/files/dav/` mounting; `rooms_out` stays |
| 3603 | the page: `src/sharing/` but the locked-workspace plumbing (moved to `src/git/` by G5 or here, whichever lands first); `e2e/src/davFake.ts` and the dav specs |
| 3604 | docs: delete backend-rs's `docs/dav.md`, `files/dav.md`, `sftp.md`, `docs/webdav-share-plan.md`, `docs/webdav-share-brief.md`; edit `files/history.md` (no shared history, the device's room only, "k7 has no git" goes), `file-capture.md`, `docs/architecture.md`, `docs/inventaire.md`, `webdav-removal-brief.md`'s "what stays" line on `dav/` |
| 3605 | gate: `cargo test --workspace`, `npm run lint:dead` names no orphan, `lint:docs` green |

## R — the list of workspaces (track E, after G4 and G5)

| # | row |
|---|---|
| 3670 | `k7-roam`: the entry (`{id, name, remote, addedAt}`) and its tombstone under the key `ws.<id>`, the id from the remote or a random one, the rules of a sweep, and the store trait with its no-op and its fake |
| 3671 | `K7Embed.h` and `k7-embed`: `k7_roam_install(list, put)`; `RoamStore.swift` over `NSUbiquitousKeyValueStore`; the entitlement `ubiquity-kvstore-identifier` |
| 3672 | `GET /git/roam`, and a sweep after a clone, a connect and any workspace write |
| 3673 | the home page's "On your other devices" with a Clone that opens the clone dialog filled in |
| 3674 | the move of the workspaces an earlier version left in the iCloud container into the device's own folder (`k7-roam`'s `container.rs`), run at an account's start |

## Stop and ask

- A new dependency not named above, or a `git2` feature turned on.
- A host whose LFS answers differ from the batch API's spec, or that offers only `git-lfs-transfer`.
- A merge result the relay's vectors disagree with, or a `.k7x` the merge writes that does not parse.
- Any path where k7 would force-push, rewrite history, or keep a project in a synced folder.
- A write path that lands files without the capture's landing (the history would see a storm of edits).
- G0 finding a caller in the médiathèque, the shares or W10a that needs a removed part.
- Any project file written to iCloud: only the key and the list go there.
- A user's file deleted by any code path but a pull that removes what the remote removed.
