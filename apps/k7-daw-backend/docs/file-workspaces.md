# The file service's native workspaces (piece `workspaces`)

What the home page asks of the file service and the three acts it offers on a whole workspace, in Rust.
`GET /workspaces`, `GET /workspaces/projects`, `GET /workspaces/surfaces`, `POST /workspaces` (make), `/workspaces/remove`,
`/workspaces/copy-project`, and Examples stocked when an account is made. The crate is `crates/k7-file-service` (`workspaces/`).
This page carries the crate's half and the front's: the hook, the capture calls around an act, the memo.

A listing walks every workspace of the account and a copy moves a whole project's bytes, so both run off the request's
async path under the answering weight (1673).

## The names and the listings

| # | decision | why |
|---|---|---|
| 1630 | `name_ok` is `assertWorkspaceName`: `[A-Za-z0-9][A-Za-z0-9._ -]*`, so never dots alone | one rule for the header's gate and the routes' bodies |
| 1631 | `workspaces.json` is frozen, harvested from the TypeScript's own functions: names, a copy's cleaned name and suffix verdict, `playlistPathOf`, `projectTagsOf`, `WORKSPACE_REF`'s matches, `String(n)` and `Math.round`, a demo's stamp, JSON strings | the harvested TypeScript answers are the referee, and the file no longer moves |
| 1632 DEPARTS | A name that names a folder only by its case is resolved to the folder's own spelling (`on_disk`: the disk must say the folder exists; the dirent equal to the name, else the one equal under ASCII case folding) before a removal or a copy does anything. Approved by the user, 2026-11-10 | a removal keyed by the asked spelling leaves the room its files, and a server brings the workspace back |
| 1633 | The open project is read from `<studio>/workspace/.active-projects/<name>`, trimmed as JavaScript trims, resolved against the root and compared as a string; a marker over 1 MiB, or a path over 16 KiB, is `Declined(LongPath)` | a native read never writes |
| 1634 | `project_dirs_under` walks with an explicit stack: a dirent's own type (a link is never entered), `skip`, code point order, a project's own folders too, an unreadable folder costing that folder, every entry against `max_seen` and the depth against `max_depth` | the editor's walk, bounded; UTF-8 byte order is code point order, UTF-16 order is not (the vectors hold it) |
| 1635 | A row's facts in the literal's order: `app`, `tags` (UTF-8 with replacement, parsed by `k7x`; `project.tags`' non-empty strings; a file that does not parse has none; a text the reader reads otherwise is `Declined(LossyText)`), `active`, `editedAt` (the newest mtime two levels down, dot entries skipped, rounded as `Math.round`), `demo` | `workspaceRow` and `projectEditedAt` as node ran them |
| 1636 DEPARTS | A project's surfaces are the song crate's `project_surfaces` over a Source mounted at the project; a read that leaves the project is `Declined(SurfacesOutside)`. Approved by the user, 2026-11-10 | one compiler; the TypeScript reads a link out where the Source refuses, so that case declines rather than differs |
| 1637 | `/workspaces` counts per name: `isSingleProject`, then 1 or the walk's count; no memo | the TypeScript has none |

## Examples

| # | decision | why |
|---|---|---|
| 1640 | The demos folder is the account's `STUDIO_DEMOS` when it exists, else the first `demos` folder walking up at most eight levels under the code root | the demos ship beside the code root |
| 1641 | A tree's stamp is the SHA-1 of its sorted `/<rel>:<size>:<mtimeMs>` lines, `mtimeMs` printed as `String(n)` (`k7_js::number::to_string`), sorted in UTF-16 units, `""` when the walk fails; `.studio-demos.json` is the stamps in order | the stamps file is the one record of what a demo was when it was copied |
| 1642 | `mirror` replaces a demo whole when either side moved, one demo's failure costs that demo, Examples that cannot be made is no mirror, a stamps file that is not an object (or over 64 MiB) reads as none, `null` included, which fails the TypeScript's whole mirror | `mirrorDemos`; it never fails |

## The acts

| # | decision | why |
|---|---|---|
| 1651 | A copy's playlists are those the source project's surfaces name, skipped when the destination has one or the source lacks one; a name whose path leaves `_playlists` once `\` reads as `/` is skipped, by both implementations. Ruled by nobody yet: the TypeScript was given the native guard, its reversible side | the plan reads before it writes; a surface's text must not decide where a file lands, and a `\`-spelled name carried a source workspace's file into the destination outside `_playlists` |
| 1652 | A copy's modules: the copied text files (extension exactly in the TypeScript's set, case kept; no `node_modules` or dot folder) scanned by a matcher of a dozen lines, the first-inserted module walked first, a folder module or a `.k7x` file under `_modules`, skipped when the destination has it or the source lacks it. Both sources and destinations stay inside `_modules`. The walk is in the host's `readdir` order (bytes on Unix, the disk's own on Windows), and a path the plan already writes exists to a later check in the disk's case rule (APFS and NTFS fold) | `carryModules`; no regex crate. The order decides which of a folder module and a file module inside it lands first, and the first makes the second's folder exist |
| 1653 | `plan_copy` in the TypeScript's order: each end's name then presence (source first), the source's gate, the recycle bin, the project suffix, the destination being a project, Examples, the cleaned name, the suffix, the destination's gate, the name taken; then every entry against the limits | every sentence is the specs' or the vitest's |
| 1654 | `apply_copy` writes the plan's entries, the project then the playlists then the modules, and a failure leaves what was written | parity |
| 1660 | `plan_remove` names the folder in its own spelling; `remove` takes the folder whole as the shim's `rm` does (a link is not followed, a junction is a folder) and then its marker | `removeWorkspace`'s disk |
| 1662 | `plan_make` declines with `NoLibraries` before it judges the name: the refusal is the hub's | a decline costs one call on a first start |

## The scope and the hook

| # | decision | why |
|---|---|---|
| 1602 | The piece's acts are the six routes and Examples at the account's start | what the home page asks, and the one write the service makes unasked |
| 1606 | The hook is `workspaces_hook::live`, for the six method and path pairs alone. In order: the body (413, then 400 for one that does not parse); the owner (403); the route's zod fields (400, zod's words); the route | the body is parsed before the owner gate, so a bad body is answered first; every ask carries the request's admin stamp |

## What a workspace decides

| choice | why |
|---|---|
| a server's workspaces each belong to one account, recorded in `<state>/owner.json` beside the root and claimed by an exclusive create; any other account is refused with 403, and an absent record is read again each time | the hub keeps accounts apart and the record makes a mistake there, a key two accounts share, fail closed; a recorded owner never changes while the process runs, so an edit by hand takes effect at the next start |
| an engine's machine holds no workspace: every workspace- and account-shaped route answers 404 there | it is shared by every account that plays on it, with copies of the server's files |
| opening does not create: only `POST /workspaces` and `Examples` make a folder, and a missing folder is a 404 distinct from a malformed name's 400 | a tab retrying a remembered name must not recreate a workspace just deleted, empty |
| `Examples` is stocked when the account's service starts, and a demo is copied again only when the demo's stamp or the copy's stamp changed | the first request is the first use; re-copying every demo on every start taxes every cold start |
| a workspace is a folder and nothing more, and a first workspace has no project | the editor asks what the first project should be; a bare address opens the marker's project, else the first by name, else the first-project wizard |
| a copy keeps the source's suffix, is refused into a workspace that is itself a single project, leaves out what is not content (`*.cache`, the journal, the recycle bin) and nothing outside the tree it copies | the suffix decides which application opens a folder; such a workspace lists `.` and nothing under it, so the copy would appear in no listing; a name that reads `\` as `/` must not climb |
| a copy carries the playlists its surfaces name and the `@workspace` modules its text names, following references inside what it copied, and the destination's own wins | a project alone lands beside no `_playlists/` and opens on "extends unknown module"; libraries are not chased because the destination pins its own |
| a rename of a project is checked with `projectApp` on both names, is not an entry rename, unloads the engine rather than asking for a stop, and the marker follows it | a project cannot become a plain folder by typing; a delete ends the project and a rename keeps the same one; the listing is dropped as after a delete |
| removing a workspace is final, there is no bin for it, and the engine is unloaded first | the bin lives inside a workspace; a loaded project holds files open and Windows answers EPERM half-way through |
| a removal lets the capture go before the folder goes: its watches, its history and its log; the files are the host's | a deleted workspace has nothing to tell a server |
| the home page's listings share a five-second window dropped whole when a workspace is made, copied into or removed, and a captured workspace whose watcher is up keeps its row up to a minute while the change feed is quiet; the open project is read again on every listing | a list five seconds behind is a wait and a stale compile is a lie; the active marker lives outside the workspace where no capture hears it |
| a row carries the surface types, whether it is the active project and `editedAt` over two levels, each optional on the wire, with diagnostics dropped | the page draws these and a page meeting an older service draws a plain row; an edit three folders down only makes a row look older; the editor shows why a file will not parse |
| which surfaces are worth a second device is the app's question, never the service's | the service reports what a project declares |
| every workspace is attached at start behind the first answers, each on a turn of its own, and a request reaching an unattached one attaches it | a learning walk and a watch per workspace cost the hub's probe minutes on a phone |
| a workspace still naming the old default library is rewritten at start by one replica, once per state of its sources (path, size, mtime and the rules' version) | one replica's edit reaches the others once instead of racing; stamping the sources rather than "done" catches a file arriving later in the old spelling |
| the bench is generated afresh at each open from its own route, names the library chain it is on, writes nothing back, and its workspace holds the rig and nothing else | there is nothing in it worth keeping past a session; a `chains/` folder that saves nowhere is worse than none |
| `_libraries/<name>/...` is a virtual folder mapped to the library's root and confined there by the same two gates | `_libraries/../..` and a link a library carries escape nothing |
| the revision token is the mutation count, the files' count and newest mtime, and the mtimes of the engine's scan cache and the machine's preferences | an edit out of process moves it too, and every view keyed on it reads the scan or the preferences, so a rescan must not leave memoised answers stale |
| the token is not a content hash, and the walk is skipped only while the capture's feed is quiet, with ten seconds as the ceiling | reading every file costs more than it guards and the mutation count covers what this process does; a stale token hides an edit from every page, a lost event may only delay it |
| a project's listing scans for directories whose name claims an app at any depth, descends into a project inside a project, sorts by code point and skips what the tree skips | what it answers and what a tree draws cannot disagree and a listing must not reorder between polls; an inner set must not be hidden with nothing on screen to say so |

## Two corrections to the TypeScript

| # | decision | why |
|---|---|---|
| 1690 | A copied file module's refs keep no `..` segment and no trailing `/`, every module's source and destination stays inside `_modules` | a song's text must not decide what else of a workspace lands in another, and an answer must not name a path that climbs |
| 1691 | Make, remove and copy answer an fs failure through `entryFailure`, as `/folder` and `/upload` do; the thrown sentence is logged `[files] ...` | a server's absolute path is logged and never answered |

## The capture and the engine around an act

| # | decision | why |
|---|---|---|
| 1621 | A removal lets go with no lock and no bound of its own: module watches, the served engine, the capture's end, in that order. The folder and its marker go under the lock | a module watch and a served project hold handles Windows refuses to delete under |
| 1622 | A make writes its pins (`pins::ensure_made`) after its folder, inside the write | a workspace left unpinned is pinned to version 1 at its first open |
| 1623 | A copy attaches both ends' captures (in the folder's own spelling, 1632) before its plan and forgets the projects' list after its writes and nothing else | the watcher hears a copy as it hears any write; the compiled doc and the revision stay |
| 1644 | Examples is mirrored by the front when the account is made, under the account's write lock and before the process-wide `making` lock | the first request is the first use; the mirror is the one write no request carries |

## How a write is judged

| # | decision | why |
|---|---|---|

## The acts and the memo

| # | decision | why |
|---|---|---|
| 1661 | Past its plan an act runs to its end in a task of its own, the request awaiting it | a client hanging up after `letGo` would leave a workspace whose room ended and whose folder is still there |
| 1662 | A make with no library on disk is `Declined(NoLibraries)`, refused with its sentence (`declines.md`) | a decline costs one call on a first start |
| 1670 DEPARTS | One memo per account holds `/workspaces/projects` and `/surfaces` as text for 5 seconds; an act of this piece, or a root whose mtime differs (compared for inequality) from the one read before the memo's walk, drops it | the root's mtime moves when a room's join makes a folder |
| 1671 | `/surfaces` is read from the listing's own walk | one walk for both lists |
| 1672 | Every act takes 1b's account lock after its plan's names and before its first write; a listing takes none | a copy names two workspaces and the mirror the account's root |
| 1673 | A listing and a copy's plan weigh a tree (72 MiB) at the gate; none free within 2 s declines `busy` | the gate weighs what a request holds |
| 1674 | The memo holds text, at most one entry per account, dropped with the account | the answer is what is sent |

## Where it departs from the TypeScript

| what | why |
|---|---|
| a plan fails on a source it cannot read (a folder refused its listing, a dangling link) where the copy would write part of the project first and fail | nothing is written when the source cannot be read; the answer is the same code |
| a link loop is stopped by the host after about 32 links (`EIO`) before the depth bound is reached | the plan answers the failure before any write; a real nest is `TooDeep` |
| the playlists and modules are read from the source project, not from its copy | the copy's files are the source's |
| a dirent whose `\`-spelled name folds out of its folder (`a\..\..\x`, `x\..`) is not copied, and a demo's stamp walk that meets one is `""` | the shim reads `\` as `/` on every host: followed, such a name reads and writes outside the project, the workspace or the account's folder, and copies a file onto itself, which truncates it |
| a stamps file read caps at 64 MiB, a marker at 1 MiB, a text at `max_text` | a bound where the TypeScript has none |

## Risks

| risk | answer |
|---|---|
| `letGo` waits up to 30 s on a server out of reach | the TypeScript's own bound; no lock is held meanwhile (1624) |

## What holds it

| check | what it holds |
|---|---|
| `workspaces_vectors` | 5,100 cases from the TypeScript's own functions |
| `workspaces_names`, `workspaces_listing` | the allow-list, links, case, the walk's order and bounds, a drive-letter home, every row fact and the JSON |
| `workspaces_demos`, `workspaces_copy`, `workspaces_modules`, `workspaces_life` | the stamps, the mirror, every refusal's sentence in order, the playlists, the modules and the escape a module's text tried, links, a race, a failure midway, make and remove |
| `tests_workspaces` (the front) | the hook's order, the calls in order, a removal that outlives its client, the lock never held across a call, the memo, a failed join, Examples stocked at the account's start |
| `tests_wflight`, `tests_wstart` (the front) | the flights' keys and their life past a client |
| `workspaces_hostile` | a thousand workspaces, a root of dot entries, names of 16 KiB, a tree too wide or too deep, files at their limits, a demos folder of 10,000 files, a copy of 30,000 |
