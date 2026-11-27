# The k7x editor and printer

`src/edit/` is k7x's document editor and printer: values set, added, moved and deleted by key path,
every other byte kept. The backend, the verbs and `song-edit` call it in Rust; the page calls it
through the browser module (`k7x.edit`, `k7x.print`), behind `@studio/k7-wasm/k7x`'s `K7Document`.

## Choices

| choice | why |
|---|---|
| the document holds an OWNED tree (`tree.rs`), rebuilt from the reader's arena tree after each splice | the arena tree borrows the text, and a splice replaces the text; an owned tree lets a writer hold the old nodes while it splices |
| offsets and splices are bytes; a width and a column are UTF-16 units | the printer wraps at `used + one.length <= 100` in JS units: `é` is two bytes and one unit, an emoji four bytes and two |
| a `Change` carries both spans | an editor dispatching the splice counts UTF-16; Rust slices by byte |
| `\s`, `trim` and `trimEnd` are `value::is_js_space`, not `char::is_whitespace` | JS counts U+FEFF and U+00A0 as whitespace and not U+0085, and the vectors were written by JS |
| `Value::object` orders keys as a JS object does | a key like `"2"` lists before `"b"`, and `apply_data` deletes and sets in that order |
| `js_number` is ECMAScript's `Number::toString` over Rust's shortest digits | `String(1e21)` is `1e+21` and `String(1e-7)` is `1e-7`; `{}` would print neither |
| `lineStart` keeps `lastIndexOf("\n", -1)`'s answer for position 0 | a text opening with a newline gets 1 there; no node starts at offset 0 of such a text, so no vector reaches it |
| an index (`insert_in`, `move_in`, a `PathSeg::Index`) is a `usize` | the page refuses a negative or fractional index with the editor's own sentence before it crosses |
| `apply_sugared` asks `present` and `expand` of the `Sugar` it is given; the page's short forms are applied by song-schema's `src/yaml/sugared.ts` over the same document | the expansions the page uses are song-schema's TypeScript until `song-edit`'s writers join the module |

## On the page

| choice | why |
|---|---|
| `k7x.edit {text, ops}` runs a batch on one `Document` and answers the text, the changes in UTF-16 units and each op's answer; a failed op stops the batch and the ops before it stay | a writer that calls the document op by op saw exactly that; a gesture that knows its ops sends them as one call |
| `K7Document` sends each write as a one-op batch | the writers read between their writes (`getIn` after `setIn`), so a write is applied before the next read |
| a read (`getIn`, `hasIn`, `toJS`) comes from the text's data, fetched once per text; `getNode`, `getSource`, `positionOf` from its outline | the data is the cheap answer (`rust.md`); the tree is only built when a span is asked |
| a value that JSON cannot spell (`NaN`, `Infinity`) is refused before it crosses, with the printer's sentence | `JSON.stringify` would turn it into `null` silently |
| the facade's classes use no parameter properties | node's type stripping refuses them, and the engine's scripts run the facade with plain node |

## The proof

`tests/vectors/` is frozen: the TypeScript editor and printer wrote it before they were deleted, and
no generator writes it any more. `scripts/check-vectors.sh` replays it in `ci:local`.

- `edit.json`: curated texts (CRLF, BOM, tabs, NFD, astral, comments, comma-first, chains) with every op, strings and
  numbers at the width where bytes and units disagree, the sugar double, and seeded random chains in three layouts.
  Each result is the text after the op or the error message; a chain also records its `Change`s. `tests/edit_vectors.rs`
  replays it on the crate, and `k7-wasm`'s `tests_k7x.rs` replays it as `k7x.edit` batches, so the page's op shapes are the vectors' own.
- `corpus.json`: every `.k7x` file under `demos/` and `k7-core/` with about 60 ops generated from its tree and the
  printed data, recorded as one hash per file. `tests/corpus.rs` skips a file edited since the freeze and requires most
  of the corpus to replay; `K7X_VECTORS_DEBUG=<file>` prints each op's result.

`tests/unicode.rs` states the width rule on its own.
