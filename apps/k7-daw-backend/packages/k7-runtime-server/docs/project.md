# The project and transport verbs

`src/doc/` reads a `ProjectDoc` off the wire; `src/server/project.rs` and `transport.rs` answer the verbs; the
engine's side is the `project` and `transport` areas of the ABI (`abi.md`). The plan is
`docs/rust-engine-server-plan.md`.

## Rust validates, C++ reads

The document is parsed, bounded, deduplicated and path-checked on the socket's task, then handed to the engine as
JSON that `engine/DocReader.cpp` reads with no check of its own. The diff and the apply stay C++.

- **The wire is a trust boundary.** The engine binds every interface under the supervisor, so a number Tracktion
  divides by or loops over is bounded where it arrives, and nowhere after. A reader that also checked would be a
  second table that can drift from the first.
- **Every field is written, defaults included, and read strictly.** A field the writer forgot is a throw on the C++
  side, not a default that plays silently. `fixtures/project.validated.json` is the one file both sides are held to:
  `tests/all/doc_bounds.rs` writes it, `tests/DocReader_test.cpp` reads it.
- **A value past a bound is replaced, never refused.** A project with one wild number still plays; only a value of
  the wrong type is refused, with nlohmann's own words, since a client may show them.
- **A path is checked by the engine's roots, not a copy of them.** `path_allowed` is an ABI call, so the roots
  the server sets stay one table; a path outside is dropped with a warning and remembered for a `render` to refuse whole.

## The bounds

| field | held to | otherwise |
|---|---|---|
| time signature | numerator 1..64 over a power of two up to 32 | 4/4 and a warning |
| tempo | finite, above 0 | 120 |
| sample rate | 8000..768000 | 48000 |
| note key, rack note, tap note | 0..127 | clamped |
| pad `repeat` and `skip` | 0..64 | clamped |
| output pair | 0..31 | clamped |
| input and output channels | positive integers | the others dropped |
| ids of tracks, plugins (one set with the master chain's), clips (per track) | unique | `#2`, `#3`, and a warning |
| a read path, a preset save path (`.vstpreset` under the write roots), the record folder, an `@instruments/` reference (no `..`) | the engine's roots | emptied and a warning |

The count-in is not bounded here: Tracktion clamps it.

## Choices

- **Supersede is a ledger over the message queue.** A newer `project` skips every document job queued since the last
  fence; a `tracks` delta skips only one naming the same tracks; any other verb of the session is a fence. A job is
  skipped, never reordered, and the ticket is taken and the job posted under one lock.
- **`tracks` is merged in Rust.** The engine says which ids it holds; Rust picks each delta track's position and the
  engine replaces by position and applies. A track the project does not hold is a warning, not an append.
- **A keyboard's held notes are followed per socket**, only for the messages the engine will play, and released on
  close: a tablet losing Wi-Fi never sends its note-off.
- **A list field that is neither an array nor null is refused**, where nlohmann walked an object's values.
