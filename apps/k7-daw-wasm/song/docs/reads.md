# The reads on the open document

## Choices

| choice | why |
|---|---|
| The page's reads of its document (takes, loops, spans, rows, the graph's inputs and racks, the shell's projection, the mute/solo rule, a few string rules) are functions of `song::reads`, reached through the `reads.*` ops of k7-wasm and `@studio/k7-wasm/reads` | one implementation serves the page and a native interface; a rule kept in TypeScript is a second copy |
| A read takes a slim model of the document (`reads/model.rs`): only the fields a rule reads, each optional as the page's types are | the page's WebProject carries more than any rule needs, and a field a rule ignores must not break a document that lacks it |
| An answer names rows, pieces and placements by index into what was passed; the page hands its own objects back | an object is the page's identity (React keys, selections); Rust owns the order and the test |
| An answer is memoised on the identity of the document or the track list passed | a render that asks twice crosses the module once |
| An open end crosses as null (a picture lane's last shot) and a silent meter as null (`-Infinity`) | JSON has no infinity; the page reads null back as one |
| Words are not here: a refusal is a key (`required`, `noSlash`, `noDot`) the page spells with paraglide | the rule is Rust's, the words are the page's |
| `project.ts` (the engine's document) and the Web* types of `timelineTypes.ts` are generated from the Rust structs; `build.ts` takes its defaults from `reads.blanks` and its names from the generated constants | the page and the DJ desk read these types, so one source states them |
| The ghost's `isFirstPlacement`, the repeat marks (`visibleRepeats`, `placedRepeats`), `litAt`, `maxSpans`, `takeColor`, `envelopePath` and the mastering control table stay in TypeScript | they run per frame or per zoom, or they are drawing (decision 12) |
