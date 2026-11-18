// formatStamp.ts — `format(N)` written into project.k7x the moment the project starts NEEDING level N,
// never before: a project with no event in seconds stays openable by a level-1 build.
// What a reader gets wrong: this is best-effort, like ensureVideoRun — a project that will not read
// still opens, and the next open or write tries again. See song-schema docs/formatLevel.md.

import { join } from "node:path";
import {
  formatLevelOfText,
  SECONDS_START_LEVEL,
} from "@studio/song-schema/formatLevel";
import { laneFolder } from "./chainItemEdit";
import { isRecord, listOf } from "./data";
import { stub } from "./editStub";
import { entityFiles } from "./yaml/moduleIO";
import { wsData, wsText } from "./yaml/workspaceDoc";

/** Whether a list of placements holds one whose `start` is a number of seconds. */
const secondsIn = (events: unknown): boolean =>
  listOf(events).some((p) => isRecord(p) && typeof p.start === "number");

/** Whether a lane or timeline file's data places an event in seconds: its own list, each alternative's, each timeline lane's. */
export function holdsSecondsStarts(data: unknown): boolean {
  if (!isRecord(data)) return false;
  if (secondsIn(data.events)) return true;
  const nested = [...listOf(data.alternatives), ...listOf(data.lanes)];
  return nested.some((x) => isRecord(x) && secondsIn(x.events));
}

/** The level a root file on disk declares, read off its raw text; absent means 1. */
export const declaredLevel = (path: string): number =>
  formatLevelOfText(wsText(path)) ?? 1;

/** Raise project.k7x's `format(N)` to `level` when it declares less; true when it wrote. */
export function stampFormatLevel(dir: string, level: number): boolean {
  return stub("song/formatStamp.stampFormatLevel", dir, level);
}

/** Stamp the level seconds need when any lane or timeline of `dir` writes one — what an OPEN runs. */
export function stampFormatForSeconds(dir: string): boolean {
  const lanes = laneFolder(dir);
  const files = [
    ...entityFiles(join(dir, lanes), lanes),
    ...entityFiles(join(dir, "timelines"), "timelines"),
  ];
  const needs = files.some(({ file }) => {
    try {
      return holdsSecondsStarts(wsData(file.path));
    } catch {
      return false; // a file that will not read is reported by the compile, not here
    }
  });
  return needs && stampFormatLevel(dir, SECONDS_START_LEVEL);
}
