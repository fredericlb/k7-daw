# The change set and the verb bundle (step 3d)

`POST /changes`, `GET /verb-bundle` and `GET /verb-bundle/library`: what the browser's verbs commit to and read from, native in
the Rust file service. The code is `apps/k7-daw-backend/packages/k7-file-service/src/changes/`
and `verb_bundle.rs`, and the front's `native/changes.rs` and `native/verb_bundle.rs`. The routes belong to the `journal` piece and
have no fallback: a front that does not answer the journal says 503.

## Choices

| choice | why |
|---|---|
| judging has an order: shape (400), limits (413), gates and permission (403), parse (422), the open project (409 `open`), freshness (409 `stale`) | a set that is both forbidden and stale, or forbidden and for another project, says forbidden; freshness is the only check that depends on the disk, so it comes last and under the lock |
| the wrong `at` answers `open` and not `project` | the Worker reruns a verb when a file moved and reopens when the project did; "another project is open" is neither |
| a `read` op is a `was` with no write | a verb reads shared texts outside the project's walk (`_modules`, `_playlists`, pins), which the token and the written files' `was` never see |
| freshness is the project's token walked under the write lock, plus each written file's `was` | `was` alone misses a file a verb only read; the token alone misses a shared file outside the project's walk |
| a text equal to what the file already holds is never stale and is never written | `PUT /file` lets an identical save through; a rerun after a 409 must not conflict with itself |
| every written file is staged beside its target, the old one renamed aside, the new one renamed in; a failure renames all back and removes the folders the set made | a verb that fails writes nothing (3-D7); the rename is the one step the disk makes atomic |
| one journal step through the existing bracket, binaries snapshotted before they are replaced | one proven path to the journal; a walk costs 10 ms on `cap128` |
| the account's write lock covers the landing and the verb bundle's `after` ask; the native stamp takes it again for its own write, and the journal's end takes none | `PUT /file` drops it there; a stamp held under the landing's lock would wait on itself |
| one relay batch through the capture's window held around the commit, named by the verb line | the window is what `PUT /file` uses; its release sweeps once, so peers see one entry |
| a written name that hides (any `skip` name) or climbs is refused, and a library is written by an admin alone | the browser composes the writes, so the server checks what lands, never what the verb meant |
| a copy or a move replaces an existing destination only with `replace: true`; a folder is never copied or moved | a set never overwrites audio by surprise; `/move` moves folders |
| `adopt` reads `<studio>/.job-out/<job>/<out>`, copies it in, removes it after the landing, and sweeps job folders a day after their newest file | audio never rides a body; the job's route (3f) writes the staged file |
| the bundle's `source` is the song crate's `SourceBundle` over a walk, libraries are named by a sha-256 and fetched by it | a library is megabytes and shared by every project of its version; the browser fetches a hash once |
| a library's hash covers paths, lengths, WAV seconds and the bytes of texts, not of binaries; the hash and the answer come from one walk | hashing a bank of samples would read it all for no change a text diff misses; two walks would cache a library forever under a hash its content is not |
| the library answers `Cache-Control: private` | the answer is the account's and authenticated; a shared cache must not serve it to another account |
| every path of the bundle is virtual (`/w`, `/l/<name>`, `/k7`, `/s`, `/b/<n>`, `/x/<n>`), the server holding the mapping | the browser's vfs needs the paths to agree with each other, not to be the server's; a hosted install's layout and an account's folder name stay private |
| a link is refused in a written path, a copy source and the staged-output folders (`.job-out`, the job's folder) | the output is read through the link, so it would not be the job's |
| the bundle is refused past the song crate's bound with 507 | a bundle cut short would be replayed as a project missing files |
| a hidden name is listed and never read | a verb lists what a folder holds, and nothing hidden is the workspace's |
| `journal` is `join`, `skip` or absent | `skip` is a pin that must not be an undo step; the set still lands whole and answers its token |
| `create` names a new project folder below the workspace and takes an empty `at`; the set may then write that folder and the shared texts | an import's samples are binaries that cannot ride a body, so they come first through `/upload` into the folder and the texts follow in one set |
| a set with `create` is refused 409 `taken` when its folder already holds a `.k7x` file, and that folder is never removed | the name is the person's; a racing second import must not take a first project's folder away with its own failure |
| `create` with `discard: true` and no ops takes the folder away | an import whose uploads failed before its set has no set to fail, and the samples it sent must not stay; the same look for a project keeps a folder that is one |
| every set with `create` that does not land (a refusal or a failed commit) removes the folder, under the account's write lock and after looking for a project again | the samples uploaded for it belong to nothing else, and a folder left behind is a project nobody asked for |
| `at` is the open project, a `_libraries/…` chain folder the person may write, or empty for the workspace alone | the chain builder and the project-less verbs (`library`, `playlist`, `system channel`) run in the Worker too; the same gates as any write there decide who may |
| a set for a chain or the workspace alone is judged by each `was` and `read`, not by a token; the workspace alone may write only `_modules`, `_playlists` and the pins | the revision token follows one project per workspace, so a second kind of folder would flip it at every ask; the shared texts are the only thing a project-less verb writes |
| the answer's `rev` is read after the engine's push when the set asked for the reload | the push bumps the revision, and a `rev` read before it is a base the next set can never use: every second verb would rerun |
| the bundle lists the studio's `.renders` folder (listed empty when it is not there) | an automatic freeze stats a render per lane, and a compile that meets an unlisted path asks for it alone, one round per lane |
| the bundle says its `lengths` are complete (`measured`), and lists the models folder empty when it is not there | a compile of a replay asks the header of an audio file its table lacks (a mov, an mp3) and the page cannot give bytes; a missing models folder is asked for by name, once per model the compile picks

## Can wait

| what | why it waits |
|---|---|
| names that differ by case alone (`a.k7x`, `A.k7x`) are two names to the plan | on APFS and NTFS they are one file; the plan compares bytes |
| `was` on a binary remove and on a `replace: true` destination | inside the project the token covers them; outside it (a shared or another project's folder) nothing does |
| a crash while a set lands | the landing is atomic while the process lives; a crash mid-set leaves targets renamed aside as hidden temps and the capture's next sweep relays half a set (an intent file comes later) |
| a restore that cannot rename the backup back | the old file then survives only as a hidden temp; a binary-named folder removed lands as an orphan backup folder |
| the plan runs before the account's weight is taken | it parses up to 8 MiB of `.k7x` and stats the disk before the gate; a set past the gate pays for it |
| a write that takes the lock but not the journal's gate (the capture landing a peer's batch) | it can fall into a set's before-walk and an undo then reverts it, as with `PUT /file` |

## Not done here

The settle's k7x half is native (`k7_sync::k7x`); the relay batch is made by the capture. The directories `also_readable`, sample banks and
the models cache are not in the bundle: a verb that reads them asks for them.

## Worker runtime

A browser Worker runs the verbs over a virtual file system and sends this route's change sets: see `apps/k7-daw-frontend/packages/commands/docs/runtime.md`.
