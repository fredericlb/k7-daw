// stems.ts — the names a deck gives one stem's lane, platter and control, answered by the Rust crate (ops/stems.rs).
// A reader would get wrong that the vocabulary is here: `STEMS` and its travel are generated constants, re-exported so a deck imports one door.

import { call } from "./abi.ts";
import type { Stem, StemLane } from "./types/index.ts";

export {
  STEM_NEUTRAL,
  STEM_RANGE,
  STEMS,
  STEMS_DIR,
} from "./consts.generated.ts";
export type { Stem };

const name = (op: string, args: object): string =>
  call<string>({ op: `stems.${op}`, args });

/** One stem's file name, as the pass writes it: `bass.wav`. */
export const stemFile = (stem: Stem): string => name("file", { stem });

/** The lane one stem plays on: `stemBassA`. */
export const stemLane = (deck: string, stem: Stem): string =>
  name("lane", { deck, stem });

/** …and the PLATTER that reads its file: `platterBassA`. */
export const stemPlatter = (deck: string, stem: Stem): string =>
  name("platter", { deck, stem });

/** The platter a deck's own RECORD plays on: `platterA`. */
export const deckPlatter = (deck: string): string =>
  name("deck_platter", { deck });

/** The desk control that drives a stem's node: `deckA.stemBass`. */
export const stemControl = (deck: string, stem: Stem): string =>
  name("control", { deck, stem });

/** Which deck and stem a lane id names, or null: the reverse of `stemLane`. */
export const stemLaneOf = (
  id: string,
  decks: readonly string[],
): StemLane | null =>
  call<StemLane | null>({ op: "stems.lane_of", args: { id, decks } });
