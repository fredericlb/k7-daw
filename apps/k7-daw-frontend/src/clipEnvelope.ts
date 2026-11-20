// clipEnvelope.ts — the key a clip's drawn envelope is cached under in the browser. It folds
// everything about the placement that decides WHICH audio the picture shows, so an edit that
// moves the window misses the cache instead of reusing the old picture. Pure, and deliberately
// a mirror of the file service's own `wfSig` (apps/k7-daw-backend/packages/k7-file-service/src/media/mod.rs): the
// two answer the same question one layer apart. docs/timeline.md ("Activity runs through the
// blocks") holds the rationale.

import type { WebClip } from "./types";

/** The project fields the region math reads — the same three `wfSig` folds in. */
export interface EnvelopeProject {
  tempo: number;
  timeSigNumerator: number;
  timeSigDenominator: number;
}

/** Everything `regionWaveform` reads, in one string — the id alone is not enough, since a trim moves the material and nothing else. */
export function clipEnvelopeKey(
  clip: WebClip,
  project: EnvelopeProject,
  buckets: number,
): string {
  return [
    clip.id,
    clip.sourceName ?? "",
    clip.offset ?? "",
    clip.length,
    clip.start,
    clip.sourceFrom ?? "",
    clip.sourceTo ?? "",
    // Only the FACT of a warp reaches the browser, so turning one on or off is caught here and moving a marker inside it is not.
    clip.warped ? 1 : 0,
    project.tempo,
    project.timeSigNumerator,
    // The denominator decides the bars, so 7/4 and 7/8 draw different grids.
    project.timeSigDenominator,
    buckets,
  ].join("|");
}
