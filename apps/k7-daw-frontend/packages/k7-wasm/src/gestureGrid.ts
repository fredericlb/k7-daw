// gestureGrid.ts — the grid's caret and rectangle, the clip panel's window and a take's counter, from Rust.
// A reader would get wrong that a window's start is handed back UNCLAMPED on purpose: `barWindow` owns the edges.

import { call } from "./abi.ts";
import { maybe, orNull, run, scalars, trapped, typed } from "./gestureIo.ts";
import type { SourceMap, WarpPoint } from "./types/index.ts";

export type { SourceMap, WarpPoint };

// the grid's caret and rectangle

/** One cell, by index; row 0 is the TOP row. */
export interface GridCell {
  row: number;
  col: number;
}

/** The `v` selection: an anchor that stays and a head that moves. */
export interface GridSelection {
  anchor: GridCell;
  head: GridCell;
}

/** How big the grid is. */
export interface GridBounds {
  rows: number;
  cols: number;
}

/** A selection as a normalised box, INCLUSIVE at both ends. */
export interface GridBox {
  rowLo: number;
  rowHi: number;
  colLo: number;
  colHi: number;
}

const cell = (s: Float64Array): GridCell => ({
  row: s[0] as number,
  col: s[1] as number,
});
const box = (s: Float64Array): GridBox => ({
  rowLo: s[0] as number,
  rowHi: s[1] as number,
  colLo: s[2] as number,
  colHi: s[3] as number,
});

/** `c` pinned inside the grid: clamped rather than wrapped. */
export const clampCell = (c: GridCell, b: GridBounds): GridCell =>
  run((x) => {
    x.k7_g_clamp_cell(c.row, c.col, b.rows, b.cols);
    return cell(scalars(x));
  });

/** `c` moved by a whole number of rows and columns, clamped. */
export const moveCell = (
  c: GridCell,
  dRow: number,
  dCol: number,
  b: GridBounds,
): GridCell =>
  run((x) => {
    x.k7_g_move_cell(c.row, c.col, dRow, dCol, b.rows, b.cols);
    return cell(scalars(x));
  });

/** The rectangle two cells describe, whichever end was dragged. */
export const selBox = (sel: GridSelection): GridBox =>
  run((x) => {
    x.k7_g_sel_box(sel.anchor.row, sel.anchor.col, sel.head.row, sel.head.col);
    return box(scalars(x));
  });

/** The caret's own cell as a one-by-one box. */
export const cellBox = (c: GridCell): GridBox =>
  run((x) => {
    x.k7_g_cell_box(c.row, c.col);
    return box(scalars(x));
  });

/** The selection's span in COLUMNS, half-open. */
export const colSpan = (sel: GridSelection): { from: number; to: number } =>
  run((x) => {
    x.k7_g_col_span(sel.anchor.row, sel.anchor.col, sel.head.row, sel.head.col);
    const s = scalars(x);
    return { from: s[0] as number, to: s[1] as number };
  });

/** A grid column in seconds of the song, the unit every transport call takes. */
export const songSecondsOfCol = (
  col: number,
  unit: number,
  clipStart: number,
  tempo: number,
): number =>
  run((x) => x.k7_g_song_seconds_of_col(col, unit, clipStart, tempo));

// the clip panel's window

/** The interval the clip plays, in seconds of the file. */
export interface BarSpan {
  from: number;
  to: number;
}

export interface BarWindow {
  /** Bars actually shown: never more than the interval has. */
  shown: number;
  /** How many bars the interval spans, rounded up. */
  allBars: number;
  /** Seconds one bar spans. */
  barSec: number;
  /** The span to draw, in seconds from the start of the file. */
  view: { start: number; end: number };
}

/** The window on a clip: `want` bars of its interval from `startBar`, CLAMPED here. */
export const barWindow = (
  span: BarSpan,
  want: number,
  tempo: number,
  beatsPerBar: number,
  startBar = 0,
): BarWindow =>
  run((x) => {
    x.k7_g_bar_window(span.from, span.to, want, tempo, beatsPerBar, startBar);
    const s = scalars(x);
    return {
      shown: s[0] as number,
      allBars: s[1] as number,
      barSec: s[2] as number,
      view: { start: s[3] as number, end: s[4] as number },
    };
  });

/** Seconds one bar spans, guarded on a tempo or a time signature the doc has not answered yet. */
export const barSec = (tempo: number, beatsPerBar: number): number =>
  run((x) => x.k7_g_bar_sec(tempo, beatsPerBar));

/** The window the panel OPENS on: the act span framed inside the reach. */
export const frameWindow = (
  reach: BarSpan,
  act: BarSpan,
  barSec: number,
  margin?: number,
): { bars: number; start: number } =>
  run((x) => {
    x.k7_g_frame_window(
      reach.from,
      reach.to,
      act.from,
      act.to,
      barSec,
      maybe(margin),
    );
    const s = scalars(x);
    return { bars: s[0] as number, start: s[1] as number };
  });

/** The next bar count for one wheel notch. */
export const zoomBars = (
  bars: number,
  deltaY: number,
  allBars: number,
): number => run((x) => x.k7_g_zoom_bars(bars, deltaY, allBars));

/** The start bar that keeps the moment under the cursor where it is: unclamped, `barWindow` owning the edges. */
export const anchorBars = (
  view: { start: number; end: number },
  frac: number,
  nextBars: number,
  barSec: number,
  spanFrom = 0,
): number =>
  run((x) =>
    x.k7_g_anchor_bars(view.start, view.end, frac, nextBars, barSec, spanFrom),
  );

/** The slice of the file an event plays, in file seconds. */
export interface ActWindow {
  from: number;
  to: number;
}

/** The transport's position as a second of THIS FILE, or null when it is outside the event. */
export function clipPlayhead(
  songSeconds: number,
  placedAt: number | null,
  act: ActWindow,
  tempo: number,
): number | null {
  const x = typed();
  try {
    return orNull(
      x.k7_g_clip_playhead(
        songSeconds,
        maybe(placedAt),
        act.from,
        act.to,
        tempo,
      ),
    );
  } catch (e) {
    return trapped(e);
  }
}

/** The line that pins the source moment under `atSec` to the beat under `toSec`. */
export const warpLine = (
  clip: string,
  atSec: number,
  toSec: number,
  map: SourceMap,
  tempo: number,
  marks: readonly WarpPoint[] = [],
): string =>
  call<string>({
    op: "gesture.warp_line",
    args: { clip, atSec, toSec, map, tempo, marks },
  });

// the take's counter

export interface TakeCount {
  phase: "count-in" | "recording";
  /** Beats REMAINING in the pre-roll, or beats recorded so far. */
  beats: number;
}

/** Where a take is, from the transport's own beat positions. */
export function takeCounter(
  startBeat: number,
  curBeat: number,
  countInBeats: number,
): TakeCount {
  const x = typed();
  try {
    x.k7_g_take_counter(startBeat, curBeat, countInBeats);
    const s = scalars(x);
    return {
      phase: s[0] === 0 ? "count-in" : "recording",
      beats: s[1] as number,
    };
  } catch (e) {
    return trapped(e);
  }
}

/** The beat the take's MATERIAL starts on: the transport's start plus the count-in. */
export const recordingFrom = (
  startBeat: number,
  countInBeats: number,
): number => run((x) => x.k7_g_recording_from(startBeat, countInBeats));

/** The take's position as bar:beat, 1-based. */
export const barBeat = (
  recordedBeats: number,
  beatsPerBar: number,
): { bar: number; beat: number } =>
  run((x) => {
    x.k7_g_bar_beat(recordedBeats, beatsPerBar);
    const s = scalars(x);
    return { bar: s[0] as number, beat: s[1] as number };
  });
