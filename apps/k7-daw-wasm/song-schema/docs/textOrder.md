# Text order

Code point order is the one order the compile path and the file service sort names in: `song`,
`song-edit`, `song-schema` and `file-service` compare strings with Rust's `str` order wherever they sort a name, a ref, a
path or a key. None of them asks a locale.

## The choices

| choice | why |
|---|---|
| one written-out order instead of a locale's | `localeCompare` is ICU's root collation in node, the user's locale in a browser, and code points in Rust. A sort that feeds a compiled document or a listing must give the same bytes wherever it runs |
| by code point, not by UTF-16 unit | code-point order is UTF-8 byte order, which is what Rust's `str` comparison gives with no table. UTF-16 order (`<`) differs only for a character above U+FFFF against one in U+E000–U+FFFF, which is enough to make the two runtimes disagree |
| no case folding, no numeric segments, no accent levels | no call site asked for any (none passed options to `localeCompare`), and each would be a table or a rule to keep identical in every runtime. A human-facing view that wants `Alizée` beside `Alain` sorts again in the browser, with `@studio/web-protocol/tree`'s collator (`apps/k7-daw-frontend/docs/listOrder.md`) |
| `0` only for the same string | the order is total, so a sort's result never depends on the input order or on the engine's sort algorithm |
| it is Rust's own `str` order | no table to keep: UTF-8 byte order is code-point order, and a page sorts the services' listings again for people (`apps/k7-daw-frontend/docs/listOrder.md`) |

## What the order changes

Against ICU's root collation, a mixed-case or punctuated list orders otherwise: upper case before lower
case (`FM Bass` before `Fingered`, `WT Bass` before `Wide`), then `_` between the two cases, `-` and digits
before letters. Where it shows:

- **the compiled documents**: in the golden corpus (`apps/k7-daw-wasm/song/docs/goldenCorpus.md`), one list:
  Surge XT's factory presets in a chain's `bases`, the graph's base-preset choices, in the four projects
  that use it. No engine document holds a name this order and ICU's place apart.
- **the file service's listings**: the file tree, the workspaces, the projects, the playlists, the
  reference tracks, the module and chain catalogues, the engine plugins and a node's presets come in this
  order; k7's views sort each again for people before drawing it (`apps/k7-daw-frontend/docs/listOrder.md`).
- **files on disk**: nothing is written in a new order. The chain file writer sorts keys only to compare two
  items (`apps/k7-daw-wasm/song-edit/src/format_chain.rs`), and any total order compares them alike.
