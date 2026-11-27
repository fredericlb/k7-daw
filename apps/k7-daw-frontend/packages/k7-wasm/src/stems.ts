// stems.ts — the names a deck gives one stem's lane, platter and control, answered by the Rust crate (ops/stems.rs).
// A reader would get wrong that the vocabulary is here: `STEMS` and its travel are generated constants, re-exported so a deck imports one door.

import { call } from "./abi.ts";
import type { Stem } from "./types/index.ts";

export {
  STEM_NEUTRAL,
  STEM_RANGE,
  STEMS,
  STEMS_DIR,
} from "./consts.generated.ts";
export type { Stem };

const name = (op: string, args: object): string =>
  call<string>({ op: `stems.${op}`, args });

/** The lane one stem plays on: `stemBassA`. */
export const stemLane = (deck: string, stem: Stem): string =>
  name("lane", { deck, stem });

/** …and the PLATTER that reads its file: `platterBassA`. */
export const stemPlatter = (deck: string, stem: Stem): string =>
  name("platter", { deck, stem });

/** The desk control that drives a stem's node: `deckA.stemBass`. */
export const stemControl = (deck: string, stem: Stem): string =>
  name("control", { deck, stem });
