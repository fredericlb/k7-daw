# `@studio/k7-wasm/engine`: the page's engine rules, from Rust

## Choices

| choice | why |
|---|---|
| The rules live in `song-schema` (`text_hash`, `engine/`: quality, compat, choose, warnings), the one wasm-safe crate the backend's file service and the page's module both depend on | a rule written once in a crate that `k7-file-service` cannot give to a browser (tokio, notify) would be two copies again; this crate builds for both |
| Each rule is one JSON op (`text.hash`, `engine.compat`, `engine.latency`, `engine.warning`, `engine.choose`, `engine.best`) in the module every host already loads | each runs once per write, probe, pick or warning, never per frame or pointer move, so a typed numeric export would buy nothing |
| The page works out what needs a browser (`URL` parsing for "reached directly", loopback) and hands the policy booleans; the order and the rule are Rust's | the policy has no URL parser to share, and a native interface brings its own address facts |
| `engine.warning` answers the kind and the missing item in one call | a warning is classified once and read twice (its sentence, the missing-files dialog) |
| The warning kinds are matched by hand over the C++ engine's prose, not with a regex crate | the texts are a dozen stable prefixes; a dependency in the module for them would outweigh them |
| The compatibility score is an integer floor | JavaScript's `29 / 100 * 100` floors to 28, and the file service already scored in integers; one formula serves both now |
| A call made before `wasmReady()` throws `unavailable`; there is no TypeScript fallback | the page awaits readiness before it mounts, and a fallback would be the twin this removes |

## Left in TypeScript, and why

| module | why it stays |
|---|---|
| `src/clipEnvelope.ts`, `src/engineSession.ts` | not twins: the cache key folds what the page holds, not what the service's `wfSig` folds, and `engineSession` is address parsing around a one-line slug |
| `src/components/engineTaken.ts` | a three-flag decision over React state, called on render |
| `lcdSounds`, `libraryTree`, `historyMarks`, `surfaceRows`, `paneInUse` | no Rust function answers them yet: they are ports, not twin replacements |
