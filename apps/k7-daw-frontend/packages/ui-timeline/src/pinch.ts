// pinch.ts — two fingers on a touchscreen, measured from the gesture's START: the baseline is taken once,
// every move is a ratio against it, so the whole pinch is absolute and cannot drift. TOUCH events, bound
// natively (React's are passive and cannot stop the page's own pinch). See docs/panZoom.md.

import { type RefObject, useEffect, useRef } from "react";

/** The distance between the first two touch points. */
const spread = (t: TouchList) =>
  Math.hypot(t[0].clientX - t[1].clientX, t[0].clientY - t[1].clientY);

export interface PinchHandlers<T> {
  /** Two fingers went down: return the baseline the pinch will be measured from, or false to refuse the gesture. */
  onStart: () => T | false;
  /** The fingers moved: `ratio` is the spread now over the spread at the start, `centre` their midpoint in client pixels. */
  onPinch: (start: T, ratio: number, centre: { x: number; y: number }) => void;
}

/** Bind a two-finger pinch on `ref`'s element; `deps` re-bind it when the element mounts late (see useHZoom's). */
export function usePinch<T>(
  ref: RefObject<HTMLElement | null>,
  handlers: PinchHandlers<T>,
  deps: unknown[] = [],
) {
  const latest = useRef(handlers);
  latest.current = handlers;
  // biome-ignore lint/correctness/useExhaustiveDependencies: `deps` is the caller's own re-bind list.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let live: { start: T; dist: number } | null = null;
    const onStart = (e: TouchEvent) => {
      if (e.touches.length !== 2) return;
      const dist = spread(e.touches);
      const start = latest.current.onStart();
      live = start === false || dist <= 0 ? null : { start, dist };
    };
    const onMove = (e: TouchEvent) => {
      if (!live || e.touches.length !== 2) return;
      e.preventDefault();
      const d = spread(e.touches);
      if (d <= 0) return;
      const t = e.touches;
      latest.current.onPinch(live.start, d / live.dist, {
        x: (t[0].clientX + t[1].clientX) / 2,
        y: (t[0].clientY + t[1].clientY) / 2,
      });
    };
    const onEnd = (e: TouchEvent) => {
      if (e.touches.length < 2) live = null;
    };
    el.addEventListener("touchstart", onStart, { passive: true });
    el.addEventListener("touchmove", onMove, { passive: false });
    el.addEventListener("touchend", onEnd);
    el.addEventListener("touchcancel", onEnd);
    return () => {
      el.removeEventListener("touchstart", onStart);
      el.removeEventListener("touchmove", onMove);
      el.removeEventListener("touchend", onEnd);
      el.removeEventListener("touchcancel", onEnd);
    };
  }, [ref, ...deps]);
}
