# Project sync: git moves projects, iCloud moves two small things

The decisions of how a workspace travels between devices and people. One mechanism moves projects; iCloud moves what is needed to reach them:

| mechanism | what it carries | who moves the data |
|---|---|---|
| git (ssh) | the files of a workspace: sharing with other people, named versions, a copy on any git host | the person: commit, push, pull |
| iCloud Keychain | the device's SSH key, so one public key on the host serves every Apple device | Apple, automatically |
| iCloud key-value store | the list of workspaces the person added: a name and a git remote each, never a file | Apple, automatically |

Android, Windows and Linux have git only.

## What a repository is

| choice | why |
|---|---|
| one repository per workspace, its root the workspace folder | `_playlists/`, `_references/`, the shared folders and `assemblies/` belong to every project of a workspace; a repository per project would split them or copy them |
| one branch, the remote's default (`main` for a new repository); no branch screen | a musician's question is "is there something new", not "which branch"; a branch made by another tool is left alone and never checked out |
| an ssh address: `ssh://user@host/path` or `user@host:path`; `https://` is not offered | GitHub, Gitea, Forgejo, a NAS: k7 never calls a host's API and never creates a repository; the person makes an empty one and pastes its ssh address |
| `file://` remotes only when `K7_GIT_ALLOW_FILE` is set | the tests' bare repository; on a phone a local path means nothing |
| the git folder is the workspace's own `.git`, on every platform | the folder stays an ordinary repository that any git tool on a computer reads; the capture already skips dot names |
| a workspace lives in the app's local storage, never in a synced folder | a file service that syncs file by file, in any order, makes conflict copies of `HEAD` and `index` and of a song two devices edited; git is the one way a project moves |

## What is committed

| choice | why |
|---|---|
| every file of the workspace except what the ignore list names | the `.k7x` texts, the audio a project plays, the samples: what is needed to open the project elsewhere |
| audio, video and large binaries through git LFS: a pointer in git, the bytes in an LFS store (below, "Large files: LFS") | every version of every take in plain git would be in every clone forever; GitHub refuses a plain blob over 100 MB |
| the ignore list is k7's own, written into `.gitignore` when k7 makes the repository, and also applied through `info/exclude` | a repository cloned from elsewhere keeps its own `.gitignore`, and k7's caches stay out anyway |
| ignored: renders and exports, waveform and analysis caches, the temp names of atomic writes, `.k7-git.json` | each is rebuilt or is the device's own; the builder confirms every name against the code (brief, 3611) |
| stems and freezes are ignored (decision 4) | regenerable; a separated stem set of a song is several hundred MB |
| the capture's state (`<home>/.sync/`) is outside the workspace and never committed | it is this device's ledger and local history |

## Large files: LFS

libgit2 has no LFS, so k7 is its own LFS client, inside its own commit and checkout: k7 never asks libgit2 to filter.

| choice | why |
|---|---|
| `.gitattributes`, written by k7 at `init` and at connect, tracks audio (`wav aif aiff flac mp3 ogg opus m4a caf`), video (`mp4 mov webm`) and archives (`zip k7x-pack`) with `filter=lfs diff=lfs merge=lfs -text` | the same file every git-lfs client reads, so a repository k7 writes opens in any git with LFS |
| any other binary file over 1 MB gets its own path line in `.gitattributes` at commit | a large file of an unforeseen type still stays out of git's blobs; the line tells other clients too |
| the clean step is k7's commit: a tracked file is hashed (SHA-256), copied into `<gitdir>/lfs/objects/<aa>/<bb>/<oid>`, and the index gets the pointer text (`version https://git-lfs.github.com/spec/v1`, `oid sha256:`, `size`) | the layout and the pointer are git-lfs's own, so the store is shared with a git-lfs on the same repository |
| the smudge step is k7's checkout: a pointer's object is written through the capture's landing | a pointer never reaches the work tree, and the engine never opens one |
| status of a tracked file compares the pointer's oid with the file's hash, cached by size and mtime in `<gitdir>/k7-lfs-stat.json` | libgit2 alone would call every LFS file modified (bytes against pointer); a take is hashed once, not at every status |
| a tracked file over 2 GB is refused at commit with a sentence | the largest object the common hosts take (GitHub's limit on its free plans) |
| the batch API (`POST <url>.git/info/lfs/objects/batch`, `basic` transfer, `sha256`) over the endpoint `git-lfs-authenticate` grants, through k7's hyper client | one TLS stack, and the grant is the only credential |
| the grant's header goes to the LFS endpoint's host only; an action's `href` on another host (a storage bucket) is called with the action's own `header` and nothing of k7's | a host hands out signed URLs; k7's credential must never leave for a third party |
| an SSH remote gets its LFS endpoint and header from `git-lfs-authenticate <path> upload\|download` over `russh`, then speaks HTTPS to it | what GitHub, GitLab and Gitea answer; the pure-SSH `git-lfs-transfer` is not spoken, and a host offering only it is refused like a host without LFS |
| push uploads every LFS object the pushed commits name and the server lacks (the batch says which), verifies when asked, and only then updates the ref | a ref naming an object the server lacks is a broken commit for everyone who pulls |
| a host without LFS (the batch endpoint 404, 501, or `git-lfs-authenticate` unknown) refuses the push before anything is sent, with a sentence naming the host; connect probes it once and warns | a push half done leaves pointers nobody can resolve |
| pull and clone download eagerly the objects of the commit checked out, before any file lands; older versions' objects are fetched when a restore or a diff needs them | a project must play offline after a pull; a song's whole past on a phone is gigabytes nobody asked for |
| the store keeps an object until it is on the remote and the work tree holds a file with its hash; past that, it is removed (a prune after each push and pull) | the store would double every take on a phone's disk; a pruned object is downloaded again if an old version needs it |
| the merge compares LFS files by oid: one-sided wins, both changed is a binary conflict (mine, theirs, both) | an audio file is never merged by bytes |
| no LFS locks: the locking API is never called, `locksverify` is ignored | k7's merge settles concurrent work; a lock would need every collaborator online |

## Who commits, and with what identity

| choice | why |
|---|---|
| a commit is the person's act: a message, the changed files listed, nothing automatic | the user's decision; a version is what the person says is one |
| author and committer: a name and an email in k7's settings, per device, prefilled from the signed-in account when there is one | git needs an identity; with the server fading, the account is not always there |
| a trailer `K7-Device: <device name>` on every commit | "which of my machines did this" is the question the history is read with |
| a commit is also a checkpoint in the local history, named by the commit's subject | the two histories line up where the person marked a point |

## Authentication

| choice | why |
|---|---|
| SSH only: HTTPS is not offered, so no token or password is asked or kept | a token over Basic is a secret typed into the page and kept per host; one key that the host knows is the whole sign-in |
| the device has ONE ed25519 key, made by the app on first need and kept in the keychain; the page shows its public line with a Copy button, and a person never brings a key | the person adds one line to a host (a deploy key, or the account's SSH keys); the private half never leaves the device |
| the server's host key is checked against S4's trusted list | the first-contact flow and the changed-key warning exist and are tested |
| the SSH key's keychain entry is synchronizable (iCloud Keychain) on Apple devices and per device elsewhere (decision 10) | one public key on the host serves the Mac and the iPhone |
| no secret in `.git/config`, in the URL or in `.k7-git.json` | a workspace folder is copied and shared; its secrets are not |

## The transports

| choice | why |
|---|---|
| `git2` with default features off: no `https`, no `ssh` feature | those features bring OpenSSL (Android, Linux) and `libssh2`, which the backend refuses as it refuses aws-lc; a C crypto stack per platform is a build per platform |
| the front's `hyper` and `hyper-rustls` (ring) carry only the LFS leg an SSH remote's `git-lfs-authenticate` points at, with the roots and `K7_EXTRA_CA_CERTS` | one TLS stack, the one every other outbound call of the front uses, and the iOS CA bundle comes for free |
| SSH is k7's own subtransport over `russh` (already pinned, ring), running `git-upload-pack` and `git-receive-pack` on an exec channel | no `libssh2`; S4's host-key list and key store are the ones the connection reads |
| libgit2 runs on a blocking thread; the subtransport drives `hyper` and `russh` through the runtime's handle | libgit2 is synchronous; the front's runtime must never be blocked |

## Knowing there is something to pull

| choice | why |
|---|---|
| a fetch when a workspace opens, then every 5 minutes while it is open and the app in front, and when the app comes back to the front | cheap (a ref advertisement when nothing moved), and the person sees news within a song's take |
| a fetch never waits for a user's operation: it skips its turn while the workspace's lock is held, with no error and no backoff | the lock is not the network's failure |
| a failed fetch backs off to 30 minutes and says why once (no network, auth, host key) | a train journey must not fill the log or the screen |
| the page repeats that it is in front with each state poll; a page silent for two fetch periods is no longer in front | a closed tab or a killed web view sends nothing, and the tray must not fetch for ever for a page nobody has open |
| the bar shows "N to pull" and "N to push" from the remote-tracking ref against `HEAD` | it is the only state the person acts on |
| no fetch in the background on a phone | iOS and Android give no reliable background slot; the next open fetches |

## Push and pull

| choice | why |
|---|---|
| one lock per workspace root covers every git operation; a second operation answers 409 `busy` at once | two operations over one repository fight over libgit2's ref locks and the merge's landing; the page disables its button anyway |
| git is a device's own: its routes answer 403 on a server front | a server would keep tokens in a plain file and a connect could make it call its own loopback services |
| push only fast-forwards; behind the remote, the Push button becomes "Pull first" | a force push loses another person's work; k7 never offers it |
| a pull with uncommitted changes asks for a commit message first, in the same dialog (decision 3) | "mine" is then always a commit, so a merge can be aborted to exactly where it began |
| a fast-forward re-checks `HEAD` and the work tree after its downloads and moves the branch from its planned value, never by force | an edit or a commit made during a download is never lost |
| a fast-forward pull writes the files through the capture's landing | the ledger learns each file before the watcher sees it, so the pull is one entry of local history ("pull a1b2c3d"), not a storm of edits |
| a pull that diverges is a three-way merge from the merge base, computed in memory, the workspace untouched until every conflict is chosen | a `.k7x` cannot hold conflict markers; a half-merged project on disk would load in the engine |
| while a merge waits, the workspace is read-only (423, the reader's refusal of the WebDAV share, which every write path asks) and plays; arming a take is refused, and a commit or a pull with a message answers 409 | a take filed into files the merge is about to write, or a commit that moves `HEAD`, would make the merge unresolvable; the engine still plays both sides |
| the pending merge is kept in the git folder (`k7-merge.json`: their commit, each file's result, each open choice) | a choice survives a restart; aborting deletes it and nothing else |
| once every choice is made: the files land, the merge commit has both parents, and the person pushes | the merge is a version like any other |

## The merge, per file

| file | merged by | a conflict offers |
|---|---|---|
| `.k7x` | `k7_sync::k7x`: the base-to-theirs change as key-path ops, settled against mine; what settles aside is a conflict at that key path | mine, theirs; both when the conflict is a whole item two sides added under one key of a keyed list whose key holds a name or a number `id` (mine stays, theirs is inserted right after it as `<name> (<author>)`, a number added when taken, or under the next free id); never for a single value, nor for a list keyed by a time, a note or a param, where another key would change the song |
| `.k7x` whose change is a whole text (the k7x diff gives no ops) | `k7_sync::diff3` on lines, the result parsed as k7x | mine, theirs, both; a "both" that does not parse is refused with the line |
| other text | `k7_sync::diff3` on lines | mine, theirs, both |
| binary (audio, images; an LFS file by its oid) | equal or one-sided wins; both changed is a conflict | mine, theirs, keep both (theirs beside as `<name> (<their device or author>).<ext>`) |
| added on one side, removed on the other | a conflict | keep, remove |

A conflict's choice is per hunk: a key path for a structured `.k7x` change, a block of lines for a text.
`k7_sync`'s `diff3` and `k7x` are the relay's merge, proven by `apps/k7/backend-rs/fixtures/` vectors; a second
merge would disagree with them on the same edits.

## Git and the local history

| choice | why |
|---|---|
| the local history (W9: the capture's batches in a room per workspace, auto checkpoints, restore) stays | it records every edit with no commit and no remote, and undo across a session needs it |
| the History panel has two lists: Edits (local, this device) and Commits (`git log` of the branch) (decision 5) | an edit is minutes old and unnamed; a commit is named and shared; one list would drown the commits |
| a restore from Edits is an ordinary change that the next commit carries | git sees files, never the room |
| the capture never commits | the user's decision: git is manual |
| the history's mirror to a remote folder (`history/share.rs`, `mirror.rs`) goes | git is the shared history now |

## iCloud

| choice | why |
|---|---|
| iCloud carries two things only: the SSH key (decision 10) and the list of the workspaces the person added (decision 7) | a project is edited on several devices at once and a file service that syncs it file by file makes conflicts of the very files a person plays; git settles them in a merge the person sees |
| the list is iCloud's key-value store, one key per workspace, `ws.<id>`, holding `{id, name, remote, addedAt}` | the store is last-writer-wins per key and holds under 1 MB; one key per workspace means two devices never overwrite each other's list |
| a workspace's id is a hash of its normalized remote when it has one, else a random id kept on the device that made it | two devices that cloned one repository write the same key; a workspace with no remote cannot be matched across devices anyway |
| `remote` is the git ssh address when the workspace is connected, else null | the list says where a workspace can be fetched from, and only git can bring it |
| the home page lists, under "On your other devices", the entries with a remote that are not on this device, each with a Clone | the clone is the existing clone dialog, with the address and the name filled in: ssh with the shared key, the host-key question, the name's collision check |
| an entry with no remote is kept but not shown | a workspace needs a git remote to travel; connecting it writes the remote into the list and the entry appears on the other devices |
| removing a workspace writes a tombstone `{id, deleted: true, at}` for 30 days, and a device never writes an entry over a tombstone by itself | a removal on one device reaches the others, and a device that still has the workspace does not bring it back at its next start; an explicit add or connect does |
| the list is brought up to the device at start, on every read of it, and after a workspace is added, connected, cloned, renamed or removed | the device's own file of what it listed (`.k7-roam.json` in the account's folder) is how a removal is noticed |
| the store is two C callbacks installed by Swift before the front starts, behind a trait whose default does nothing | Android, Windows and Linux have no list and the page shows no section; the tests use a fake |
| the page reads the list through the front (`GET /git/roam`), at open and every 20 s while visible | a change from another device is applied by iCloud to the store; the next read sees it |
| iCloud signed out or iCloud Drive off: the list is the device's own and nothing roams | the app works as it does without a list |
| the container `iCloud.se.frediver.k7.daw` is read once, to move out the workspaces an earlier version put there (`k7-roam`'s `container.rs`) | a workspace is moved by rename or by a verified copy, never over a name that is already here, and never while a file is not downloaded; what cannot move stays in the container and is named in the log |

The Swift side and the Rust side meet through one C call added to `K7Embed.h`, `k7_roam_install(list, put)`: Swift hands the
front a function that lists the store's `ws.` pairs and one that puts or removes a pair. `apps/k7/backend-rs/docs/roam.md`
holds the mechanism; `engine/k7-runtime/docs/ios.md` the Apple side.

## Android, Windows, Linux

| choice | why |
|---|---|
| git only; the workspaces stay where they are (`filesDir/standalone` on Android, the tray's folder on a computer), and there is no list of workspaces | the list rides on iCloud; there is no list store there |
| the same `k7-git` crate on every platform | one behaviour; a phone has no git binary |
| libgit2 is compiled by `libgit2-sys` with `cc` for each target (no CMake), zlib by `libz-sys` | the engine's toolchains already cross-compile C for the phones; about 1.2 MB of code on arm64 |

## What leaves with the WebDAV share

| part | fate |
|---|---|
| `crates/k7-dav`, `crates/k7-remote`, `crates/k7-sftp`'s verbs, lock, manifest, probe and fake | removed |
| `k7-capture/src/dav/`: `queue`, `worker`, `reader`, `readers`, `sync`, `share`, `shares`, `open`, `fork`, `known`, `link`, `view`, `tree`'s upload | removed |
| `k7-capture/src/history/share.rs`, `mirror.rs` | removed; `history/mod.rs` (the room) stays |
| `dav/secret*.rs`, `hosts.rs`, `keys.rs` and `k7-sftp`'s `keys.rs` and connection | kept, moved into `k7-git` (`auth/`, `transport/ssh.rs`) |
| `dav/landing.rs` | kept, moved to `k7-capture/src/landing.rs`: the pull's writes |
| `dav/failure.rs`'s shape (status, word, sentence) | kept as `k7-git`'s error answer |
| the reader's 423 refusal and the frontend's locked-workspace plumbing (`useLockedWorkspaces`, `lockedWriters`, `verbWrites`) | kept for the pending merge |
| W10a's server import (`routes/files/import.rs`, `tree.rs`'s download behind the source trait) | kept with its server source only, while server workspaces can still be brought down |
| `frontend/src/sharing/` dialogs (`ShareWorkspace`, `OpenShared`, `DavForm`, `SftpFields`, `SftpKey`, `WorkspaceBar`) | removed; `SftpKey`'s key making and pasting reappear in the git connect dialog |
| `e2e/src/davFake.ts` and the dav specs, `docs/dav.md`, `files/dav.md`, `sftp.md`, `webdav-share-plan.md`, `webdav-share-brief.md`, the frontend's `sharing.md` | removed or rewritten as the git pages |

## Where it lands

| choice | why |
|---|---|
| this work is on `webdav-icloud`, from `webdav`, and merges into `webdav` | it builds on the local-first workspaces and W10's removal of the server's |
| `webdav` becomes the next `main` once git sync works and the iOS and macOS test round passes | the local-first workspaces, W10 and git are one product; `main` keeps shipping until then |
| the new crate and its transports start now; the removal of the share waits for W10f and W10g on `webdav` | the removal and W10 touch the same capture and hub files |
| the médiathèque's removal lands on `main` and reaches `webdav` before the share's removal | what W10 kept "for the médiathèque" (the device's pairing, the shares) can then go with it rather than be kept twice |
| the key and the list need a signing team | an unsigned build has no iCloud entitlement; CI's unsigned `.ipa` runs with no list |

## Apple's portal, the user's part

1. Certificates, Identifiers & Profiles: on the App ID `se.frediver.k7.daw`, enable iCloud; the key-value store needs no container.
2. Regenerate the development and distribution profiles of the App ID: the profile lists `com.apple.developer.ubiquity-kvstore-identifier`.
3. A device or simulator signed in to an Apple ID with iCloud on; two devices on one Apple ID to see the list travel.

## Tests

| what | where |
|---|---|
| status, commit, ignore, the LFS clean and smudge, log, fetch, push, clone, fast-forward, the merge and its pending store | `k7-git` tests, a bare repository in a temp folder as the remote (`file://`), every CI run |
| LFS: batch, upload before the ref, eager download, prune, a host without LFS, a foreign `href` getting no token | an in-process LFS server in the tests (hyper's server side and `sha2`, both in the workspace), every CI run; the SSH fake answers `git-lfs-authenticate` |
| a repository k7 pushed read by git-lfs, and the other way | `#[ignore]` tests that need the `git` and `git-lfs` binaries on the developer's Mac, run by hand |
| the SSH subtransport | `k7-sftp`'s former russh fake, moved, running `git-upload-pack` with the `git` binary; skipped likewise |
| the merge of `.k7x` | vectors: base, mine, theirs, the expected text and conflicts, including the relay's `settle-vectors` cases |
| the list (encode and decode, the tombstone rules, the sweep that follows the device, the callbacks) and the move out of the container (a move, a refused name, a placeholder, a failed copy, a repeat) | `k7-roam` tests over a fake store and temp folders, CI |
| the screens | vitest on the state reducers; one e2e spec per bed, against a bare repository the spec makes |
| the Swift bridge and two devices sharing the list | a signed build on devices, by hand, a checklist in `engine/k7-runtime/docs/ios.md` |
| libgit2 on the phones | `android-check.sh` and `ios.yml` build it; a clone and a push from a phone are on the device checklist |

## The user's decisions (taken on 2026-11-17)

| # | decision | taken |
|---|---|---|
| 1 | repository granularity | one per workspace |
| 2 | audio | git LFS, k7's own client |
| 3 | a pull with uncommitted changes | commit first, in the same dialog |
| 4 | stems and freezes | ignored, regenerated |
| 5 | history screen | two lists, Edits and Commits |
| 6 | transports | k7's own SSH over russh; HTTPS is not offered for git, only for an SSH remote's LFS leg |
| 7 | what iCloud carries besides the key | the list of workspaces, one key-value entry each; projects never live in iCloud and travel by git only (taken on 2026-11-18) |
| 8 | workspaces in the Files app | none: they are in the app's own storage (taken on 2026-11-18) |
| 9 | where a workspace's git folder is | its own `.git`, on every platform (taken on 2026-11-18) |
| 10 | the git SSH key on Apple devices | one key through iCloud Keychain (taken on 2026-11-18); every other git secret, and every other platform, stays per device |
| 11 | fetch cadence | on open, every 5 minutes in front, on return |
| 12 | `webdav` and `main` | `webdav`, with this work, becomes the next `main` once git sync works and the iOS and macOS test round passes |
