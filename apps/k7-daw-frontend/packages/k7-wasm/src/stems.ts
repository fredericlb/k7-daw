// stems.ts — the names a deck gives one stem's lane, platter and control, answered by the Rust crate (ops/stems.rs).
// A reader would get wrong that the vocabulary is here: `STEMS` and its travel are generated constants, re-exported so a deck imports one door.

export {
  STEM_NEUTRAL,
  STEM_RANGE,
  STEMS,
  STEMS_DIR,
} from "./consts.generated.ts";
export * from "./ops/stems.ts";
export type { Stem } from "./types/index.ts";
