# The song editor in Rust, lane and module side

`rust/` holds, beside the base (`rustBase.md`), the writers of a lane, a chain project's nodes and the module libraries:
base presets, timeline rename, chain node and lane creation, the preferences file, a module's move, the account's forks,
the library forks, the engine module's fork, a new project's files, the mixer's placement writers
(`track_mix`, `placement_move`) and the timeline cut from a lane's range. Nothing routes to it yet; the TypeScript stays the source of truth until the cut

## Choices

| choice | why |
|---|---|
| a fresh node id comes from a `NewId` closure the host gives (`ids.rs`) | the crate reads no random source; a replay hands the ids the harvest saw, in the order the TypeScript made them |
| `shared_folders.rs` takes the places from `Env` and reads pins through the `Fs` | the TypeScript reads `$STUDIO_HOME` and the process's overlays; the crate may read neither |
| a pin file in the old YAML spelling is not read or upgraded | the upgrade writes and deletes files through YAML; it belongs to the native server, so a workspace with only that file answers no pins |
| `preferences_file.rs` answers a leftover `preferences.yaml` with the "does not read" problem, never with defaults | a mutator that saved over it would lose what the file holds |
| channel numbers in the preferences stay `f64` | the TypeScript keeps JavaScript numbers; `Number("0x10")` and `Number(null)` are what a hand-written file may hold, and `js_text.rs` spells them |
| `module_fork` takes the engine catalogue and the panel's text from the host | the loader and the panel library stay TypeScript; the host reads them and the crate writes what they say |
| `scaffold_files.rs` and `scaffold_live.rs` hold the chain files' text, ids marked `<idN>` | the golden replay of `scaffold` keeps the text and the order the ids are minted in |
| the sample video of the `video` shape is a byte slice the host gives | the crate bundles no binary |
| the library forks' `module.k7x` test keeps the TypeScript's unescaped dot (`module` + any character + `k7x`) | a file named `module-k7x` is a module file to the TypeScript, and its answers are the contract |
| the library versions' digest is SHA-256 over each file's relative path, a NUL and its bytes, in UTF-16 name order | the digest is compared and never stored, but the same walk keeps the two sides' answers equal |

## Where it differs from the TypeScript

| difference | why |
|---|---|
| `gain_to_db` and `db_to_gain` use the platform's `log10` and `pow`, which differ from V8's in the last digit for about one `log10` in five and one `pow` in six hundred | V8's own functions are not the same on every machine (the vectors' two misses are one digit off), so the replay accepts four ulps for these two alone; a value written to a file from them may differ in its last digit |
| `repoint_refs` lists the files it changed in directory order, which is bytewise | a TypeScript `readdir` is in disk order |
| `insert_chain_input` names the chain decoder's message after `the input would make … unreadable —`, not Zod's | as `edit_chain_item_in` in `rustBase.md` |
| the preferences' `find` reads an unreadable file's cause as `EIO` whatever the error | `Fs` has no errno |

## The proof

`rust/tests/replay_lane/` replays the 3e vectors of these modules (`basePreset`, `timelineRename`, `chainCreate`,
`preferences`, `trackMix`, `timelineFromRange`) and the goldens of the modules the tap did not reach
(`preferences`, `moduleMove`, `accountFork`, `libraryForks`, `moduleFork`, `scaffold`, and the takes, seconds, alternatives
and timelines of `trackMix` and `timelineFromRange`), harvested into `rust/tests/goldens/lane/` from the TypeScript and frozen. A golden
file has the 3e vectors' shape, so the one replay reads both; a vector the harvest recorded without the library
versions (`forksWhoseBaseMoved`'s) is skipped by name and covered by a golden.
