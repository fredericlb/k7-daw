// videoDrag.ts — a picture placement dragged on the timeline, as the page holds it: the drag's state, and the one threshold the gesture reads.
// The arithmetic (where the shot goes, what a finished drag writes) is apps/k7-daw-wasm/gesture/src/video_drag.rs, in SECONDS. See ../docs/picture.md.

import type { VideoEdit, VideoShot } from "@studio/k7-wasm/gesture";

export type { VideoEdit };

/** Below this a gesture is a click, not a drag — the same 4px the ruler's gestures use. */
export const DRAG_SLOP = 4;

/** A picture placement in hand; `pxPerSec` and the ticks are captured at the GRAB, so a zoom mid-drag cannot slide the shot. */
export interface VDrag extends VideoShot {
  trackId: string;
  /** Its place in the lane's `events:` — see VideoEdit. */
  index: number;
  /** An event with no `ref`: a span saying the lane is ON, with no source to trim. */
  activation: boolean;
  /** Pointer x at grab. */
  x: number;
  pxPerSec: number;
  moved: boolean;
  /** Written, still drawn until the reload — as Move.pending is. */
  pending?: boolean;
}
