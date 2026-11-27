// clipLoop.ts — where a clip's LOOP sits on the axis its editor draws; the arithmetic is Rust's (song/src/reads/clip.rs).
// A reader would get wrong that the repeat marks stay here: they are drawn per zoom, and a pane's scale is not the document's.

import { type ClipLoop, clipLoop, playedZone } from "@studio/k7-wasm/reads";

export type { ClipLoop };
export { clipLoop, playedZone };

/** The repeat lines worth drawing at this scale — 256 passes are a solid grey block, and the window's own edges are never dropped. */
export function visibleRepeats(
  loop: ClipLoop,
  pxPerBeat: number,
  minPx = 6,
): number[] {
  return loop.win * pxPerBeat >= minPx ? loop.repeats : [];
}

/** The same repeats as offsets from the PLACEMENT's start — same arithmetic, different zero, and the wrong one is silently wrong for MIDI. */
export function placedRepeats(
  loop: ClipLoop,
  pxPerBeat: number,
  minPx = 6,
): number[] {
  if (loop.win * pxPerBeat < minPx) return [];
  const out: number[] = [];
  for (let k = 1; k < loop.passes && k <= 256; k++) out.push(k * loop.win);
  return out;
}
