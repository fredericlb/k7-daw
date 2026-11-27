# The desk's rules are Rust

What the desk decides is the `turntables` crate (`apps/k7-daw-wasm/turntables`), reached from the page as `@studio/k7-wasm/desk` (the `desk.*` ops, `apps/k7-daw-wasm/k7-wasm/src/ops/desk.rs`). The views, the per-frame drawing and the reads of the engine's live state stay TypeScript. The rule is `docs/rust-first.md`; the choices below are what this zone made of it.

| choice | why |
|---|---|
| the deck document (`with_decks`, `deck_doc`) is built in Rust over `song`'s `ProjectDoc`, not over a mirror | one document type, written once; the compiled project crosses as JSON and comes back as the document the engine takes, so the desk and the compiler cannot disagree about a field |
| the vocabulary (the decks, every knob's ends, the loop lengths, the tempo module's names) is generated into `consts.generated.ts`; `DeckLoad`, `DeckTempo` and the control and binding types are generated with the ops | a number the desk draws and a number the rules clamp to are the same number |
| one op per rule, in `@studio/k7-wasm/desk`, named as the rule is | a view or a hook asks one question and gets one answer; nothing is composed across the boundary |
| the scratch's pace is a typed export (`k7_g_scratch_delay`), not a JSON op | it is asked on every pointer move; three numbers cost nothing, a request does |
| the lookups against the engine's live state (`hasPlugin`, `shownValue`, `engineAgrees`, `controlDriving`, `leavesByItsOwnPair`) stay in `mixBindings.ts` | they read what is on screen and run on every render over the whole parameter store, which would cross the boundary whole each time |
| the waveform's grid keeps `firstBeatAfter` and asks the crate for a record's phase once per envelope | the lines are drawn every frame; the phase is a fact about the record |
| the song browser's search and sort stay in `setList.ts` | the sort is the browser's own collator (`fr`, accents and digits folded), which Rust does not carry |
| the set list comes back with issue keys (`notFound`, `unreadable`) and `setList.ts` puts them in words | the rule is Rust's, the words are the page's catalogue |
| JavaScript's arithmetic is kept where a result is shown or sent: `Math.round` takes halves up, a NaN is "not positive", a float crosses JSON exactly | a loop edge or a seek must not move by a hair because the language changed |
| `put-on` is a binary beside the crate (`apps/k7-daw-wasm/put-on`) with its own manifest and lock | its socket library stays out of the browser module's lock |
