# `key.ts` — the Camelot wheel

`src/key.ts` holds what a page reads of a key: the twelve pitch classes and `camelot`, the wheel's
hour for a tonic and a mode (`8B` for C major, `8A` for A minor).

## Choices

| choice | why |
|---|---|
| no key detector in TypeScript | no page calls one; a detector belongs to the analysis pass in Rust (k7-mediatek), where it has the decoded audio |
| the wheel walks in fifths | a neighbour on the wheel is a fifth away, which is what makes a mix between two codes sound related |
| the measurements stay in `docs/key-detection-plan.md` | what a detector would cost and what is unknown are the reason none is built yet |
