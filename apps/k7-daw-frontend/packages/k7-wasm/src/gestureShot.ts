// gestureShot.ts — a picture drag and the tools' lines, from Rust.
// A reader would get wrong that a shot is in SECONDS, a picture lane being placed on the clock rather than the bar grid.

import { call } from "./abi.ts";
import {
  input,
  MODE_CODE,
  maybe,
  run,
  type SnapMode,
  scalars,
  trapped,
  typed,
} from "./gestureIo.ts";
import type {
  ClipTarget,
  FadeClip,
  FadePair,
  Note,
  ShotTarget,
  Tool,
  ToolAct,
} from "./types/index.ts";

export type { ClipTarget, Note as ToolNote, ShotTarget, Tool, ToolAct };

// a picture drag

/** One field of a picture placement, in seconds, by INDEX in `events:`. */
export interface VideoEdit {
  index: number;
  field: "start" | "from" | "length";
  value: number;
}

/** A picture placement in hand, in seconds throughout. */
export interface VideoShot {
  mode: "move" | "in" | "out";
  origStart: number;
  /** 0 is to the end of the source. */
  origLength: number;
  origFrom: number;
  /** How fast the source runs under the shot, signed; absent is 1. */
  speed?: number;
  start: number;
  length: number;
}

const VIDEO_CODE = { move: 0, in: 1, out: 2 } as const;
const FIELDS = ["start", "from", "length"] as const;

/** What a pointer move snaps a picture drag to. */
export interface VideoSnap {
  mode: SnapMode;
  /** The ruler's unit in beats: what a relative drag steps by. */
  unit: number;
  /** Beats per second. */
  bps: number;
  /** The ruler's drawn ticks, in beats. */
  ticks: readonly number[];
}

const HEAD = 12;

/** Where a pointer move puts a shot: `dx` pixels from the grab at `pxPerSec`, snapped as `snap` says; in seconds. */
export function videoDragMove(
  v: VideoShot,
  dx: number,
  pxPerSec: number,
  snap: VideoSnap,
): { start: number; length: number } {
  const x = typed();
  try {
    const n = HEAD + snap.ticks.length;
    const b = input(x, n);
    b[0] = VIDEO_CODE[v.mode];
    b[1] = v.origStart;
    b[2] = v.origLength;
    b[3] = v.origFrom;
    b[4] = maybe(v.speed);
    b[5] = v.start;
    b[6] = v.length;
    b[7] = dx;
    b[8] = pxPerSec;
    b[9] = MODE_CODE[snap.mode];
    b[10] = snap.unit;
    b[11] = snap.bps;
    b.set(snap.ticks, HEAD);
    x.k7_g_video_drag(n);
    const s = scalars(x);
    return { start: s[0] as number, length: s[1] as number };
  } catch (e) {
    return trapped(e);
  }
}

/** The fields a finished picture drag writes, in SECONDS: the left edge writes three. */
export function videoDragEdits(
  v: VideoShot & { index: number; activation: boolean },
): VideoEdit[] {
  return run((x) => {
    x.k7_g_video_edits(
      VIDEO_CODE[v.mode],
      v.origStart,
      v.origFrom,
      maybe(v.speed),
      v.start,
      v.length,
      +v.activation,
    );
    const s = scalars(x);
    const out: VideoEdit[] = [];
    for (let i = 0; i < (s[0] as number); i++)
      out.push({
        index: v.index,
        field: FIELDS[s[1 + i * 2] as number] as VideoEdit["field"],
        value: s[2 + i * 2] as number,
      });
    return out;
  });
}

/** Whether a finished drag left the placement exactly where it was: then nothing is written. */
export const videoDragIsStill = (v: VideoShot): boolean =>
  run(
    (x) =>
      x.k7_g_video_is_still(
        VIDEO_CODE[v.mode],
        v.origStart,
        v.origLength,
        v.start,
        v.length,
      ) !== 0,
  );

/** Seconds drawn for a `length: 0` placement, whose real extent needs the source opened to know. */
export const videoNatural = (): number => run((x) => x.k7_g_const(5));

// the tools

/** Only the four fields the crossfade rule reads cross, not the whole clip. */
const fadeClip = (c: FadeClip): FadeClip => ({
  id: c.id,
  start: c.start,
  length: c.length,
  type: c.type,
});

/** What a press produces: a line to run, or a refusal to say out loud; null is the pointer's, the gesture underneath running. */
export const clipToolLine = (tool: Tool, target: ClipTarget): ToolAct | null =>
  call<ToolAct | null>({ op: "gesture.tool_line", args: { tool, target } });

/** The same tools on a picture lane, each on its `picture` verb. */
export const shotToolLine = (tool: Tool, target: ShotTarget): ToolAct | null =>
  call<ToolAct | null>({
    op: "gesture.picture_tool_line",
    args: { tool, target },
  });

/** The neighbour a crossfade press means: its index in `clips`, and whether it lies AFTER the pressed one. */
export const crossfadePair = (
  clips: readonly FadeClip[],
  pressed: FadeClip,
  at: number,
): FadePair | null =>
  call<FadePair | null>({
    op: "gesture.crossfade_pair",
    args: { clips: clips.map(fadeClip), pressed: fadeClip(pressed), at },
  });

/** What a crossfade click makes, in beats: a drag sizes it instead. */
export const crossfadeBeats = (): number => run((x) => x.k7_g_const(8));
