# The song editor in Rust, base

`rust/` is the first modules of `song-edit` in Rust, after `src/` and the write side of `@studio/song`: the workspace's
k7x files read and written, the module folders, the confinement of a path, a chain project's item edits and the writers
of a graph and a module file. Nothing routes to it yet; the TypeScript stays the source of truth until the cut.

## Choices

| choice | why |
|---|---|
| every file function takes `&mut dyn Fs` (a reader `&dyn Fs`), `song::Source` plus the writes | the same code runs over the server's disk, the Worker's vfs and a `MemFs`; no `std::fs` appears in the crate |
| a path is absolute and spelled with `/`; `paths.rs` is `node:path`'s posix half | the host always gives absolute paths, and one spelling is what `song::Source` already requires |
| a Windows path (`C:\x`, `\\srv\share\x`, `\\?\…`) is mapped by `winpath.rs` at the host edge to `/C:/x` and `/UNC:/srv/share/x`; past it there is one posix algebra | a second algebra (win32 in `paths.rs`) would double every join, relative and confinement case; `..` cannot climb past a drive or share, names on a drive or share compare without case, a reference from a song file reads `\` as a separator and a drive switch, a device path, `C:x` or an NTFS stream is refused |
| `DiskSource` reads a UNC root as `//server/share/`, and `normalize` keeps it whole; `edge.rs` stays the one conversion point (`\\srv\share\x` is `/UNC:/srv/share/x` is `//srv/share/x`) | `song`'s `normalize` read a leading `//` as an empty name and collapsed it to `/`, so a share root lost a slash; making server and share part of the root also stops `..` at the share, as at a drive |
| `Env` carries what `studioHome()` and the shared folders read from the process | a crate that reads the environment cannot run in the browser, and a test could not set it |
| the sugar's `try_expand` returns the error and `Sugar::expand` falls back to the raw data | k7x's trait cannot fail; `ws_edit` and `k7x_data` call `try_expand` first, so the sentence still reaches the caller |
| `isDocFile` and `docPattern` keep their unescaped dot (`is_doc_file`, `sugar_kind`); `isK7xPath` keeps its escaped one | a name `x_k7x` is a document to the first two and not to the third, and the TypeScript's answers are the contract |
| the graph writers keep Zod's key order for the data they hand back to `apply_sugared`, and `{...file, k: v}`'s place rule | a key a graph lacks lands last, a key it has stays; the printer writes what it is given in that order |
| `read_module_file` returns the data as written, with `type: plugin` when nothing says otherwise | the Zod output's other defaults are not applied; the three modules a vector names carry none |

## Where it differs from the TypeScript

| difference | why |
|---|---|
| a graph that does not read fails with Zod 4's wording for an unknown key, a wrong type, an empty name and a port ref; other findings are approximated | only the first finding is shown; the vectors hold the unknown key and the port ref |
| `edit_chain_item_in` names the chain decoder's message after `the edit would make its chain unreadable —`, not Zod's path and message | `song::routing::decode_chain` has no Zod path |
| `read_module_file`'s issues are `<rel>:<line>:<col>: <message>` from `song::modules::decode_module` | the Zod wording is not reproduced |
| `MemFs::write_atomic` needs the folder to exist; an empty folder is not remembered by a tree | `writeFileAtomic` renames into a folder, and the vectors record files only |
| `Fs::list` is sorted bytewise; `module_io` re-sorts `[base, name]` pairs as UTF-16 strings, as the TypeScript does | a TypeScript `readdir` is in disk order, which no replay can pin |
| a `K7Path` index is a `usize` and a number-like `Key` is a key | the 3a rule |
| a graph `runs` run whose destination list holds a string, number or map fails with the JS `TypeError` text | `dests.push` is not a function on those; no real file has one |

## What is not here

`laneStrip`'s plan, its chain project branch, `createPictureLane` and `stripNewLane` are in `lane_strip_plan.rs` and
`lane_files.rs` (`rustNodeSide.md`); the other song-schema file writers (`trackFile`, `sequenceFile`, `videoTrackFile`,
`takeFile`, `curveFile`, `alternativeFile`, `mixFile`, `assemblyFile`, `playlist`) are each used by one module of the editor and
are ported with it.

## The proof

`rust/tests/replay/` replays the 3e vectors of these modules (a count per module is in the test's output). A `$TMP`
path in a vector's arguments names a folder the harvest could not record, so those vectors are skipped by name. The
harvest sorts every object's keys, so `ws_write_data` takes its key order from the file the vector expects. `rust/tests/goldens.rs`
replays `rust/tests/goldens/writers.json`, harvested from the TypeScript writers the tap did not reach (frozen: the
writer is gone) (the graph's, the module file's, the notes short form, `wsEdit` over short forms, the lane strip's
plain write, a chain item's edit).

The replays of `song-edit__projectEdit`, `song-edit__graphEdit`, `song-edit__rackTaps`, `song__assemblyEdit`,
`song-edit__alternativeSwitch` and `song-edit__engineSwitch` are the node side's; `rustNodeSide.md` says which vectors it
skips and why.
