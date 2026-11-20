// timelineRename.ts — a timeline renamed: its file, and every event placing it, on the song's lanes
// and inside other timelines. What a reader gets wrong: a placement can sit in a lane's
// ALTERNATIVE as well as in its playing `events`, so every `events` list in a file is rewritten.
// See @studio/song docs/timelines.md.

import { stub } from "@studio/song/editStub";

/** Rename timeline `from` to `to`; how many placements were repointed. Throws when either name is wrong. */
export function renameTimeline(dir: string, from: string, to: string): number {
  return stub("song-edit/timelineRename.renameTimeline", dir, from, to);
}
