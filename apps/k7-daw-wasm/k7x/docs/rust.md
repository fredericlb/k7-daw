# The k7x reader

`src/parse.rs` is k7x's one reader: the backend, the verbs and the page all read through it, the page
through the browser module (`apps/k7-daw-wasm/k7-wasm`, ops `k7x.data` and `k7x.outline`). Its tree
is the one `language.md` describes, every node with its span.

## Choices

| choice | why |
|---|---|
| the page reads a file's DATA as JSON text (`k7x.data`), which `JSON.parse` turns into the value | over the corpus (1,272 files), `JSON.parse` of the data takes 5 ms with or without a JIT, where building the same objects in JS takes 43 ms with one and 139 ms without |
| the page reads a file's TREE as numbers plus one JSON array of keys and scalars (`k7x.outline`, `outline.md`) | the tree with its spans as JSON objects is 13.9 MB for the corpus and parses in 43 ms, slower than a JS parse |
| spans are byte offsets inside the crate, UTF-16 units wherever they cross to the page | Rust slices by byte; an editor and a JS string count UTF-16 |
| a syntax error's offset crosses in UTF-16 units, its column is counted in UTF-16 units already | a `K7SyntaxError` on the page carries the message, offset, line and column the editor places |
| a number keeps its JS value: `-0` is written `-0` and an overflow `1e999` | `JSON.parse` reads both back as a JS parser would have them |
| a lone surrogate escape (`"\uD800"`) reads as U+FFFD, and `Document::lossy` says so | a Rust string cannot hold one; no workspace file has one |
| the tree is built in a `bumpalo::Bump` the caller owns: a key or a string is a slice of the text unless it carries an escape, a container's children are one slice of the arena | a tree of `Vec`s and owned strings, freed node by node, measures 1.25–1.4× slower on every set below |
| the browser module keeps one arena and resets it per call | a read allocates nothing it keeps |

## What was measured

2026-10-19, Windows 11, x86-64, node 24.18, rustc 1.97 (release, LTO), bumpalo 3.20; the parse alone
(`k7x-bench`), medians in ms:

| set | node (JIT) | node --jitless | rust, `Vec` tree | rust, arena per file | rust, one arena reset |
|---|---|---|---|---|---|
| the corpus, 1272 files, 1.52 MB | 23 | 108–115 | 9.0–9.6 | 6.8 | 4.8–5.0 |
| its largest file, 0.30 MB | 6.4–8.9 | 24–26 | 2.4 | 2.0 | 1.0–1.1 |
| one synthetic file, 2.51 MB | 34 | 132–135 | 12.6 | 10.0–10.3 | 6.8 |

The node columns are the TypeScript parser k7x had then, the baseline the reader was held to.
