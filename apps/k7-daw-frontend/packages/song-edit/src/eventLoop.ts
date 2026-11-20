// eventLoop.ts — an event's LOOP as the clip panels edit it: switched on or off, and its two points moved.
// The points ARE the event's window (`from`/`to`), so moving them narrows what it plays — but a looped event
// keeps its length on the lane, its pass count recomputed, the way a loop brace behaves. See docs/eventLoop.md.

import { stub } from "@studio/song/editStub";
import type { MaterialKind } from "./clipResize";

/** Switch the event's loop on (`FIRST_LOOP` passes) or off (one); `fileSeconds` gives an audio window with no end one. */
export function setTrackPlacementLoop(
  dir: string,
  id: string,
  fromBeats: number,
  num: number,
  den: number,
  tempo: number,
  on: boolean,
  fileSeconds?: number,
): { loops: number; changed: boolean } | null {
  return stub(
    "song-edit/eventLoop.setTrackPlacementLoop",
    dir,
    id,
    fromBeats,
    num,
    den,
    tempo,
    on,
    fileSeconds,
  );
}

/** Move the event's loop points — its window — to `from`/`to` in the material's unit, clamped to the material. */
export function setTrackPlacementLoopPoints(
  dir: string,
  id: string,
  fromBeats: number,
  num: number,
  den: number,
  tempo: number,
  from: number,
  to: number,
): { kind: MaterialKind; from: number; to: number; loops: number } | null {
  return stub(
    "song-edit/eventLoop.setTrackPlacementLoopPoints",
    dir,
    id,
    fromBeats,
    num,
    den,
    tempo,
    from,
    to,
  );
}
