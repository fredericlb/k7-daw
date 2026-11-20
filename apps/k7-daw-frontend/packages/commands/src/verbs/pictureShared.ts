// pictureShared.ts — what every `picture …` verb file opens a lane with: the lane's file (the
// song's own, or a timeline's map), the placement a time names, and the snapshotting edit.
// What a reader gets wrong: a timeline's lane is a map INSIDE the timeline's file, not a file.

import { join } from "node:path";
import type { K7Path } from "@studio/k7x/ast";
import { K7Document, type K7Scope } from "@studio/k7x/document";
import { pictureLaneFile, pictureLaneIds } from "@studio/song/pictureLanes";
import { wsData, wsEdit, wsText } from "@studio/song/yaml/workspaceDoc";
import { wsExists } from "@studio/song/yaml/workspaceFile";
import { stripDocExt } from "@studio/song-schema/docExt";
import { timelineLaneOf } from "@studio/song-schema/timelineLane";
import {
  listVideoPlacements,
  type VideoPlacement,
} from "@studio/song-schema/yaml/videoTrackFile";
import { parseWallClock } from "@studio/song-video/videoProject";
import type { CommandHost } from "../commands.ts";
import { splitWords } from "./shared.ts";

/** How close two starts have to be to be the same one — the file is written to the millisecond. */
export const SAME = 5e-4;

/** A timeline's name, the way `timeline new` makes one — also what keeps a path out of it. */
export const ID = /^[A-Za-z0-9_.-]+$/;

/** Where a picture lane's map is: its own file at the root, or a timeline's file at `pictures[i]`. */
export interface PictureLaneAt {
  rel: string;
  abs: string;
  text: string;
  path: K7Path;
  /** The timeline holding it, when it is one of a timeline's (@studio/song docs/timelines.md). */
  timeline?: string;
}

/** The placements a lane holds, in file order — the indices every edit writes by. */
export const placementsOf = (lane: PictureLaneAt): VideoPlacement[] =>
  listVideoPlacements(new K7Document(lane.text).at(lane.path));

/** A timeline's picture lane, `<timeline>><lane>`, or a refusal naming the ones it has. */
function timelinePictureLane(
  tl: { timeline: string; lane: string },
  h: CommandHost,
): PictureLaneAt | null {
  const rel = `timelines/${tl.timeline}.k7x`;
  const abs = join(h.songPath, rel);
  if (!ID.test(tl.timeline) || !wsExists(abs)) {
    h.refuse([`no timeline '${tl.timeline}' — ${rel} does not exist`]);
    return null;
  }
  const pictures = wsData(abs).pictures;
  const names = (Array.isArray(pictures) ? pictures : []).map((p) =>
    String((p as { lane?: unknown })?.lane),
  );
  const at = names.indexOf(tl.lane);
  if (at < 0) {
    h.refuse([
      `timeline '${tl.timeline}' has no picture lane '${tl.lane}'${names.length ? ` — it has ${names.join(", ")}` : ""}`,
    ]);
    return null;
  }
  return {
    rel,
    abs,
    text: wsText(abs),
    path: ["pictures", at],
    timeline: tl.timeline,
  };
}

/** A lane's file, or a refusal naming the lanes there are; a timeline's lane is its map in the timeline's file. */
export function laneFile(lane: string, h: CommandHost): PictureLaneAt | null {
  const tl = timelineLaneOf(lane.trim());
  if (tl) return timelinePictureLane(tl, h);
  const name = stripDocExt(lane.trim());
  // A lane name is one segment, never a way out — and never a path the lookup below would try.
  if (!ID.test(name)) {
    h.refuse([`'${lane}' is not a lane name`]);
    return null;
  }
  const file = pictureLaneFile(h.songPath, name);
  if (!file) {
    const ids = pictureLaneIds(h.songPath);
    h.refuse([
      `no picture lane '${name}'${ids.length ? ` — this song's are ${ids.join(", ")}` : " — this song has none"}`,
    ]);
    return null;
  }
  return {
    rel: file.rel.replace(/^\//, ""),
    abs: file.path,
    text: wsText(file.path),
    path: [],
  };
}

/** `laneFile` for a verb about the LAYER — its frame, its visibility — which a timeline's lane borrows from the song's. */
export function layerFile(
  lane: string,
  h: CommandHost,
  verb: string,
): PictureLaneAt | null {
  const tl = timelineLaneOf(lane.trim());
  if (tl) {
    h.refuse([
      `picture ${verb}: ${lane} draws on the song's picture lane '${tl.lane}' — ${verb} that one`,
    ]);
    return null;
  }
  return laneFile(lane, h);
}

/** A wall-clock argument as seconds, or a refusal in the dialect's own words. */
export function seconds(
  word: string | undefined,
  h: CommandHost,
): number | null {
  const s = word === undefined ? null : parseWallClock(word);
  if (s === null) {
    h.refuse([
      `'${word ?? ""}' is not a time — picture reads seconds (12.5) or m:ss (1:30.5)`,
    ]);
    return null;
  }
  return s;
}

/** The one placement starting at `at`, or a refusal — two there is an ambiguity nothing may guess at. */
function placementAt(
  list: VideoPlacement[],
  at: number,
  rel: string,
  h: CommandHost,
): number | null {
  const hits = list
    .map((p, i) => ({ p, i }))
    .filter(({ p }) => Math.abs(p.start - at) < SAME);
  if (!hits.length) {
    const starts = list.map((p) => p.start).join(", ");
    h.refuse([
      `nothing starts at ${at}s on ${rel}${starts ? ` — there are shots at ${starts}` : " — it holds no shot"}`,
    ]);
    return null;
  }
  // Two shots on one lane at one instant: the format allows it and no rule picks between them.
  if (hits.length > 1) {
    h.refuse([
      `${hits.length} shots start at ${at}s on ${rel} — move one of them first, they cannot be told apart`,
    ]);
    return null;
  }
  return (hits[0] as { i: number }).i;
}

/** Edit the lane, snapshotting first for the undo journal; an edit answering false writes nothing and is refused with `refusal`. */
export function put(
  lane: { abs: string; path: K7Path },
  edit: (doc: K7Scope) => unknown,
  h: CommandHost,
  verb: string,
  refusal = `picture ${verb}: nothing to change there`,
): boolean {
  let refused = false;
  try {
    h.snapshot?.(lane.abs);
    wsEdit(lane.abs, (doc) => {
      if (edit(doc.at(lane.path)) !== false) return;
      refused = true;
      throw new Error(refusal);
    });
    return true;
  } catch (e) {
    h.refuse([refused ? refusal : `picture ${verb}: ${(e as Error).message}`]);
    return false;
  }
}

/** What every verb but `add` opens: the lane, its placements, and which one the time names. */
export function target(
  a: string,
  h: CommandHost,
  verb: string,
  tail: string,
):
  | (PictureLaneAt & {
      list: VideoPlacement[];
      at: number;
      index: number;
      rest: string[];
    })
  | null {
  const [laneArg, atArg, ...rest] = splitWords(a);
  if (!laneArg || !atArg) {
    h.refuse([`usage: picture ${verb} <lane> <at> ${tail}`.trimEnd()]);
    return null;
  }
  const found = laneFile(laneArg, h);
  if (!found) return null;
  const at = seconds(atArg, h);
  if (at === null) return null;
  const list = placementsOf(found);
  const index = placementAt(list, at, found.rel, h);
  if (index === null) return null;
  return { ...found, list, at, index, rest };
}
