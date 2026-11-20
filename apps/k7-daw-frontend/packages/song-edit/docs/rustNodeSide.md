# The song editor in Rust, the node side

`rust/` holds the node-side verbs of `song-edit` after `src/`: `projectEdit`, `graphEdit`, `rackTaps`, `tapChain`,
`alternativeSwitch`, `engineSwitch`, `assemblyEdit` and `presets` of `@studio/song`, with the module-file writers, the chain
project's reader and writers, the lane strip and the lane's files they need. The base is `rustBase.md`.
Nothing routes to it yet; the TypeScript stays the source of truth until the cut.

## What the other sides call

| file | what it holds |
|---|---|
| `chain_data.rs` | `Chain`, `Step`, `Link`, `Subchain`, `Start`: a chain file as plain `Value` items, in Zod's key order; `name_of`, `lane_of`, `set_key` |
| `chain_project.rs` | `read_chain_project` (chains, nodes, texts, issues), `read_layout`, `write_layout`, `write_chains`, `renamed`, `raw_items` |
| `format_chain.rs` | `format_chain(chain, prev_text)`: the chain as text, keeping the old text's header, comments and unchanged items |
| `chain_segments.rs` | `to_segments(project, pins)` over song's, and the graph file's text |
| `from_segments.rs` | `from_segments(GraphIn, FromSegments)`: segments back into chain files, with the caller's `item_of`, `is_source`, `name_for` |
| `chain_nest.rs` | `nest_subchains`, `regroup_calls`: derived chains back into their files |
| `chain_graph_edit.rs` | `edit_project_graph`, `edit_chain_graph` (`ChainGraphEdit`: `add`, `drop`, `rename`), `chain_node_name`, `node_arg`, `rename_chain_lane`, `delete_chain_item` |
| `graph_edit.rs` | the graph verbs; every one that writes takes `env` after `fs` |
| `tap_chain.rs` | `add_tap_chain_node`, `remove_tap_chain_node`, `move_tap_chain_node` |
| `lane_strip.rs`, `lane_strip_plan.rs` | the strip's names and file; `LaneGraph`, `plan_lane_strip`, `apply_lane_strip`, `add_lane_strip`, `strip_new_lane`, `strip_lanes` |
| `lane_files.rs` | `create_track_file`, `create_picture_lane_file`, `create_picture_lane` |
| `presets.rs`, `node_presets.rs` | the preset schema, text, paths and listing; `write_node_preset`, `apply_preset_to_node` |
| `ids.rs` | `NewId`, the host's source of node ids: a function that mints ids takes one |

## Choices

| choice | why |
|---|---|
| a verb takes `(fs, env, dir, …)` and reads modules through `loader.rs`, which builds `song::Roots` from the `Env` and calls `song::modules::load_nodes`, the vocabulary and `resolve_map` | the compile and the editor must resolve a node the same way, and `song` already owns that |
| the libraries are read as they stand, with no version pins | the pins are the host's; `Roots` is the one place a host can give them |
| a node's file is found from `LoadedModule::home` and `chain`, a shared one by the namespace of `Roots::module_roots` | one rule for where a module's text is, as `moduleFileOf` |
| a callback that must read the project while a file is open (`refuse_swap`) is run before the edit | the edit holds the file system mutably, and the project's files do not change under it |
| a chain project is read by song's decoder for validity and for `to_segments`, and by the editor into plain data for its own writes | song types a node's module fields, which a rewrite must hand back as written; one reader of the text each, over the same files |
| a chain edit builds the segments, runs the graph writer on their text and rebuilds the chain files from the result | the writers are the same for a `graph.k7x` and for chains, so a chain project needs no verbs of its own |
| a link built by `from_segments` writes `port` before `level`, a parsed one `level` before `port` | the printer keeps the order of the data it is given, and the TypeScript's two producers differ |
| a verb that mints a node id takes `NewId` | the crate reads neither a clock nor a random source |
| `strip_new_lane` and `create_picture_lane` say a strip that cannot be wired in the answer, never as a failure | a lane just made is kept whatever its run does |
| `set_clip_onsets` takes the corrections as the caller built them and `set_clip_warp` takes markers that may lack a member | the TypeScript writes what it is given, a missing member as `null` |

## Where it differs from the TypeScript

| difference | why |
|---|---|
| a chain project's issues are `<file>:<line>:<col>: <message>` from song's decoder, with its message, not Zod's path and message | `decode_chain` has no Zod path |
| a preset file that does not read names the first finding in Zod 4's wording for the ones a preset can hold: a wrong type, an empty name, a key out of range, an unknown key | the schema is a few fields; the rest is approximated |
| a graph the loader refuses reaches `strip_new_lane`'s answer as the first finding, or a count of wiring faults | song gives the faults as values, not as the TypeScript's sentence |
| an integer-like chain name is not sorted first among the chains, as a JS object would hold it | `IndexMap` keeps insertion order; no lane is named by a bare number |
| a library's root is the one the project reads as it stands, with no version pin | the pins are the host's, as in `loader.rs` |
| `rename_node_in_assemblies` names the decoder's finding after `assemblies:`, not Zod's message | `song::assemblies::decode_assembly` has no Zod path |
| the engine scan is not read: a module's engines come from the vocabulary's built-ins | the host gives the scan as a file mount; none is mounted here |

## The proof

`rust/tests/replay/` replays the 3e vectors. A vector is skipped, by name, when its project extends a module of the workspace's
`_modules/` or of a library (they stand outside `$DIR`, which the harvest records), when a name is NFD on APFS and a memory folder
does not fold it, and when the engine's scan decided the answer; every `presets` vector is of the first kind. A key order the
harvest sorted is taken from the file the vector expects, and the ids a call minted are taken from the file it wrote.
`rust/tests/goldens/chain.json`, harvested from the TypeScript and frozen, holds 79 cases over folders no
vector built: the graph verbs on five shapes of chain project (a line, a shared bus, a diamond, a subchain, loose chains and pins),
the lane strip and a picture lane, a tap's subchain, and the presets over a workspace with `_modules/`, `_presets/` and a
library. The replay compares the answer and every file of the temp folder after, byte for byte.
