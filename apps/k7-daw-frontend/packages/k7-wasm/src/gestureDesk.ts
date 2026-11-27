// gestureDesk.ts — the desk's one typed call: how long to hold a scratch, in milliseconds, asked on every pointer move.
// A reader would get wrong how "no rate" crosses: as NaN, since a release hands the record back and never waits.

import { trapped, typed } from "./gestureIo.ts";

/** How long to hold this scratch: 0 goes now; a release (null rate) never waits; no `every` is the default pace. */
export function scratchDelay(
  sinceLast: number,
  rate: number | null,
  every?: number,
): number {
  const x = typed();
  try {
    return x.k7_g_scratch_delay(
      sinceLast,
      rate ?? Number.NaN,
      every ?? Number.NaN,
    );
  } catch (e) {
    return trapped(e);
  }
}
