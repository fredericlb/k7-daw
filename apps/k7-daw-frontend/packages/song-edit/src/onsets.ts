import { stub } from "@studio/song/editStub";
// onsets.ts — the onsets a clip actually has: what the engine detected, plus the corrections made by hand.
//
// The engine's set is derived and rebuilt whenever the audio or the sensitivity changes; the corrections are
// in the clip file and outlive both. This resolves the two into the one list every surface should draw.

/** A correction block as `clips/*.k7x` stores it — both lists in SECONDS into the source file. */
export interface OnsetEdits {
  added?: number[];
  removed?: number[];
}

/** The onsets to draw: `detected` less what was removed, plus what was added, near-duplicates collapsed. */
export function resolveOnsets(
  detected: readonly number[],
  edits: OnsetEdits | undefined,
): number[] {
  return stub("song-edit/onsets.resolveOnsets", detected, edits);
}

/** The edits after removing the onset at `atSec` — removing an ADDED one undoes the addition. */
export function removeOnset(
  edits: OnsetEdits | undefined,
  detected: readonly number[],
  atSec: number,
): OnsetEdits {
  return stub("song-edit/onsets.removeOnset", edits, detected, atSec);
}

/** The edits after adding an onset at `atSec` — dropping a removal it cancels, rather than stacking on it. */
export function addOnset(
  edits: OnsetEdits | undefined,
  atSec: number,
): OnsetEdits {
  return stub("song-edit/onsets.addOnset", edits, atSec);
}

/** Moving one is removing it and adding it back elsewhere — which is what moving it is. */
export function moveOnset(
  edits: OnsetEdits | undefined,
  detected: readonly number[],
  fromSec: number,
  toSec: number,
): OnsetEdits {
  return stub("song-edit/onsets.moveOnset", edits, detected, fromSec, toSec);
}
