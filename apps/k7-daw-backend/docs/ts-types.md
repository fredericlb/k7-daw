# TypeScript types generated from the wire structs

A TypeScript type that mirrors a Rust struct is written by ts-rs, never by hand (`docs/rust-first.md`).
This page holds the choices of that pipeline.

## Choices

| choice | why |
|---|---|
| a wire struct that the page's WebAssembly rules take as an argument (git's state, status, conflicts: `apps/k7-daw-wasm/git-rules`) is exported by `k7-wasm`'s bindings into `@studio/k7-wasm`'s `types/`, not by this binary | a generated file imports its neighbours by relative path, so a request type and the structs inside it must land in one folder |
| each crate derives `ts_rs::TS` on its wire structs behind its own `ts` feature | the struct and its TypeScript type live in one place, and a build without the feature carries no ts-rs |
| one binary, `k7-front`'s `bindings` (`--features ts`), exports every crate's types | the crates stay free of an output path, and the page's packages are written from one command |
| the binary writes a `generated/` folder into each package that owns types, `graph-model` today | a surface that draws a graph must not drag in `@studio/web-protocol`, so the graph's types land beside the graph, and the next package takes one line in `PACKAGES` |
| `npm run rebuild:generated` runs `apps/k7-daw-backend/scripts/bindings.mjs`: the binary, biome's formatter, then a `// SOURCES <hash>` line in `generated/index.ts` | the folder passes `npm run lint` as written, and `lint:generated` fails when the hash of the Rust sources no longer matches the stamp |
| a map is `Rec<V>` (`song/src/rec.rs`), named by `ts(as = "…")` | ts-rs writes a map as `{ [key in string]?: V }`, whose values read as possibly absent; `Record<string, V>` is what the page indexes, and the import of `V` stays generated |
| a field the Rust side always sends but a hand-written fixture leaves out is typed optional with `ts(as = "Option<…>")` | the page's tests build graphs from literals; the wire stays complete, the type stays as loose as the page always treated it |
| what only the page adds to a wire value (a dropdown's `arg`, a tab's folded `cols`, the narrow card's `tabsAs`) is a `Page*` type in `graph-model/types.ts`, built from the generated ones | the compile never sends those fields, so the wire struct does not carry them |
| a value both sides agree on (`MODULE_CATEGORIES`, `PORT_SEP`) stays in `types.ts`, checked against the generated type with `satisfies` | a generated type has no values; the check is what keeps the list honest |
| a generated type is renamed with `ts(rename = "…")` where the page's name differs from the Rust one | the callers keep their names, and the Rust side stays idiomatic |
| the file service's and the camera hub's wire shapes are structs of their own (`k7-file-service/src/wire/`, `k7-front/src/routes/cameras/wire.rs`), and `web-protocol/generated/` is written from them | most routes write their JSON text by hand, so there is no serialised struct to derive from; `tests/all/wire.rs` reads each builder's output back through the struct with unknown keys refused |
| a field the route always sends is required in the type, even where a hand-written fixture left it out | the struct is the truth; the fixture gains the field |
| the project address grammar is `song-schema/src/address.rs`, reached through the `address.*` ops of k7-wasm; the file service's bench rule calls the same function | one copy for the page and the backend; `web-protocol/projects.ts` keeps only the origin join |
