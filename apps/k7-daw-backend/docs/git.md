# The git crate (`apps/k7-daw-backend/packages/k7-git`)

A workspace as a git repository over libgit2. The decisions of what a repository is, what it commits and who it
commits as are in `docs/project-sync-plan.md`; this page holds the choices the code makes under them.

## The repository

| choice | why |
|---|---|
| `git2` with default features off | its `https` and `ssh` features bring OpenSSL and `libssh2`, which the backend refuses as it refuses aws-lc; k7 has its own transports |
| `Repo::open(workdir, gitdir)` takes the two folders apart | a git folder may live away from the files |
| a git folder elsewhere gets `core.worktree` and no `.git` file | libgit2 links a foreign git folder with a `.git` file; the folder is synced and shared, and the link would be wrong on another device, so `init` removes it |
| libgit2 never filters: no autocrlf, no attributes, no LFS | a `.k7x` and an audio file reach git byte for byte; `core.autocrlf` is set false in the repository whatever the person's global config says |
| the index is built from the diff against `HEAD`, never by `add_all` | the same diff gives `status` and the commit one answer, and a `Cleaner` can turn a file into a pointer before it is stored |
| a symlink or a nested repository is never a change | the capture leaves links out, and a link inside a workspace must not carry a path out of it |
| a rename is an exact match only | inexact detection hashes every pair of added and removed files, which an audio folder cannot afford |

## The ignore list

| choice | why |
|---|---|
| the list is in `ignore.rs`, each name with the code or decision that makes it | a name nobody writes is a line nobody can remove |
| the partial files k7 writes in a workspace (`.<name>.k7-part`, `<name>.part`, `<name>.part-<id>`) are ignored | a commit taken during a download, a prepare or a landing, or after a crash, must not carry half a file to every device |
| `init` writes `.gitignore` when there is none; `info/exclude` always carries the list between markers | a repository cloned from elsewhere keeps its own `.gitignore`, and k7's caches stay out anyway |
| `.k7-git.json` and `.*.icloud` are in the list | a pointer file is the device's own, and a placeholder is Apple's stub of a file not downloaded |
| a tracked file that becomes ignored stays tracked | git's own rule; k7 never removes a person's file from a repository by a list |

## Status and commit

| choice | why |
|---|---|
| a file the device's `Present` hook calls not here is unchanged, in `status` and in a commit | an evicted take must never be committed as removed |
| a commit with no change is refused (`nothing-to-commit`), and so is one with no name or email | an empty version is noise; git needs an identity |
| a file over 2 GiB is refused at commit with its name, unless the commit names it in its exclusion | what the common hosts take; the check runs before any file is read |
| a commit takes an explicit exclusion that may name files over 2 GiB only; `status` flags such a file `tooLarge` and keeps listing it, and a pull that commits leaves every such file out | the file stays out of every later commit with no state to keep, a request that does not exclude it is still refused, and the person's file is never touched |
| a `tooLarge` file is not a change for the pull's and the merge's own "uncommitted" checks, nor for the state's `changed` | a file the commit leaves out must not block the pull that follows it |
| the size limit is a field of `Repo` (`set_max_file`) and a process default (`set_default_max_file`) | tests size the limit down instead of making a 2 GiB file |
| the trailer `K7-Device:` ends every message | "which of my machines did this" is the question the log is read with |
| the identity is a file beside the account's settings, per device, prefilled from the account | the preferences file is the account's and travels; two machines of one person may sign differently |

## The transports

| choice | why |
|---|---|
| the front hands `Net` its runtime handle, TLS config, secret store and trusted-host list | the crate holds no process-wide state but the libgit2 registration; the front's roots and `K7_EXTRA_CA_CERTS` reach it through the `ClientConfig` |
| only ssh is registered, once per process, under its scheme name (libgit2 adds the `://`) | a registration spelled with the separator matches nothing; libgit2 has no http client here, so an http address could not be spoken anyway |
| one operation's context (secrets, runtime, typed failure) travels in a thread-local set around the libgit2 call | libgit2 calls a subtransport on the calling thread and has no channel for anything but a string |
| every libgit2 call runs on `spawn_blocking`; the SSH subtransport and the LFS client drive russh and hyper with `Handle::block_on` | the front's runtime is never blocked, and a call made inline on an async thread panics instead of stalling the front |
| a remote is an ssh address (`ssh://user@host[:port]/path` or `user@host:path`); `https://`, `http://`, `git://` and any other scheme are `ssh-only` (400) | a token is a secret typed into the page and kept per host; the device's one key is the whole sign-in |
| SSH judges the server's key during the key exchange, before the store is read | an unknown key is `UnknownHost { fingerprint }` and a changed one `HostKeyChanged`, and in neither is a secret read or sent |
| a push is a fast-forward only: libgit2 refuses a remote ahead of the branch and the call answers `Behind` | a force push loses another person's work |
| `file://` and bare local paths are `ssh-only` too, except when `K7_GIT_ALLOW_FILE` is set (the tests' and the e2e bed's seam) | on a phone a local path means nothing, and a pasted path must not reach the device's disk |
| a clone is made beside its target under a dot name and renamed into place | a failed clone leaves no workspace; the target is never written into when it exists |
| a clone checks out with no filter and k7's ignore list only in `info/exclude` | the remote's attributes must not rewrite audio, and k7 adds no file to a repository it cloned |

## Credentials

| choice | why |
|---|---|
| the key is kept under `<account tag>/k7@device-ssh-key` in the service "k7 git": the device's keychain, a private file where there is none, iOS's protected keychain on a phone | one entry per account on the device, and never in `.git/config` or a URL; the next row says where it travels |
| the device has ONE ed25519 key, made on first need (`auth::public_key`, or the first SSH sign-in) and kept as one entry of the account's store, whatever the server | a key per workspace or per host would be one more line to add on every host; a person never brings a key, so the private half has one origin |
| the public line is answered, the private text never leaves the crate except to the SSH client | a page, a log or a workspace must never hold it |
| on an iPhone, an iPad and a Mac app signed with its provisioning profile the key's entry is an iCloud Keychain item (`cloud-sync`, no `ThisDeviceOnly` class), read before it is made and moved up from a device-only one; the private text is never a file in iCloud Drive | the user's choice: ONE key for all of a person's Apple devices, so the person adds one public line to a host and has one key to manage; revoking it on a host revokes every Apple device there; the keychain's default class for a synced item is "when unlocked", which a foreground app meets |
| the synced store wins whenever it holds a key: a device adopts it; a device makes or promotes one only when the store is empty, and reads the store again just before writing | one deterministic rule, so a Mac and an iPhone never each settle on a "shared" key of their own; a write that still crosses another before iCloud syncs is the race of the row below |
| the forge tokens (`github-oauth`, `forge:<id>`, `auth::is_token`) are iCloud Keychain items on an iPhone, an iPad and a Mac app signed with its provisioning profile (`apps/k7-daw-desktop/docs/icloud.md`); every other platform keeps them per device | the person signs in to GitHub or a Forgejo server once for all their Apple devices; Android, Windows and Linux have no such keychain |
| `auth::Synced` is the one rule over two stores: an entry that travels is read from the synced store first, and one found only in the device's store (an earlier k7's) is moved up on that read and deleted there | a sign-in made before tokens travelled still works, once, with no step of the person's |
| a Mac whose own key the shared one replaces keeps it, local and never synced, under `old-device-ssh-key` (`Synced::keeps_old`); a promoted key is kept there too, and an old key equal to the shared one counts as none | the Mac's hosts list its own key: a host the person has not given the shared key yet must still let it in, and should another device's write win in iCloud, the Mac still holds the key its hosts know |
| an SSH sign-in offers the shared key, then the old key once when the host refused it; the hosts that took the old key are noted in memory per `Net` (`Net::old_key_hosts`), and a host that takes the shared key later leaves the list | the fallback costs one refused attempt, only where the shared key is missing; the list is what the page names, and the next sign-in to each host rebuilds it after a restart |
| the old key is deleted only by the person (`auth::forget_old_key`, `POST /git/forget-old-key`), never by k7 | k7 cannot know which hosts outside the ones it signed in to still list only the old key |
| without iCloud (a Mac built without a profile, Android, Windows, Linux) the key is the device's own, and no old key exists | nothing syncs, so there is nothing to adopt |
| the access group is the app's default one, `<prefix>.rocks.k7.daw`, named first in both apps' `keychain-access-groups` | the iOS app and the Mac app share one bundle identifier and team, so they read each other's items with no group in the code |
| two devices that make the key before the keychain syncs each keep their own until the later write wins (a Mac then falls back on the key it kept); `GET /git/key` answers `synced` (the item is the synchronizable one), and the page says "shared" or "stays on this device" | a first-need race is rare and heals by itself, and the page cannot know whether iCloud Keychain is on, only which kind of item it holds |
| a locked keychain is an error, and the private file takes over only where no keychain answers | a file must not quietly stand in for a store the person has locked |
| `K7_GIT_SECRETS=file` keeps a desktop's secrets in the private file alone; the dev session and the desktop app task set it (`docs/dev.md`) | an unsigned dev build changes signature at each build, so the keychain would ask at every read; a dev run signs in again once and never reads the installed app's tokens |
| the tests' SSH server and keys are public in the repository | they protect nothing and are used nowhere else |

## Large files (LFS)

| choice | why |
|---|---|
| k7 is its own LFS client: libgit2 never filters, the clean step is k7's commit and the smudge step is k7's checkout | libgit2 has no LFS; a filter hook would hash a take twice and hide the landing from the capture |
| `.gitattributes` is written at `init` (a clone keeps the remote's own) with k7's block between markers, and a path line for any other binary over 1 MB at commit | the file every git-lfs client reads; a large file of an unforeseen type still stays out of git's blobs |
| only the root `.gitattributes` is read, the last line naming `filter` for a path deciding | what k7 writes; nested files are not k7's to interpret |
| whether a blob is a large file is read from the blob (a pointer, parsed strictly, under 1 KiB), never from either tree's attributes; the attributes only decide what a commit cleans | a pull brings the attributes and the files in one tree, landed in path order: a judgment by the work tree's old attributes lands pointer text, or loses the lines a person's "mine" dropped |
| a work file whose bytes already are a pointer is committed as that pointer | a pointer made an object of its own is a pointer to a pointer, and the push replaces the take with text for every device |
| the store is `<gitdir>/lfs/objects/<aa>/<bb>/<oid>` and a pointer is git-lfs's own text, parsed strictly | a repository k7 writes is read by git-lfs and the other way (the `#[ignore]` interop tests) |
| an object is written through a temp name and named only when its hash is the one asked for | a cut or lying download must never become an object |
| a tracked file is judged changed by its hash, cached by size and mtime together in `k7-lfs-stat.json` | libgit2 would call every LFS file modified; a take is hashed once, not at each status |
| a push stocks the store from a work file with the same hash, uploads what the batch names, verifies, and only then sends the ref | a ref naming an object the host lacks is a broken commit for everyone who pulls |
| a host without LFS is one that knows no `git-lfs-authenticate` (no output, an unknown-command line, exit 127) and nothing else: a valid grant means the host speaks LFS (connect asks nothing of the endpoint), a refusal prints the host's own line (`lfs-refused`), and a 404 or 501 on a real batch names the status and the endpoint (`lfs-status`) | GitHub answers the empty object with a 404 and a repository it will not serve with a message; neither is "no LFS", and a push half done leaves pointers nobody can resolve |
| a pull or clone brings the checked-out commit's objects before any file is landed; one the host lacks is `Offline` naming the file | a project must play offline after a pull |
| files land through a `Landing`; a clone uses a direct write beside the file | the front's pull passes the capture's landing so the history sees one entry |
| an object is removed from the store only when the remote branch's tip holds it and the work tree holds a file with its hash, after a push, a pull or a merge | the store would double every take on a phone's disk, and an unpushed take the work tree later overwrites is held by nothing else |
| a push owes the objects of its commits that no parent of the commit holds | a merge commit carries the files of their side, which the host has through them: demanding the bytes would block every push once a person removes one of those files |
| the grant's header goes to the endpoint's host only; an action on another authority gets its own headers and nothing of k7's | a signed URL on a bucket must not be a place the credential travels to |
| the endpoint and header come from `git-lfs-authenticate` over the SSH remote, kept per operation until they expire; the https href it names is the one https k7 speaks | what GitHub, GitLab and Gitea answer; `git-lfs-transfer` is not spoken, and `.lfsconfig` is not read |
| `git-lfs-authenticate` gets the repository path bare when it holds only safe characters, shell-quoted otherwise; `git-upload-pack` and `git-receive-pack` always get it quoted | GitHub does not unquote the argument of `git-lfs-authenticate`, so a quoted path is a repository that does not exist; git-lfs sends it bare, and the test sshd refuses a quoted one |
| a `file://` remote keeps its objects in its own `lfs/objects` | what git-lfs does for a local endpoint, and what the tests' bare repository needs |
| the LFS locks API is never called | k7's merge settles concurrent work; a lock needs every collaborator online |
| LFS transfers are bound by progress like every http call: each chunk uploaded or received restarts the clock | a take of a gigabyte on a slow link passes, a server gone quiet ends the call |

## The merge

| choice | why |
|---|---|
| a pull is refused while changes are uncommitted (`uncommitted`); the route commits first | "mine" is then always a commit, so a merge can be abandoned to exactly where it began |
| the merge is planned in memory and kept in `<gitdir>/k7-merge.json`, written whole or not at all; the work tree and `HEAD` move only at the end | a `.k7x` cannot hold conflict markers, and a half-merged project on disk would load in the engine |
| the file names each path's result, each conflict and its choice, never a file's bytes (merged text is an object of the repository) | deleting the file undoes the merge exactly; a restart keeps every choice |
| each path is one of: same, only theirs changed, only mine changed, both changed | only the last needs a merge; a side that left a path as the base had it yields |
| a `.k7x` change is theirs' key-path ops settled against mine by `k7_sync::k7x::settle_k7x`; what finds its value applies, the rest is a conflict at its key path | the relay's own merge: a second one would disagree with it on the same edits (the relay's settle vectors are run through the merge's settle) |
| a `.k7x` whose change is a whole text, and any other text, is merged by `k7_sync::diff3` on lines, a conflict per block | the same LCS the relay uses; one place for the work budget |
| a result or a choice that leaves a `.k7x` unreadable is refused with the parser's line and not kept | a project that does not load must never reach the disk |
| audio, images and a large file changed on both sides are one conflict: mine, theirs, or both (theirs beside as `<name> (<author>).<ext>`); equal by oid is no conflict | an audio file is never merged by bytes |
| a file one side removed and the other changed offers keep or remove | removing a person's work is always their choice |
| `Both` is offered for a block of lines, for a whole file, and for a `.k7x` item two sides added under one key: mine stays, theirs is inserted right after it under `<name> (<author>)` (a number added when taken) or the next free `id`; never for a single value or a list keyed by a time, a note or a param | a second item under another key must mean something: a name or an id can be another item, a time or a note cannot |
| files land through the `Landing` (the capture's, in the front), removals through its `remove`, large files from the store; if a large file's bytes are missing nothing lands (`Offline`) | the history must see one entry, never a storm of edits, and never a merge half on disk |
| a fast-forward writes only the files that differ and moves `HEAD`; it keeps no pending file | the common case must cost a pull and nothing more |
| a pull downloads the large files a merge brings before the pending file is written | a download that fails after the plan is saved leaves a merge that cannot finish and refuses every write until it is aborted |
| each file lands only if its size and time are those the plan saw; the first that changed stops the landing with `uncommitted` and keeps the edit | an autosave can write while thirty files land, and a landing marker would refuse writes in a window the stat check closes without a lock that a crash can leave behind; files already landed hold their planned bytes, so the next pull resumes |
| a fast-forward re-checks, after its downloads, that `HEAD` is where it was planned and that the work tree holds nothing but the planned bytes, and moves the branch only from that old value, never by force | the downloads take minutes: an edit or a commit made meanwhile must stay |
| a path about to land that is not tracked here, exists on the disk and folds (lowercase) equal to a tracked path is refused with `case-clash` (409) at the plan and again at the landing, naming both | the disk is probed, not the OS name: a volume that reads `Song.k7x` and `song.k7x` as one file would write theirs over the local file, which no status check sees; a case-sensitive volume lets both land |
| the background fetch holds a remote lock apart from the workspace lock; pull, push and connect take both, a commit only the workspace lock, and a fetch that finds the remote lock taken skips its turn | a fetch writes the remote refs and objects and a commit writes the index and `HEAD`, so a commit must not answer `busy` for as long as a slow link takes, while two writers of the remote refs still never meet |

## Auto-save (`src/autosave/`)

A connected workspace auto-saves to the person's **technical branch**; a Save is what reaches the main branch. When a
round runs is the front's (`files/git.md`, "Auto-save"); what a round writes is decided here.

| choice | why |
|---|---|
| one branch per person, `k7/autosave/<slug>`, shared by every device of that person; the slug is the forge account's login when the remote is on a signed-in GitHub or Forgejo host, else the commit email | the branch is the person's own sync channel between their devices, and only becomes a branch apart when several people work on one project, each with theirs; a login is the same on every device of the account, an email is what a custom remote knows of the person |
| the slug is lowercase, `@` spelled `-at-`, every character but `[a-z0-9._-]` a dash, runs of dashes and dots folded, no leading or trailing dash or dot, no `.lock` ending, `user` when nothing is left (`autosave::slug`) | a valid ref name from any login or address, stable across devices; `ada@example.com` is `k7/autosave/ada-at-example.com` |
| `HEAD` stays on the main branch; a snapshot is a commit written beside it, its tree built in an index of its own from the work tree, too-large files left out | the bar, the status, Pull and the merge keep judging the work tree against main, and a Save stays the commit of the whole tree it always was |
| a snapshot whose tree and base equal the branch's tip is no commit | an idle round over an unchanged workspace writes nothing |
| a snapshot's message ends with `K7-Base: <main commit>` and its main commit joins as a second parent when main moved since the branch's tip | another device knows which main the snapshot sits on, and its fetch brings that main commit with the branch |
| a Save is the existing commit of the whole tree on main: one commit per Save, with its message; the snapshots stay in the technical branch's history only | the squash of the branch into main is exactly the work tree once it is flushed, so no second commit path exists |
| after a Save the branch restarts at the new main commit, unless the remote's branch holds a snapshot this device never took in; the reset is pushed forced, under a lease: the remote tip seen at the reset, re-read by a fetch just before | a reset that dropped another device's snapshot would lose it on a machine that died since; a remote that moved past the lease is merged in first and reset at the next Save |
| taking in the remote's branch flushes this device's edits first, then fast-forwards when the remote holds this device's tip, and otherwise writes a merge commit of both tips | the work tree always equals the branch's tip before anything lands, so landing is a plain tree-to-tree move |
| in that merge each side's own changes are kept; a file both changed keeps the newer snapshot's version whole (commit time, then id), the other staying in the history | a person's two devices rarely edit one file at once, a `.k7x` must never be line-merged into an unreadable file, and both devices pick the same side, so they converge |
| main follows the other device's main when it is only behind it (their Save), stays when it is ahead, and the take-in waits when the two diverged | a fast-forward of main loses nothing, while two diverged mains are the person's Update and merge to make |
| an edit made while the take-in downloaded large files cancels the landing for that turn | the person's typing always wins over a snapshot from elsewhere; the next round merges both |
| a snapshot lands through the same `Landing` as a pull, its large files fetched first | the capture's ledger learns each file, and nothing lands half |
