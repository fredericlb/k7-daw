// timelineFromRange.ts — the events of ONE lane starting in a range, moved into a new timeline and
// replaced by one event of it, so the lane sounds as it did. What a reader gets wrong: the range
// takes events by START, as `events move` does; one starting before it stays where it is.
// See @studio/song docs/timelines.md.

import { stub } from "@studio/song/editStub";

/** Move lane `laneId`'s events starting in `[from, end)` (beats) into timeline `name`; how many moved, 0 writing nothing. */
export function timelineFromRange(
  dir: string,
  laneId: string,
  from: number,
  end: number,
  num: number,
  den: number,
  name: string,
): number {
  return stub(
    "song-edit/timelineFromRange.timelineFromRange",
    dir,
    laneId,
    from,
    end,
    num,
    den,
    name,
  );
}
