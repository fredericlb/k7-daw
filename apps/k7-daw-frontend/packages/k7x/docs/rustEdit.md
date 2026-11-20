# The k7x editor and printer in Rust

`rust/src/edit/` is `src/edit.ts`, `write.ts`, `remove.ts`, `applyData.ts`, `layout.ts`, `print.ts`,
`scope.ts` and `sugared.ts` in Rust: the same splices, byte for byte. No host calls it yet; the
`song-edit` crate builds on it. The TypeScript stays the source of truth
until that crate replaces it.

## Choices

| choice | why |
|---|---|
| the document holds an OWNED tree (`tree.rs`), rebuilt from the reader's arena tree after each splice | the arena tree borrows the text, and a splice replaces the text; an owned tree lets a writer hold the old nodes while it splices |
| offsets and splices are bytes; a width and a column are UTF-16 units | the printer wraps at `used + one.length <= 100`, and `length` counts UTF-16: `é` is two bytes and one unit, an emoji four bytes and two |
| a `Change` carries both spans | an editor dispatching the splice counts UTF-16; Rust slices by byte |
| `\s`, `trim` and `trimEnd` are `value::is_js_space`, not `char::is_whitespace` | JS counts U+FEFF and U+00A0 as whitespace and not U+0085 |
| `Value::object` orders keys as a JS object does | a key like `"2"` lists before `"b"` in the TypeScript, and `applyData` deletes and sets in that order |
| `js_number` is ECMAScript's `Number::toString` over Rust's shortest digits | `String(1e21)` is `1e+21` and `String(1e-7)` is `1e-7`; `{}` would print neither |
| `lineStart` keeps `lastIndexOf("\n", -1)`'s answer for position 0 | a text opening with a newline gets 1 there; no node starts at offset 0 of such a text, so no vector reaches it |
| `Value` has no undefined and a string is well-formed | the host drops an undefined key as `JSON.stringify` does; a lone surrogate escape reads as U+FFFD and `Document::lossy` says so |

## The proof

`scripts/makeEditVectors.mts` runs the TypeScript and writes `rust/tests/vectors/`:

- `edit.json`: curated texts (CRLF, BOM, tabs, NFD, astral, comments, comma-first, chains) with every op, strings and
  numbers at the width where bytes and units disagree, the sugar double, and seeded random chains in three layouts.
  Each result is the text after the op or the error message; a chain also records its `Change`s.
- `corpus.json`: every `.k7x` file under `demos/` and `k7-core/` with about 60 ops generated from its tree and the
  printed data, recorded as one hash per file. The op list is generated twice, in `makeEditVectors.mts` and in
  `rust/tests/corpus.rs`; `K7X_VECTORS_DEBUG=<file>` prints each op's result on both sides.

`scripts/check-vectors.sh` regenerates both and fails on a difference, which includes an edited corpus file.
`rust/tests/unicode.rs` states the width rule on its own.

## Where it differs from the TypeScript

| difference | why |
|---|---|
| an index (`insert_in`, `move_in`, a `PathSeg::Index`) is a `usize` | "not an integer" and "negative" are errors of a JS number; the host validates before it crosses |
| `apply_sugared` asks `present` and `expand` of the `Sugar` it is given, and never walks the prototype chain | the TypeScript's `part.target in raw` is also true for `toString`; no real kind has such a key |
| an `undefined` value does not exist (see the table above) | the host drops it |
