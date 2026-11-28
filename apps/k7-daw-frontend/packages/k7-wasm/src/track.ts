// track.ts — what a track's name declares (`[position.][prefix.]label`), answered by the Rust crate (ops/track.rs).
// A reader would get wrong that the prefix table is here: it is `TRACK_PREFIXES` in the generated constants.

export * from "./ops/track.ts";
export type { TrackKind } from "./types/index.ts";
