# The file service's native projects (piece `projects`)

`POST /open`, `/project/rename`, `/project/delete` and `GET /project` in Rust. The crate is `apps/k7-daw-backend/packages/k7-file-service` (`projects/`: the marker, the open's and the moves' plans, the
lists); the front's half is `native/projects.rs` and `projects_open.rs`. The active project is the native side's own fact, written into its marker.

A row marked DEPARTS differs from the TypeScript's behaviour or from the spec. Rows 2000 to 2099 are the decisions, each
named by its number in a test or a line.

## The scope

| # | decision | why |
|---|---|---|
| 2000 | the routes are `POST /open`, `/project/rename`, `/project/delete` and `GET /project` | what moves the active project, and the poll that reads it |

## The marker

| # | decision | why |
|---|---|---|
| 2020 | With `projects` on, the native side is the one writer of `<studio>/workspace/.active-projects/<workspace>` for an open, a rename and a recycle, under the account's lock, and the account's open-project memory is set in the same lock (`activate`, `moved`) after the marker is on disk | the spec's rule, one reader per piece, never a second memory: every native reader (`Space::active`, the revision, the journal, the roots) takes the marker |
| 2021 | The format is the TypeScript's, byte for byte: `<rel>\n`, `""` for none; `rel` is 1b's `to_rel` of the project (`/`-spelled, `.` for a workspace that is a project, never `\\?\`, never canonical: `/private/var` stays `/var`); written through 1d's `write_atomic` (the temp `.<workspace>.<pid>.<8 hex>.tmp` in `.active-projects/`, the fsync, the rename retried on Windows), its folder made first. Vectors from the TypeScript's `writeActiveMarker` and read back by its `readActiveMarker`, both ways, for 300 rels (unicode NFC and NFD, astral, spaces, `C:/` roots, a lowercase drive) | a tray or a phone of an earlier release reads the same marker |
| 2022 | A marker is written only when its text would change: the native side reads it first, under the lock, and an equal text is left as it is; a project the memory already holds open (`isOpen`) leaves the marker untouched, whatever it says, and the open's `activeProject` is that project (`serviceOpen.spec.ts`'s stale-marker case) | the TypeScript writes it at every open that is not a repeat; the read spares a rename on the fast path and does not move its mtime, which 1f's listing excuse reads |
| 2023 | `projects.json` is frozen, harvested with node's path shim, never the host's: `toRel` of the project, the rename's cleaned name (`basename` then `[\\/]` dropped then JavaScript's trim), `projectApp`, the recycle bin's names. `projects_guards.json` is frozen too, harvested from a real `Workspace` on a fresh fixture per case: `renameProject` and `deleteProject` (the guards' order and sentences, the open project afterwards), `defaultProject` over marker bytes (UTF-8 or not), `projectInfoOf` over `project.k7x` texts, `readActiveMarker` over bytes that are not UTF-8 | the shim's `basename` reads `\` as `/` on every host; node's on POSIX does not |
| 2025 | The marker is never rolled back: once written, a failed `activate` leaves it, and the answer is the failure | marker and memory meet on the new project, where a rollback would part them |

## The open

| # | decision | why |
|---|---|---|
| 2030 | `default_project` is `defaultProject`: `.` for a workspace that is a project; else the marker as `readActiveMarker` reads it (trimmed, resolved, NOT confined to the root) when `isProjectDir`, spelled by `to_rel`; else the first of 1f's `project_dirs_under`; else none. `../other.k7` is then refused by `plan_open`'s gate, as the TypeScript refuses it | `Space::active` confines (1040), `defaultProject` does not, and the sentence a person reads differs |
| 2031 | `plan_open` in the TypeScript's order: the zod field (`path` non-empty, `libraries` an optional string, ignored; `body.rs`' messages); `safe_path` (its sentence); `project is in the recycle bin: <rel>`; absent (`stat` following links) is the "no project '<rel>' in this workspace — it may have been renamed…" sentence, whole; `not a project (the folder must end in .k7, …): <rel>` with `PROJECT_SUFFIXES`' values in their order. Vectors from the TypeScript's own `setActiveProject` through node for 200 cases | the order decides which sentence a stale link reads |
| 2032 DEPARTS | The answer is `{root, rootIsProject, activeProject, projects, activeIsDemo?, engineLoad, rev}` in that order: the info from the native side, `projects` walked afresh by 1f's `project_dirs_under` with `.` first for a project root, `activeIsDemo` by 1f's demo flag; `engineLoad` and `rev` from `startEngine`. DEPARTS: node answers its list from a 5-second memo, the native one is fresh. `root` goes through `live::as_node_answers` | a fresher list is no reader's loss (1212) |
| 2033 | The open's sequence: the first half (no lock; a 409 when `sync.ready` is false, `{error: "'<want>' is still syncing", sync}`, nothing else run); no `want` answers the info and `startEngine` with nothing set; else the account's lock, `plan_open`, the marker (2022), `activate`, the lock let go, `startEngine`, the info walked, the answer | the TypeScript's order: the libraries and pins, the sync gate, the guards, the marker, the repairs, the engine, the list, the token |
| 2034 | Past its plan, an open runs to its end in a task of its own (1216): the marker, `activate`, `startEngine`; the request awaits it. The client's 180 s (`OPEN_MS`) does not bound it | a client leaving after the marker would otherwise leave the marker on a project the engine never started |
| 2035 | A name that names a project folder only by its case is answered as 2092 rules | — |

## What the open project decides

| choice | why |
|---|---|
| the tree walks the workspace root, not the project, and paths are workspace-relative | what it shows is what a person can choose from, and the ▶ on a project row is how the engine's project is picked |
| a file of a project that is not playing is a clip by every pattern test, so an id looked up in the active project's compile answers wrongly; opening it gives its text | it is a file of this repository; the editor gates its minimap and clip panel on membership of the active project |
| the shared `_modules` is visible and editable in place, and a `.dawproject` outside a project is on the tree and can be imported | a write there is what every project in the workspace sees; an import lands a project at the root where the tree looks |
| `isProjectDir` requires a directory and the existence check runs before the name rule, saying what happened | a name rule says yes about a folder that does not exist (a rename racing a stale listing), and the compile's "unsupported song" names none of the real problem; a stale marker must not be restored |
| `POST /project/create` takes an archetype (`arrangement`, `mix` or `video`) and adds the `.k7` suffix to a bare name | every project is a `.k7` folder, so one field asks one question; a folder without the suffix is one no listing shows and no open takes; a caller newer than the service gets a 400 naming the enum |
| `POST /module` writes `<root>/<type>/<name>/module.k7x` and a `root` the listing did not answer is a 400 | a module is a folder with a definition; any other folder takes a file no loader reads, written wherever a caller names |
| `/project-plugins` takes the project in the query, listing internal and external plugins each once; `/project-compat` scores the list against this machine's scan and answers null for an empty scan | the engine gate asks before any engine is chosen; on a server the scan is whichever engine the hub bound last; null cannot say, and is not "compatible with nothing" |
| a plugin baked into a render in force counts in both | the render lets an engine without it play the track but not edit it, so that engine keeps its loss |
| `/module-libraries`, `/library-chains` and `/playlists` are beside `/tree`, and only the listing of playlists is a route | `/tree` is the song; reading and writing a set list are `/file` and `/write`, and a second pair would be two ways to write one file |
| a missing entry is a 404 `no such file or folder: <path>`, a folder where a file was asked a 400, and a file system's own message is logged and never answered | the message spells the server's absolute paths |
| `POST /auto-freeze/thaw` leaves a VST live for the session in memory, never in the song, and the scheduler ends it | the baked VST has no plugin a knob could reach |

## Rename and recycle

| # | decision | why |
|---|---|---|
| 2040 | The sequence: `join`; the plan; `releaseFolder` (no lock, no gate); the journal's gate alone (2061); the account's lock; the plan again, its answer the one that counts; the move (1b's `rename_with_retry`; the bin's first free `<base>`, `<base>-2`… by 1b's rule); the marker (a rename of the open project or of an ancestor of it re-points it, a recycle of the open project empties it); `moved`; 1c's `Revisions::forget(<the old folder>)`; 1f's memo dropped; the lock and gate let go | the TypeScript's order (`cancelPrewarm`, the watches, the unload, the move, the marker, the memory), with the unload outside the lock as 1624's `letGo` |
| 2041 | `plan_rename` in the TypeScript's order: `safe_path`; the root (`can't rename the workspace root`); 1b's `guard_shared(…, "rename")`; the bin (`'<rel>' is already in the recycle bin`); not a project; the cleaned name (2023) and the suffix's sentence whole; the dest through `safe_path` of `to_rel(join(dirname, clean))`; the same path answers `{path}` with nothing done; an existing dest `already exists: <rel>` | every sentence is the specs' or the vitest's |
| 2042 | `plan_recycle` in the TypeScript's order: `safe_path`; the root (`can't delete the workspace root`); the bin (`already in the recycle bin: <rel>`); with 2094, `guard_shared(…, "delete")`; not a project. `was_active` is the marker's project equal to the folder, exactly (`===`), where a rename's is the folder equal to or an ancestor of it (`isWithin`) | the TypeScript's two rules, kept apart on purpose: the gap below |
| 2043 | Past its plan, a rename or a recycle runs to its end in a task of its own: `releaseFolder`, the move, the marker, `moved`, `forget` | a client hanging up after the unload would leave the engine stopped and the folder where it was; after the move, a marker naming a gone folder |
| 2044 | `releaseFolder` failing (a throw of the unload) is the route's 400 with its message, nothing moved; `moved` failing after the move costs nothing of the answer, logged `[files] native: moved failed: <what>` | the TypeScript awaits the unload before the move; the move happened and the person must hear it (1214) |

## Concurrency

| # | decision | why |
|---|---|---|
| 2060 | 1b's account lock (1210 DEPARTS) covers an open's marker and `activate`, and a rename's or a recycle's move, marker and `moved`; it is never held across `openPrepare`, `startEngine` or `releaseFolder`, and no ask waits on it. the repairs inside `activate`, a verb and the capture take no lock, as 1210 rules | the marker and the memory must change together against every other native write of the account; holding it across `activate` cannot deadlock; an unload or a compile must not hold every rename of the account |
| 2061 DEPARTS | A rename and a recycle take 1d's journal gate of the workspace alone (`Journals::alone`, an undo's hold) after `releaseFolder` and before the account's lock (1733's order), until the marker is written: no record is in flight on the folder while it moves. The TypeScript lets a verb's record (one awaiting the engine) span a rename, and its after-image then reads every file of the project deleted: its undo writes the old folder back. 2090 asks the user to fix the TypeScript the same way | an undo that resurrects a renamed project is the costliest wrong step there is; the gate already serializes undo against records |
| 2062 | An open takes no journal gate: a record holds its project's folder from its begin (1d's `begin {root, dir}`), so a change of the active project during a record changes no step | 1725: the next use reads the other journal |
| 2063 | An open, a rename and a recycle drop 1f's listing memo of the account | its `active` flags and its projects changed, and a project moved below a workspace moves no mtime 1f reads (1670) |
| 2064 | Two opens of one account at once are served in the lock's order; the second's `startEngine` may run before the first's: the engine serves the last project set, and each answer names the project active when its info was walked | the native lock serializes what writes, and the last open wins either way |

## `GET /project`

| # | decision | why |
|---|---|---|
| 2070 | `project_info` is `projectInfoOf(active ?? root)`: `name` from `project.k7x`'s `project.name` when a string (the `k7x` crate's reader, the text as `wsData` reads it), else the folder's basename; `root` the folder, spelled as node spells it; a text the Rust reader declines is `Declined(LossyText)`; the active project from `Space::active` | the poll every tab runs every 2 s; computed in 0.05 ms and waiting behind no verb |

## Rulings pending, built on the recommended side

| # | decision | why |
|---|---|---|
| 2096 | a rename behind a record waits for its lease, with no 409 | the TypeScript's own `exclusive` waits without end, and a hung record is abandoned by the lease (1745) |
| 2097 | renaming a folder that holds the open project keeps the inner project open at its new path, in both languages | `o2.k7/inner.k7`, not `o2.k7`, which the TypeScript chose by accident |
| 2098 | an admin's recycle of a library's project goes into the library's own bin, in both languages | the project must not leave the library for every account |
| 2100 DEPARTS (U4) | a rename or a recycle whose marker write fails after the move answers `200` with the move's body, logs `[files] native: marker write failed after the move`, and counts the failure as `io` | the folder moved, and a refusal would tell the person it did not; the open's 2093 refuses before anything is moved |

## Decisions the user ruled

| # | decision | why |
|---|---|---|
| 2090 | a rename or a recycle runs inside `history.exclusive` of the workspace in the TypeScript, and takes the journal's gate alone natively | a record open on the project would otherwise journal every file of it as deleted, and an undo would write the old folder back beside the renamed one |
| 2092 DEPARTS | a project path naming a folder only by its case is resolved to the folder's own spelling before an open, a rename or a recycle, and that spelling is what the marker, `activate` and `releaseFolder` carry | a project open under two spellings is two journals and an engine never released; the TypeScript keeps the typed spelling |
| 2093 DEPARTS | a marker that cannot be written fails the open with `500 {"error": "the file could not be changed (<CODE>)"}` and nothing is asked of `activate` | the marker is every native reader's truth, so a failed write must not part the journal and the engine from the project on screen |
| 2094 | `deleteProject` runs `guardShared(..., "delete")` after the bin's check, and the native side the same | an account that may not edit a library must not remove part of it for every account |
| 2095 | rename and delete answer an fs failure through `entryFailure`, without the server's path, which is logged | a server's path is logged and never answered |

## Gaps the native side keeps

| gap | state |
|---|---|
| the spelling lookup folds ASCII only | `É.k7` against `é.k7` on APFS keeps the typed spelling; each segment's lookup reads its folder once, counted and bounded by `Limits` |
| a recycle of an outer project whose nested project is open | the marker keeps naming the nested one, now in the bin; the next open drops it |
| a rename's journal | the renamed project's journal is another store |
| no clobber is a check, then a rename | a folder made in between is replaced, on both sides |
| a rename and an open of the same project racing | the rename's unload, the open's engine start, then the move: Windows answers the handle's code after 1b's retries |
| an open the plan declines hands the request back after `openPrepare` ran | the live router refreshes the libraries and pins once more (pins `fresh: false`, one more hub ask) |
| a native undo running alone (2101) | the rename takes the journal's gate alone, so it waits for the undo (2061) |

## Risks

| risk | answer |
|---|---|
| the marker and the memory part | one writer, one lock, never rolled back (2020, 2025) |
| a rename waits on a long verb's record | the lease abandons a hung record (1745) |

## The proof

`tests_ptwins.rs` replays tables of cases and 32 seeded sequences of 12 steps on a real disk, including a case-only open that departs by 2092; `serviceOpen.spec.ts` pins the open's whole body, the marker's bytes after each move and the repeat open writing nothing.
