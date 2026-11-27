// gestureTimeline.ts — the timeline's own arithmetic from Rust: an edge drag, a fade, a crossfade's size, the song's extent and the scale it is drawn at.
// A reader would get wrong that an edge answers in beats from pixels: the grab's ppb and unit are captured at the press, so a zoom mid-drag cannot slide the edge.

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

/** Where a resize leaves an event's edges: the pointer `dx` px from the grab at `ppb`, on a grid of `unit` beats. */
export function edgeResize(
  edge: "start" | "end",
  origStart: number,
  origEnd: number,
  dx: number,
  ppb: number,
  unit: number,
  mode: SnapMode,
): { start: number; end: number } {
  const x = typed();
  try {
    x.k7_g_edge_resize(
      edge === "start" ? 0 : 1,
      origStart,
      origEnd,
      dx,
      ppb,
      unit,
      MODE_CODE[mode],
    );
    const s = scalars(x);
    return { start: s[0] as number, end: s[1] as number };
  } catch (e) {
    return trapped(e);
  }
}

/** A fade's live length in beats: whole beats unless free, never past `span`. */
export function fadeLen(
  which: "fadeIn" | "fadeOut",
  dx: number,
  ppb: number,
  span: number,
  orig: number,
  free: boolean,
): number {
  const x = typed();
  try {
    return x.k7_g_fade_len(
      which === "fadeIn" ? 1 : 0,
      dx,
      ppb,
      span,
      orig,
      +free,
    );
  } catch (e) {
    return trapped(e);
  }
}

/** A crossfade's length in beats for a pointer `dx` px from where the press began. */
export const crossfadeLen = (dx: number, ppb: number): number =>
  run((x) => x.k7_g_crossfade_len(dx, ppb));

/** The song's extent in whole bars: the caret moves within it. */
export const contentBeats = (
  ends: readonly number[],
  timelineLength: number,
  barBeats: number,
): number =>
  run((x) => {
    input(x, ends.length).set(ends);
    return x.k7_g_content_beats(ends.length, timelineLength, barBeats);
  });

/** What a timeline is drawn at. */
export interface TimelineLayout {
  beatLo: number;
  beatHi: number;
  /** Pixels per beat. */
  ppb: number;
  /** The width of the lane in pixels. */
  laneW: number;
  /** The last beat drawn: the zoom's end, or the song's rounded-up extent. */
  lastBeat: number;
  /** Beats per second. */
  bps: number;
}

/** The layout for a pane of `paneW` px with a `gutter` that does not scale, at `zoomX`, narrowed to `view` or not. */
export const timelineLayout = (
  view: { beatStart: number; beatEnd: number } | null,
  content: number,
  zoomX: number,
  paneW: number,
  gutter: number,
  pxPerBeat: number,
  tempo: number,
): TimelineLayout =>
  run((x) => {
    x.k7_g_timeline_layout(
      maybe(view?.beatStart),
      view?.beatEnd ?? 0,
      content,
      zoomX,
      paneW,
      gutter,
      pxPerBeat,
      tempo,
    );
    const s = scalars(x);
    return {
      beatLo: s[0] as number,
      beatHi: s[1] as number,
      ppb: s[2] as number,
      laneW: s[3] as number,
      lastBeat: s[4] as number,
      bps: s[5] as number,
    };
  });
