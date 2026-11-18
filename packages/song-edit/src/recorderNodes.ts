// recorderNodes.ts — a project's recorder NODES, taken out at open: the recorder module is gone, a project with no
// timeline records its output from the bar instead (packages/song/docs/outputRecording.md). Each one is deleted as
// `node delete` deletes, its run spliced shut, so `MIX → rec → OUT` reads `MIX → OUT` and sounds the same.

import { stub } from "@studio/song/editStub";

/** The module ref recorder nodes extended. */
export const RECORDER_REF = "@meta/audio/recorder";

/** Delete every node of `dir` extending the recorder; the names it took out. */
export function dropRecorderNodes(dir: string): string[] {
  return stub("song-edit/recorderNodes.dropRecorderNodes", dir);
}
