# The text writers

How the page edits a file's text: `apps/k7-daw-frontend/packages/k7-wasm/src/text.ts`, over the `text.*` ops
(`apps/k7-daw-wasm/k7-wasm/src/ops/text.rs`) and the writers of `apps/k7-daw-wasm/song-edit`.

## Choices

| choice | why |
|---|---|
| a writer takes text and answers text plus the splices it made (`TextAnswer`), never a document held across calls | the page keeps its buffer in React state; a writer the module carries has no host to hold a handle for |
| one op per family (`text.video`, `text.sequence`, `text.playlist`, `text.mix`, `text.assembly`) carrying a list of writers | a gesture is several writes on one document (a drag sets a field and shifts the keys), and one crossing keeps it one undo step |
| a refused edit is a `false` in `results` and the text unchanged; an unreadable text or a writer that throws is `error` | the page tells "nothing to do" from "this does not read", as the TypeScript writers did |
| a picture lane's writers take the key path of the lane (`at`) | a timeline file holds several lanes in `pictures:`, and the writer edits the one the drag grabbed |
| a sequence is edited on its expanded notes and the short form put back by the one rule of `apps/k7-daw-wasm/k7x/src/edit/sugared.rs` | a writer never has to know `notes("c4 d4")` is a string; `text.sugar_apply` and `text.sugar_expand` give the page the same rule |
| the readers (`text.mix_cues`, `text.mix_view`, `text.assembly_read`, `text.chain_read`) answer the Rust decoder's first finding beside an empty value | the schema is the reader that compiles the file; the page draws what is wrong instead of catching a throw |
| a list position crosses as a float and is refused unless whole | ts-rs types a Rust integer as `bigint`, which a page's counter is not |
| a writer that must read other files is a verb behind the backend, not a `text.*` op | the module imports nothing from its host (`docs/rust-song-twins-plan.md`) |
| `mapNotesInRegion` is a Rust function only | its edit is a closure, which cannot cross; the page asks the verbs for a region edit |

## Tests

The writers are tested where their rules are: the exact text they leave, in Rust
(`apps/k7-daw-wasm/song-edit/tests/text_writers.rs`, and through the ops `apps/k7-daw-wasm/k7-wasm/src/tests_text.rs`,
`tests_text_video.rs`, `tests_text_sequence.rs` and `tests_text_desk.rs`). The page's own test,
`apps/k7-daw-frontend/packages/k7-wasm/src/text.test.ts`, only proves that a call reaches the module and answers its shape.
