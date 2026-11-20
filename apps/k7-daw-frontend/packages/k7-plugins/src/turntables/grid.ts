// grid.ts — laying a corrected tempo over what the analysis measured. The correction belongs to the
// RECORD and not to this set: 87 BPM is 87 BPM in every project that plays it. See docs/grid.md.

import type { DeckTempo } from "./deckVocabulary";

/** A correction of a record's grid, held for the session. */
export interface GridFix {
  bpm?: number;
}

/** The analysis, with this set's correction laid over it — docs/grid.md.
 *
 * `natural` is what the record IS, so a corrected tempo replaces it and `target` follows unless a
 * fader has already moved: halving a wrongly-doubled 174 has to leave the deck playing at 87, not
 * stretching an 87 record to 174. */
export function corrected(
  tempo: DeckTempo,
  fix: GridFix | undefined,
): DeckTempo {
  if (!fix || fix.bpm === undefined) return tempo;
  const bpm = fix.bpm;
  const followed = bpm !== undefined && tempo.target === tempo.natural;
  return {
    ...tempo,
    ...(bpm !== undefined ? { natural: bpm } : {}),
    // A fader already moved is a decision; only an untouched one follows the correction.
    ...(followed ? { target: bpm } : {}),
  };
}
