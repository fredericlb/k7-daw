// gesture.ts — the page's gesture arithmetic from Rust: the grid snap and the piano roll's drags and fit; the rest is re-exported from its siblings.
// A reader would get wrong that the pointer-move calls are typed, numbers in and numbers out, and only the once-per-release ones carry JSON (docs/gesture.md).

import { call } from "./abi.ts";
import {
  input,
  MODE_CODE,
  MODES,
  maybe,
  run,
  type SnapMode,
  scalars,
  trapped,
  typed,
} from "./gestureIo.ts";
import type { GroupMove } from "./types/index.ts";

export * from "./gestureGrid.ts";
export * from "./gestureShot.ts";
export type { GroupMove, SnapMode };

// snap

/** Is this gesture FREE of the grid: Ctrl or Cmd, read LIVE off each event. */
export const freeSnap = (e: {
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
}): boolean => run((x) => x.k7_g_free_snap(+e.ctrlKey, +e.metaKey) !== 0);

/** Read live off each pointer event: Shift frees the gesture, Ctrl keeps the event's own offset from the grid. */
export const snapMode = (e: {
  readonly shiftKey: boolean;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
}): SnapMode =>
  MODES[
    run((x) => x.k7_g_snap_mode(+e.shiftKey, +e.ctrlKey, +e.metaKey))
  ] as SnapMode;

/** `v` on the nearest multiple of `step`, or untouched when there is none: a step of 0 IS the free case. */
export function snapTo(v: number, step: number): number {
  const x = typed();
  try {
    return x.k7_g_snap_to(v, step);
  } catch (e) {
    return trapped(e);
  }
}

/** A clip move's offset in beats under `mode`, never before the song's start. */
export function moveOffset(
  dx: number,
  ppb: number,
  step: number,
  origStart: number,
  mode: SnapMode,
): number {
  const x = typed();
  try {
    return x.k7_g_move_offset(dx, ppb, step, origStart, MODE_CODE[mode]);
  } catch (e) {
    return trapped(e);
  }
}

// the piano roll

/** A drag on one note, addressed by key and by where it starts NOW. */
export interface RollDrag {
  key: number;
  time: number;
  dur: number;
  /** Which end is in hand, or the whole note. */
  mode: "move" | "start" | "end";
  /** How far it has travelled in time, in beats. */
  dBeats: number;
  /** How far it has travelled in PITCH, in semitones, positive up. */
  dKeys: number;
}

/** Where a drag has taken the note. */
export interface RollSpan {
  key: number;
  start: number;
  end: number;
}

const ROLL_CODE = { move: 0, start: 1, end: 2 } as const;

/** Where a drag has taken the note: ONE answer, read by the preview and by the commit. */
export function tSpan(t: RollDrag): RollSpan {
  const x = typed();
  try {
    x.k7_g_roll_span(
      ROLL_CODE[t.mode],
      t.key,
      t.time,
      t.dur,
      t.dBeats,
      t.dKeys,
    );
    const s = scalars(x);
    return { key: s[0] as number, start: s[1] as number, end: s[2] as number };
  } catch (e) {
    return trapped(e);
  }
}

/** Did this drag change the note's PITCH: which verb commits it. */
export const movesPitch = (t: RollDrag): boolean =>
  run(
    (x) =>
      x.k7_g_roll_moves_pitch(
        ROLL_CODE[t.mode],
        t.key,
        t.time,
        t.dur,
        t.dBeats,
        t.dKeys,
      ) !== 0,
  );

/** Has this drag anything to write at all: a press that has not travelled commits nothing. */
export const dragMoved = (t: RollDrag): boolean =>
  run(
    (x) =>
      x.k7_g_roll_drag_moved(
        ROLL_CODE[t.mode],
        t.key,
        t.time,
        t.dur,
        t.dBeats,
        t.dKeys,
      ) !== 0,
  );

/** The span a draw gesture covers in beats: the CELLS it was dragged across. */
export function drawSpan(
  a: number,
  b: number,
  step: number,
  origin: number,
  freeLen: number,
): { from: number; to: number } {
  return run((x) => {
    x.k7_g_draw_span(a, b, step, origin, freeLen);
    const s = scalars(x);
    return { from: s[0] as number, to: s[1] as number };
  });
}

/** A velocity from where the pointer sits INSIDE a note's row: 0 at its bottom edge, 1 at its top. */
export const velAt = (yInRow: number, rowH: number): number =>
  run((x) => x.k7_g_vel_at(yInRow, rowH));

/** A note as a group drag reads it. */
export interface GroupNote {
  key: number;
  time: number;
  duration: number;
  vel?: number;
}

/** The offset a SELECTION drag may take, clamped so the whole group stays on the keyboard and after beat 0. */
export function groupOffset(
  group: readonly GroupNote[],
  dBeats: number,
  dKeys: number,
): { dBeats: number; dKeys: number } {
  return run((x) => {
    const v = input(x, group.length * 3);
    group.forEach((n, i) => {
      v[i * 3] = n.key;
      v[i * 3 + 1] = n.time;
      v[i * 3 + 2] = n.duration;
    });
    x.k7_g_group_offset(group.length, dBeats, dKeys);
    const s = scalars(x);
    return { dBeats: s[0] as number, dKeys: s[1] as number };
  });
}

/** Where a selection drag sends every note, or why it cannot: a note that STAYS is in the way. */
export const planGroupMove = (
  group: readonly GroupNote[],
  others: readonly GroupNote[],
  dBeats: number,
  dKeys: number,
): GroupMove =>
  call<GroupMove>({
    op: "gesture.plan_group_move",
    args: { group, others, dBeats, dKeys },
  });

/** What the roll opens on and how far its canvas runs. */
export interface RollFit {
  loKey: number;
  hiKey: number;
  rows: number;
  loBeat: number;
  hiBeat: number;
  beats: number;
  placedEnd: number;
  canvasBeats: number;
  origin: number;
}

/** The view for one clip: FRAMED on the window with a margin when there is one, else on what is USED. */
export function rollFit(
  notes: readonly { time: number; duration: number; key: number }[],
  clipLength: number,
  barBeats: number,
  loop: { from: number; to: number; win: number } | null,
): RollFit {
  return run((x) => {
    const v = input(x, notes.length * 3);
    notes.forEach((n, i) => {
      v[i * 3] = n.time;
      v[i * 3 + 1] = n.duration;
      v[i * 3 + 2] = n.key;
    });
    x.k7_g_roll_fit(
      notes.length,
      clipLength,
      barBeats,
      maybe(loop?.from),
      loop?.to ?? 0,
      loop?.win ?? 0,
    );
    const s = scalars(x);
    return {
      loKey: s[0] as number,
      hiKey: s[1] as number,
      rows: s[2] as number,
      loBeat: s[3] as number,
      hiBeat: s[4] as number,
      beats: s[5] as number,
      placedEnd: s[6] as number,
      canvasBeats: s[7] as number,
      origin: s[8] as number,
    };
  });
}

/** Where a pointer is on the roll: the lane's left edge is the ABSOLUTE `origin`, a row is one key, and a pointer outside is pulled onto the canvas. */
export function rollPoint(
  dx: number,
  dy: number,
  origin: number,
  ppb: number,
  canvasBeats: number,
  rowH: number,
): { beat: number; key: number } {
  const x = typed();
  try {
    x.k7_g_roll_point(dx, dy, origin, ppb, canvasBeats, rowH);
    const s = scalars(x);
    return { beat: s[0] as number, key: s[1] as number };
  } catch (e) {
    return trapped(e);
  }
}

/** How far a note drag has taken the note from the press: the pointer `dx`, `dy` px away, snapped as `snap` says. */
export function rollDragDelta(
  t: Pick<RollDrag, "mode" | "time" | "dur">,
  dx: number,
  dy: number,
  ppb: number,
  rowH: number,
  step: number,
  snap: SnapMode,
  movesKeys: boolean,
): { dBeats: number; dKeys: number } {
  const x = typed();
  try {
    x.k7_g_roll_drag_delta(
      ROLL_CODE[t.mode],
      t.time,
      t.dur,
      dx,
      dy,
      ppb,
      rowH,
      step,
      MODE_CODE[snap],
      +movesKeys,
    );
    const s = scalars(x);
    return { dBeats: s[0] as number, dKeys: s[1] as number };
  } catch (e) {
    return trapped(e);
  }
}

/** The roll's scale: the framed rows fill the height, the framed beats the width, and zoom stretches the time axis. */
export const rollScale = (
  availW: number,
  availH: number,
  rows: number,
  beats: number,
  canvasBeats: number,
  zoomX: number,
  pxPerBeat: number,
): { rowH: number; ppb: number; laneW: number } =>
  run((x) => {
    x.k7_g_roll_scale(
      availW,
      availH,
      rows,
      beats,
      canvasBeats,
      zoomX,
      pxPerBeat,
    );
    const s = scalars(x);
    return { rowH: s[0] as number, ppb: s[1] as number, laneW: s[2] as number };
  });
