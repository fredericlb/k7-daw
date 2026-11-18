# The song editor in Rust, time side

`rust/` holds the modules that move, cut and loop material in time, after `src/` and song-schema's `trackFile`,
`takeFile`, `curveFile` and `timeSplice`: `materialCut`, `materialWindow`, `clipResize`, `crossfade`, `eventGain`,
`eventLoop`, `takeEdit`, `insertTime`, `sequenceLoops`, `songTempo` and `onsets`. Nothing routes to it yet; the
TypeScript stays the source of truth until the cut. The shared base is `docs/rustBase.md`.

## Choices

| choice | why |
|---|---|
| the planners (`material_window`, `clip_resize`, `crossfade`, `onsets`) are pure functions over plain structs | they read no file, so the same code serves the editor in the browser and the server |
| the lane writers take the project meter as `(num, den)` floats, as the TypeScript's `parseInt` can state | a `3/8` or a `6/8` header reads the same through both |
| `track_file` edits a `Scope`, `track_edit::edit_track_struct` opens the lane's file and hands it the root | one place knows where a lane lives: `nodes/`, a chain project's `tracks/`, or a timeline's lane |
| an event's list is the playing one, and the first edit on an alternative that inherits forks it | a lane on an alternative has two lists, and the index found in one must edit the same item in the other |
| a splice is a `SplicedList` trait over one list of positioned items | lanes, curves and video lanes differ only in where a position is read and how a move is written |
| `insert_time` holds every document and writes only when all are made | a splice that stops midway shifts the song apart |
| `project_root` checks the root's shape by hand, in Zod 4's wording | the loader's one use here is the first finding and the empty root on a bad shape; `song`'s own decoder refuses `type` and `project_type`, which the TypeScript accepts |
| `to_fixed6` rounds a tie up in magnitude | `toFixed` breaks ties upward, where Rust's formatting goes to even, and a note at `1/128` beat sits on one |
| `js_round`, `js_to_number` and `js_string` stand for `Math.round`, `Number` and `String` on a plain value | an address, a window edge and a loop count are read the way the TypeScript reads them |

## Where it differs from the TypeScript

| difference | why |
|---|---|
| a project root that does not match its schema reports the first finding only, in a wording checked for the keys, types and markers the vectors and goldens hold | the Zod message is not reproduced for every refinement |
| `halve_loops_of` takes its snapshot as `Option<&mut dyn FnMut(&str)>` | a Rust caller cannot pass a bare function that mutates |
| a `NaN` argument is not given the TypeScript's comparison results everywhere | `f64::max` and `min` skip a NaN, and the hosts pass finite numbers |

## The proof

`rust/tests/replay/` replays the 3e vectors of these modules, byte for byte on the files they change. The four `halveLoopsOf`
vectors the harvest marked as carrying a callback are not replayed; the goldens run the same calls with a snapshot that
records what it saw. `rust/tests/goldens/time.json`, harvested from the TypeScript and frozen, holds
the cases the tap does not reach: alternatives, seconds starts, chains, takes and comps, markers, curves, a root that does not
read, the tie in `toFixed`; each is compared on its answer, its error and every file of the folder after.
