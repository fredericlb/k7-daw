// clipResize.ts — turn "this event should span these two beats" into the fields that model
// actually has: the MATERIAL's window (`from`/`to`) and the placement's `start`/`loop`. A
// placement has no length to set, so a resize is not one number — see docs/clipResize.md.

import { stub } from "@studio/song/editStub";

/** The units a window is written in: seconds into a file, or beats into a sequence's notes. */
export type MaterialKind = "audio" | "sequence";

/** The event as it stands: its EFFECTIVE window, its pass count, and the cage it may not leave. */
export interface ResizeBefore {
  kind: MaterialKind;
  from: number; // 0 when nothing narrows it — the material's start
  to?: number; // absent means "to the end of the source", a length only the engine knows
  loops: number;
  startBeats: number; // where the placement sits on the timeline
  /** The MATERIAL's window, which the event's is clamped inside; absent means from 0, to unknown. */
  bounds?: { from?: number; to?: number };
}

/** Where the drag put the two edges, on the timeline, in beats. */
export interface ResizeTo {
  startBeats: number;
  endBeats: number;
  tempo: number;
}

/** What to write: the placement's start, and the material's window and pass count. */
export interface ResizeResult {
  startBeats: number;
  from: number;
  to: number;
  loops: number;
}

/** The EVENT's window and count spanning `startBeats..endBeats`, clamped inside `bounds` — apps/k7-daw-frontend/packages/song/docs/clipResize.md. */
export function resizeToWindow(
  before: ResizeBefore,
  to: ResizeTo,
): ResizeResult {
  return stub("song-edit/clipResize.resizeToWindow", before, to);
}
