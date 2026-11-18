// insertTime.ts — insert or delete a span of song time, across everything positioned in it: every
// lane's events, its takes and its alternatives, the markers, every drawn curve's points, and the
// video lanes. One pass, one report. Full rationale: docs/insertTime.md.

import { stub } from "@studio/song/editStub";

/** What a splice touched, by kind — what the command reports and what a test asserts on. */
export interface TimeSpliceReport {
  files: string[]; // project-relative, in the order they were written
  moved: number; // placements, curve points and markers shifted
  removed: number; // …and dropped, which only a delete does
}

/** Insert (or delete) `lenBeats` of time at `atBeats`, everything judged by its `start`. */
export function spliceProjectTime(
  dir: string,
  atBeats: number,
  lenBeats: number,
  mode: "insert" | "delete",
): TimeSpliceReport {
  return stub(
    "song-edit/insertTime.spliceProjectTime",
    dir,
    atBeats,
    lenBeats,
    mode,
  );
}
