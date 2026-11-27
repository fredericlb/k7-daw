// gestureTicks.ts — the rulers' and the panes' arithmetic from Rust: the tick ladder, the labels' spacing, the clock's text, the wheel and the anchored zoom.
// A reader would get wrong that the lists cross by the module's number buffers: a tick list is copied in for a call, never held.

import { call } from "./abi.ts";
import {
  input,
  listed,
  maybe,
  orNull,
  run,
  scalars,
  text,
  trapped,
  typed,
} from "./gestureIo.ts";
import type { RulerTick } from "./types/index.ts";

export type { RulerTick };

/** What the ruler is READING IN: not what the song is placed in, which is always beats. */
export type RulerMode = "bars" | "seconds";

/** The COARSEST candidate at least `minPx` apart: touching labels are unreadable, sparse ones are not. */
export const chooseTick = (
  candidates: readonly number[],
  pxPerUnit: number,
  minPx?: number,
): number =>
  run((x) => {
    input(x, candidates.length).set(candidates);
    return x.k7_g_choose_tick(candidates.length, pxPerUnit, maybe(minPx));
  });

/** The seconds ladder for a song that has a picture at `fps`: below a second it counts FRAMES. */
export const secondTicks = (fps?: number): number[] =>
  run((x) => listed(x, x.k7_g_second_ticks(maybe(fps))));

/** A seconds tick's label: `m:ss` on whole seconds, and between them the FRAME it is when the ladder counts frames. */
export const formatSecondTick = (
  sec: number,
  step: number,
  fps?: number,
): string =>
  run((x) => text(x, x.k7_g_format_second_tick(sec, step, maybe(fps))));

/** `m:ss`, plus decimals only when the STEP needs them. */
export const formatClock = (sec: number, step = 1): string =>
  run((x) => text(x, x.k7_g_format_clock(sec, step)));

/** A position in the song as the transport clock writes it: the hour only when there is one. */
export const formatAt = (seconds: number): string =>
  run((x) => text(x, x.k7_g_format_at(seconds)));

/** Every tick of `step` from 0 up to and including `end`. */
export const ticksUpTo = (end: number, step: number): number[] =>
  run((x) => listed(x, x.k7_g_ticks_up_to(end, step)));

/** The first tick strictly after (`dir` 1) or before (-1) `value` in an ascending list, or null past the end. */
export const adjacentTick = (
  ticks: readonly number[],
  value: number,
  dir: 1 | -1,
): number | null =>
  run((x) => {
    input(x, ticks.length).set(ticks);
    return orNull(x.k7_g_adjacent_tick(ticks.length, value, dir));
  });

/** The tick nearest `value`, or null when the nearest is further than `tolerance` away. */
export function nearestTick(
  ticks: readonly number[],
  value: number,
  tolerance = Number.POSITIVE_INFINITY,
): number | null {
  const x = typed();
  try {
    input(x, ticks.length).set(ticks);
    return orNull(x.k7_g_nearest_tick(ticks.length, value, tolerance));
  } catch (e) {
    return trapped(e);
  }
}

/** How far apart two gridlines must be for `widest` to fit: pass the widest the ruler CAN print. */
export const labelMinPx = (widest: string, charPx?: number): number =>
  run((x) => x.k7_g_label_min_px(widest.length, maybe(charPx)));

/** Label every Nth gridline, N a power of two, so labels sit at least `minPx` apart. */
export const labelStride = (spacingPx: number, minPx?: number): number =>
  run((x) => x.k7_g_label_stride(spacingPx, maybe(minPx)));

/** Whether the gridline at `index` keeps its label; indices count from the ruler's ORIGIN. */
export const labelled = (index: number, stride: number): boolean =>
  run((x) => x.k7_g_labelled(index, stride) !== 0);

/** The unit a ruler should draw, in BEATS: a musical ladder, floored at a 64th. */
export const subdivideBeats = (
  barBeats: number,
  ppb: number,
  maxPx?: number,
): number => run((x) => x.k7_g_subdivide_beats(barBeats, ppb, maybe(maxPx)));

/** The EXTRA times between the ticks a ruler was handed, split per PAIR once its columns open up. */
export const subdivideGrid = (
  times: readonly number[],
  pxPerSec: number,
  maxPx?: number,
): number[] =>
  run((x) => {
    input(x, times.length).set(times);
    return listed(
      x,
      x.k7_g_subdivide_grid(times.length, pxPerSec, maybe(maxPx)),
    );
  });

/** A ruler's constants, read once the module is ready: the widest column, the finest grid split. */
export const maxColumnPx = (): number => run((x) => x.k7_g_const(0));
export const maxGridSplit = (): number => run((x) => x.k7_g_const(1));
export const minLabelPx = (): number => run((x) => x.k7_g_const(2));
export const monoCharPx = (): number => run((x) => x.k7_g_const(3));
export const labelGapPx = (): number => run((x) => x.k7_g_const(4));
export const maxLanePx = (): number => run((x) => x.k7_g_const(6));

/** The ticks a ruler draws, in BEATS whichever unit it reads in. */
export const rulerTicks = (
  mode: RulerMode,
  beatLo: number,
  lastBeat: number,
  unit: number,
  barBeats: number,
  bps: number,
  secStep: number,
  view?: { lo: number; hi: number },
  fps?: number,
): RulerTick[] =>
  call<RulerTick[]>({
    op: "gesture.ruler_ticks",
    args: { mode, beatLo, lastBeat, unit, barBeats, bps, secStep, view, fps },
  });

/** The scale a ruler draws at: ONE function, so the timeline and the clip editors cannot pick different ladders. */
export const rulerScale = (
  mode: RulerMode,
  ppb: number,
  barBeats: number,
  bps: number,
  lastBeat: number,
  fps?: number,
): { unit: number; secStep: number; barStride: number } =>
  run((x) => {
    x.k7_g_ruler_scale(
      mode === "bars" ? 0 : 1,
      ppb,
      barBeats,
      bps,
      lastBeat,
      maybe(fps),
    );
    const s = scalars(x);
    return {
      unit: s[0] as number,
      secStep: s[1] as number,
      barStride: s[2] as number,
    };
  });

/** A wheel event that zooms whatever the DEVICE is: a pinch, or the modifier. */
export const isZoomWheel = (e: {
  ctrlKey: boolean;
  metaKey: boolean;
}): boolean => run((x) => x.k7_g_is_zoom_wheel(+e.ctrlKey, +e.metaKey) !== 0);

/** A mouse WHEEL rather than a surface, wrong in one direction only: towards the surface. */
export const isMouseWheel = (e: {
  deltaX: number;
  deltaY: number;
  deltaMode: number;
  wheelDeltaY?: number;
}): boolean =>
  run(
    (x) =>
      x.k7_g_is_mouse_wheel(
        e.deltaX,
        e.deltaY,
        e.deltaMode,
        maybe(e.wheelDeltaY),
      ) !== 0,
  );

/** How much one wheel event zooms, a pinch being unable to share the notch's answer. */
export const wheelFactor = (delta: number, pinch: boolean): number =>
  run((x) => x.k7_g_wheel_factor(delta, +pinch));

/** How far a pane of `width` px may zoom before the browser can no longer lay it out. */
export const zoomCeiling = (width: number): number =>
  run((x) => x.k7_g_zoom_ceiling(width));

/** The zoom a wheel event asks for from `cur`, kept between `min` and `ceiling`. */
export const wheelZoomTo = (
  cur: number,
  delta: number,
  pinch: boolean,
  min: number,
  ceiling: number,
): number => run((x) => x.k7_g_wheel_zoom_to(cur, delta, +pinch, min, ceiling));

/** The zoom two fingers ask for: the spread now over the spread at the start. */
export const pinchZoomTo = (
  startZoom: number,
  dist: number,
  startDist: number,
  min: number,
  ceiling: number,
): number =>
  run((x) => x.k7_g_pinch_zoom_to(startZoom, dist, startDist, min, ceiling));

/** The scrollLeft to apply after the width changes from `cur` to `next`, keeping what is under `offsetX` fixed. */
export const anchoredScrollLeft = (
  scrollLeft: number,
  offsetX: number,
  gutter: number,
  cur: number,
  next: number,
): number =>
  run((x) =>
    x.k7_g_anchored_scroll_left(scrollLeft, offsetX, gutter, cur, next),
  );
