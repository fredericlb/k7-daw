// eventGain.ts — one audio event's own trim, `gain:` in dB on its placement, ±6. Rounded to a tenth of a dB, since
// a slider's every pixel is not a level anyone chose, and deleted at 0, which is unity and the default.
// See docs/eventGain.md.

import { stub } from "@studio/song/editStub";

/** The trim an event may carry, either way: a balance between takes, not a fader. */
export const EVENT_GAIN_MAX_DB = 6;

/** `db` as the file keeps it: clamped, to a tenth of a dB, and undefined for unity. */
export function eventGainValue(db: number): number | undefined {
  return stub("song-edit/eventGain.eventGainValue", db);
}

/** Write the trim of the placement at `fromBeats`; false when no event sits there. */
export function setTrackPlacementGain(
  dir: string,
  id: string,
  fromBeats: number,
  num: number,
  den: number,
  db: number,
): boolean {
  return stub(
    "song-edit/eventGain.setTrackPlacementGain",
    dir,
    id,
    fromBeats,
    num,
    den,
    db,
  );
}
