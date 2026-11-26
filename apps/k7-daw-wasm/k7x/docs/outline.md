# The editor's tree: `k7x.outline`

The code editor reads a k7x buffer's tree on every change: its links, the ▶ gutter and the slice
colours walk it (`apps/k7-daw-frontend/packages/ui-editor/docs/k7xTree.md`). The tree comes from the
Rust reader through the browser module, and `@studio/k7-wasm/k7x`'s `parseTree` builds its nodes.

## Choices

| choice | why |
|---|---|
| the layout crosses as u32 cells in the module's memory (`k7_cells_ptr`, `k7_cells_len`), read through a `Uint32Array` view before the next call | the same layout as JSON numbers cost 4.9–7 ms on the largest file, most of it writing and parsing the digits |
| the keys and scalars cross as one JSON array, in the order the cells meet them | they are strings and JS values; `JSON.parse` builds those faster than any binding |
| the cells are preorder, a node's tag then its span then its children's count, positions in UTF-16 units | the reader on the page walks them once, with no lookup and no conversion |
| the nodes the page builds are the same objects, field for field, as the tree the editor always read | the editor's walkers and the chain writer read them unchanged; the outline was checked equal on all 1,356 corpus files before the switch |
| the budget is the TypeScript parse it replaced, measured on the largest library file (`k7-core/modules/instruments/synth/surge-xt/ui.k7x`, 302 KB) | a keystroke in that file pays one outline; anything slower would have been felt as typing lag |

## What was measured

2026-11-26, M-series Mac under a load of 11–23, node 26, medians of 60 runs, `npm run bench:outline -w @studio/k7-wasm`:

| | `surge-xt/ui.k7x` (302 KB) | the corpus (1,356 files, 1.58 MB) |
|---|---|---|
| the TypeScript `parseTree` it replaced | 3.6–3.8 ms | 16.4–17.7 ms |
| `k7x.outline` and the nodes built | 3.3–3.5 ms | 17.5–18.1 ms |
| the same layout as JSON numbers (not kept) | 4.9–7.7 ms | 25–37 ms |

That file's outline is 41,047 keys and scalars beside its layout cells. A corpus of small files pays the
call's fixed cost per file; the editor reads one buffer at a time.
