// materialCut.ts — cutting and merging MATERIAL, where trackMix.ts moves and resizes PLACEMENTS.
// `event split` writes two sequences, or two windowed placements of one audio source with nothing
// rendered; `selection consolidate` writes one sequence over a range, while on audio it needs a
// render and lives with the bounce (@studio/audio-passes' processChain.ts).
// Full rationale: docs/materialCut.md.

import { stub } from "@studio/song/editStub";

/** One material this wrote: what it is called, where it landed, and how much is in it. */
interface WrittenMaterial {
  ref: string;
  /** Project-relative POSIX path — for the log, so a person can go and open it. */
  file: string;
  notes: number;
}

export interface PhysicalSplitReport {
  head: WrittenMaterial;
  tail: WrittenMaterial;
  /** The material that was cut. Left on disk untouched — another lane may still play it. */
  from: string;
}

export interface ConsolidateReport {
  written: WrittenMaterial;
  /** How many placements became the one. */
  replaced: number;
  /** The materials it drew on, in the order they played. */
  from: string[];
}

/** Cut a SEQUENCE placement's material in two at `atBeats` and play the halves — docs/materialCut.md. */
export function splitSequenceMaterial(
  dir: string,
  trackId: string,
  fromBeats: number,
  atBeats: number,
  num: number,
  den: number,
): PhysicalSplitReport | null {
  return stub(
    "song-edit/materialCut.splitSequenceMaterial",
    dir,
    trackId,
    fromBeats,
    atBeats,
    num,
    den,
  );
}

/** Write ONE sequence over `[fromBeats, toBeats]`, gaps kept — see docs/materialCut.md. */
export function consolidateSequenceMaterial(
  dir: string,
  trackId: string,
  fromBeats: number,
  toBeats: number,
  num: number,
  den: number,
): ConsolidateReport | null {
  return stub(
    "song-edit/materialCut.consolidateSequenceMaterial",
    dir,
    trackId,
    fromBeats,
    toBeats,
    num,
    den,
  );
}

/** What an audio split wrote: the cut, in seconds of the source both halves still play. */
export interface PlacementSplitReport {
  cut: number;
}

/** Cut an AUDIO placement in two WITHOUT a render: two placements of one source, windowed either side of the cut — docs/materialCut.md. */
export function splitAudioPlacement(
  dir: string,
  trackId: string,
  fromBeats: number,
  atBeats: number,
  num: number,
  den: number,
  cut: number,
): PlacementSplitReport | null {
  return stub(
    "song-edit/materialCut.splitAudioPlacement",
    dir,
    trackId,
    fromBeats,
    atBeats,
    num,
    den,
    cut,
  );
}
