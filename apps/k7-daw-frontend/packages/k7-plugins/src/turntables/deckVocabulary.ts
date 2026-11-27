// deckVocabulary.ts — what a deck IS and the ends of every knob the desk draws over it, as the page reads them.
// A reader would get wrong where the numbers are written: in Rust (turntables' vocab.rs), generated into @studio/k7-wasm; only the colours are drawn here.

import type { WebModuleCategory } from "@studio/graph-model";
import {
  DECKS,
  type DeckLoads,
  type DeckModuleDoc,
} from "@studio/k7-wasm/desk";
import { seriesColor } from "@studio/ui/series-colors";

export {
  BPM_RANGE,
  CENSOR_RATE,
  DECKS,
  DEFAULT_LOOP_BEATS,
  type DeckId,
  type DeckLoad,
  type DeckLoads,
  type DeckTempo,
  type DeckTempos,
  ECHO_RANGE,
  EQ_BANDS,
  EQ_DB,
  FILTER_RANGE,
  LEVEL_DB,
  MIX_CENTRE,
  PITCH_RANGE,
  TEMPO_SOURCE_PARAM,
  TRIM_DB,
} from "@studio/k7-wasm/desk";

/** A module the deck runs, as the desk holds it: a library module ref plus what it plays as. */
export interface DeckModule extends DeckModuleDoc {
  /** Its type folder, which is what picks the node's glyph. */
  category: WebModuleCategory;
}

/** Nothing on either deck. */
export const NO_LOADS: DeckLoads = Object.fromEntries(
  DECKS.map((d) => [d, null]),
) as DeckLoads;

/** A colour per cue BUTTON rather than per cue: the series palette's first eight, which keep away from the readout's amber — see docs/cues.md. */
export const CUE_COLOURS: readonly string[] = Array.from(
  { length: 8 },
  (_, i) => seriesColor(i),
);
