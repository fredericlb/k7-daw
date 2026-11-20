# The workspace's and the libraries' listings, read natively 

What a page draws without compiling anything, answered by Rust: the workspace's playlists and
reference tracks (`GET /playlists`, `/references`), the libraries' chains and assemblies (`GET /library-chains`,
`/library-assemblies`) and the module levels the editor's library tab and the rack builder draw (`GET /module-libraries`).
It stands on [file-native.md](file-native.md): the path model and the limits are 1a's. The crate is
`crates/k7-file-service/src/listings/`, the front's half `crates/k7-front/src/routes/files/service/native/listings.rs`.
A row marked DEPARTS differs from the TypeScript's behaviour or from the spec; a row marked USER is the user's to rule,
and is taken on the side it recommends until then.

## What it decides

| # | decision | why |
|---|---|---|
| 1403 | `/library-chains` and `/library-assemblies` are 1e's | two walks of the libraries' families, the walk the account's chains will stand on |
| 1410 | A playlist: the text read whole as UTF-8 with replacement, a BOM kept, at most `max_text`; the k7x reader both sides share; a text that does not parse is `{count: 0, ok: false}`, a lone surrogate escape is `Declined(LossyText)`; the title trimmed as JavaScript trims, absent when empty; `count` the lines `playlistFromData` keeps; an unreadable file, or one that stopped being a regular file, is `ok: false` | `playlistsIn` |
| 1411 | `files_below`: the folder asked is followed when it is a link; below it a dirent's own type decides, and a link is neither folder nor file, a Windows junction included; nothing is hidden; a folder that cannot be listed is no files; a name holding `\` is joined as the shim joins it; an explicit stack; every entry counts against `max_seen`, the depth against `max_depth`. Entries are visited in byte order, as `readdir` lists them on Unix, so two rows with one path keep the TypeScript's order; on Windows no two rows can share a path, since no name holds `\` | the referee's walk; the TypeScript recurses without a bound |
| 1412 | A reference: the extension after the last dot not at the start, compared without case against the ten; `name` the base name before that dot; `bytes` a `stat` that follows links, 0 when it fails | `referencesIn` |
| 1413 | The folders are found as the TypeScript joins them, `<root>/_playlists` and `<root>/_references`, and spelt so in every path | the answer spells the join, not the disk |
| 1415 | The libraries are the folders of the libraries' home whose name passes `is_library_name`, by a dirent's type, sorted; each root by 1a's `library_root`. A family's roots: the account's library first, then those, each `<root>/<kind>`, kept when a `stat` that follows links finds it | `accountLibraries` and `libraryFamilyRoots` |
| 1416 | `library_chains`: a folder holding a file `graph.k7x` is a folder chain when its path is not empty, and the walk stops there; else each folder entered and each `.k7x` file (any case) a file chain; a folder chain's `nodes` its k7x files in `.sort()`'s UTF-16 order; one ref held both ways is the file's; the whole in code point order by ref | `libraryChainDirs`; the two orders differ above U+FFFF |
| 1417 | `assemblies`: per family root each folder in UTF-16 order, entered when `stat` following says folder; a folder holding `assembly.k7x` is one and nothing inside it is another; the list is not sorted again | the TypeScript's walk |
| 1418 | Where the TypeScript throws, the native listing declines: an assembly walk's listing or `stat` failing (a dangling link) is `Declined(WouldThrow)` | parity without a departure |
| 1420 | The folder the levels are read from is the open project or the root; a library as the open project is declined | the native side reads the marker |
| 1421 | The levels: the shared `_modules` beside the project when it exists, then the `modules` family roots not already listed, then every library not yet a row at `<root>/modules`; a row's `ns` is the library the folder lies in, else `workspace` | `moduleRootsOf`, in its order |
| 1422 | A row: `exists` by a following `stat`; `nodes` the tree of its folder, every path spelt by the row's `to_rel` once; a library of the account's (not the account's own, not the workspace's) gains `samples`, `irs`, `wavetables`, `models`; one `seen` counter and one byte budget across the whole listing; a test holds the lookups of `library_of`, counted per thread, to a handful for 600 entries | a limit weighs the listing, not each tree; a lookup per entry is a defect |
| 1423 USER DEPARTS | A folder of a level that cannot be listed costs that folder (`children: []`), where `fileTree` throws and the route answers 500; `Listed` names it. Taken on that side; otherwise the listing declines | an unreadable library folder empties the editor's whole library tab |
| 1424 | The engine's row: the song crate's `Scan::read` of `<studio>/exts.json` through a source that mounts that file alone, `engine_modules`, grouped by category in code point order, a leaf for each module; the built-ins stand in for a scan that cannot say. Where that reader would say less than the TypeScript, the row is declined: a link as `exts.json` (`NotRegular`), and a scan read empty from a file large enough to pass the song crate's 20,000 plugins (`TooMany`) | the compiler's own port of `engineModules`, so the two cannot drift |
| 1430 | `versions_at`: 1a's pins; per library `pinned` the pin or 1; a numeric pin reads its manifest and a `library.k7x` that reads badly is no manifest; `requires` set when the manifest reads; `upgrade` the newest local version that this k7's level runs, when above the pin; a draft pin has neither. The library's folder is listed once for its versions, not once a version as `libraryVersionRoot` does. A test holds the level and the unpinned version equal to `libraryVersions.ts` | `libraryVersionsAt` |
| 1440 | The four ops answer through `reads`' one `compute_with`, and `/module-libraries` answers natively or is refused | one answer per route |
| 1441 | A live route's context is built only when the workspace header passes `workspace`'s checks and names a folder; otherwise the route answers its error | node's `resolveWorkspace` spoke first on a bad header |
| 1442 | Answering holds 1057's weight: a level listing, the chains and the assemblies weigh a tree, a playlist listing 4 MiB grown by three times each text before it is read, without waiting, the references 4 MiB | the walks are trees; a playlist's text is the one thing held per row |
| 1443 | The answer is sent as its text | the levels nest the trees one level deeper |
| 1460 | `listings.json` is frozen: 36 scenarios built in a temporary folder, the TypeScript's own `playlistsIn`, `referencesIn`, `libraryChainsOf`, `libraryAssembliesOf`, `moduleLibrariesOf` and `libraryVersionsAt` run over them once, and the 180 answers kept as text; the Rust test builds each scenario and compares the text, so the keys' order is held | the TypeScript is the referee, not a reading of it |

## Hostile input

Each answered within a bound a busy machine keeps (15 s, 60 under musl, as the crate's other hostile tests) on the best of three: 20,000 references, a nest
300 deep (declined, and the stack holds on a 256 KiB thread), a link loop below the folder, 2,000 playlists and one of
100,000 lines, a 16 MiB playlist and one a byte past `max_text`, 2,000 chains and 200 libraries, a sound family of
10,000 samples, an `exts.json` of 8 MiB, 2,000 version folders. Where a walk would turn superlinear by a lookup of
`library_of` per entry, the references, the chains and the sound family count those lookups too, which no machine's
speed hides.

## Gaps the TypeScript leaves open, which the native side copies

| gap | state |
|---|---|
| `_playlists` or `_references` a link out of the workspace | its files are listed and named inside it, on both sides |
| an unreadable playlist | `ok: false`, the same as one that does not parse |
| the version rows ask the hub once per library, every listing | the native side asks the same |
| two plugins whose names collapse to one ref | the first wins, in each side's order of the scan |

## Risks

| risk | answer |
|---|---|
| the chains' sub-step (1400) | it stands on `family_roots` and `library_chains`; nothing here resolves a module |
