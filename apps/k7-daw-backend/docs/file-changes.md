# The change set, `POST /verb`, the picture and an import

`POST /changes`: a set of writes landed whole as one journal step, native in the Rust file service; a verb and an import
land through it. The code is `apps/k7-daw-backend/packages/k7-file-service/src/changes/` and the front's
`native/changes.rs`. The routes belong to the `journal` piece and have no fallback: a front that does not answer the
journal says 503.

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

## The picture: `POST /picture` and `POST /picture/scene`

`POST /picture` answers the open project's picture document and its warnings, `{doc, warnings}`, or the compile's refusal
`{error}`; `POST /picture/scene {media, armed?, use?}` answers `{scene, mixes}`, the scene `@studio/ui-eyes` draws. Both are the
`verbs` crate's `picture/` read over a `StagedFs` of the workspace; the code is the file service's `edits/picture.rs` and the
front's `native/picture_run.rs`.

| choice | why |
|---|---|
| a picture that does not build answers `{error}` with a 200 | the refusal is the compile's sentence, which the viewport shows; only a mount that cannot be read fails the call |
| the document's media and LUT paths are workspace-relative | the page fetches every byte back through `/media` |
| the scene's files are `media` followed by each one's encoded workspace path, `media` sent by the page | the page's `/media` URL carries its own session in the query, which the backend does not hold |
| `use: export` reads the optimized copy or the source, never the proxy | a proxy is for drawing live, a film is made of the real picture |
| both reads share the compile cache the verbs keep | a scene asked right after a line recompiles only what moved |
| `picture render` builds its scene with the same builder and asks `POST /verb-host/picture`, its files by path; the route makes each `src` and `lut` a `media/` URL and maps a folder inside a root back to its real path | one builder for the preview and the film; the asker maps only strings that start with `/`, so the route makes the URLs the render page fetches |

## A verb run by the backend: `POST /verb`

`{line, at?, journal?, want?}` in, the `verbs` crate's answer out (`{log, refuse, refused, effects, asks?, threw?}`) with `landed`,
the step's `changed`, `revBefore` and `rev`, and `graph` when asked: the route the page runs every command line and editing gesture
through (`docs/verbs-port.md`). `POST /verb/complete {line, cursor?, labelled?}` answers `{items}`. `GET /verb/catalogue` answers the palette's `CommandInfo[]`
(`apps/k7-daw-frontend/packages/web-protocol/commandTypes.ts`), built by the crate's `palette.rs` from its `commands.json`: it reads
no file, so it answers the same for any workspace. The code is the file service's
`edits/verb.rs` and the front's `native/verb_run.rs` and `native/verb_ask.rs`.

| choice | why |
|---|---|
| the verb runs over a `StagedFs` inside the bracket, under the journal's gate and the account's write lock, and lands through the same commit as a change set | one landing path and one journal step per line; a verb that reads under the lock needs no `base` and is never stale |
| a refusal, a throw or a pending call lands nothing and makes no step, even when the verb wrote before it | a verb that says no writes nothing (3-D7), the rule the crate's tests hold it to |
| a pending call is asked outside the lock, of the front's own routes with the caller's headers, a job polled each second to its end; the line then runs again from its start with every answer so far | a render or a separation takes seconds to minutes, which neither the lock nor the account's weight may be held through; each route keeps its own gates |
| the ids the runs of one line draw are replayed in order, and its render stamp is read once | a call's args must be the same on every run for its answer to match |
| the args' real paths are mapped to the bundle's virtual roots and the answer's back; a job's answer gains its `job` and its `dir` | the routes judge virtual paths, and a verb copies a job's file from its folder in the step it lands |
| `at` is the open project or a `_libraries/…` chain folder the person may write, judged as `/changes` judges it; a chain's line writes only there and the shared texts | the chain builder runs its gestures on the chain it shows |
| `journal` is `join` or `skip`, as for a change set | a knob's drag joins one step, and `graph pin` is no step |
| the stage writes in the workspace and the studio's folder, and reads the meta modules, the libraries and the account's chains | a verb writes its project, the shared texts and the machine's preferences; nothing else is a verb's |
| with no project open the line still runs, journaled as `skip` | the machine's verbs (`system …`) work with no project |
| the engine is pushed once after a landing whose verb asked `reload` or `applyProject`, then the asks are asked and the lines they answer said | the push is the host's half of both effects, and an ask such as `transport play` needs the pushed song |
| `rev` is the project's token read after the push | the page adopts a token as its own only when it is the one after its line |
| one compile cache, for the last project run, kept from one line to the next and shared with completion | its entries are keyed by each file's stamp, so a file that moved is decoded again; one project bounds what it holds |
| the route is a write for the merge gate | a line may write anything a verb writes; a verb that writes nothing is refused while a merge waits, a minor cost |

## A foreign project: `POST /import`

The route builds a set with `create` from the translated files and lands it through the same path, after writing the samples into the folder it claims: `docs/import.md`.
