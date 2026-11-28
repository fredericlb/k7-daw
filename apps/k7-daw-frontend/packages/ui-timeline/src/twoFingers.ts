// twoFingers.ts — on a touch screen an editing surface moves under TWO fingers only: their midpoint pans it, their spread zooms it,
// both measured from the gesture's START so the whole gesture is absolute and cannot drift. One finger moves nothing (docs/panZoom.md).
// The one thing a reader would get wrong: the surface reads `touch-action: none`, so the browser's own one-finger scroll is gone ON PURPOSE.

import { type RefObject, useEffect, useRef } from "react";

/** How far the spread must change, as a fraction, before a two-finger pan also zooms. */
export const PINCH_SLOP = 0.04;

export interface FingerPoint {
  id: number;
  x: number;
  y: number;
}

export interface TwoFingerFrame {
  /** The spread now over the spread at the start; exactly 1 until it has moved past PINCH_SLOP. */
  ratio: number;
  /** The midpoint's travel since the start, in client pixels. */
  dx: number;
  dy: number;
  /** The midpoint at the start: the point a zoom is anchored on, before the pan carries it. */
  origin: { x: number; y: number };
}

export interface TwoFingerHandlers<T> {
  /** Two fingers went down: return the baseline the gesture is measured from, or false to refuse it. */
  onStart: () => T | false;
  /** The fingers moved, relative to the start. */
  onMove: (start: T, f: TwoFingerFrame) => void;
}

let held = 0;

/** A one-finger drag armed by a hold owns the touch: no two-finger gesture starts until the returned release. */
export function holdTwoFingers(): () => void {
  held++;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    held--;
  };
}

/** The gesture's state over the fingers on the surface; no DOM, so a test hands it plain points. */
export class TwoFingerGesture<T> {
  private live: {
    key: string;
    start: T;
    origin: { x: number; y: number };
    d0: number;
    zooming: boolean;
  } | null = null;
  private refused: string | null = null;

  constructor(private readonly handlers: () => TwoFingerHandlers<T>) {}

  /** The fingers now down on the surface; answers whether the gesture is ours, so the caller cancels the browser's. */
  update(points: readonly FingerPoint[]): boolean {
    if (points.length !== 2) {
      this.live = null;
      this.refused = null;
      return false;
    }
    const [a, b] = points;
    const key = [a.id, b.id].sort((x, y) => x - y).join(",");
    const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
    const d = Math.hypot(a.x - b.x, a.y - b.y);
    const L = this.live;
    if (!L || L.key !== key) {
      this.live = null;
      if (this.refused === key || held > 0 || d <= 0) return false;
      const start = this.handlers().onStart();
      if (start === false) {
        this.refused = key;
        return false;
      }
      this.live = { key, start, origin: mid, d0: d, zooming: false };
      return true;
    }
    if (!L.zooming && Math.abs(d / L.d0 - 1) > PINCH_SLOP) L.zooming = true;
    this.handlers().onMove(L.start, {
      ratio: L.zooming ? d / L.d0 : 1,
      dx: mid.x - L.origin.x,
      dy: mid.y - L.origin.y,
      origin: L.origin,
    });
    return true;
  }
}

/** Bind the two-finger gesture on `ref`'s element, which no longer scrolls under one finger; `deps` re-bind it when it mounts late. */
export function useTwoFingers<T>(
  ref: RefObject<HTMLElement | null>,
  handlers: TwoFingerHandlers<T>,
  deps: unknown[] = [],
) {
  const latest = useRef(handlers);
  latest.current = handlers;
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const g = new TwoFingerGesture(() => latest.current);
    // Only the fingers that came down on this surface: one elsewhere on the screen is not half of this gesture.
    const points = (e: TouchEvent): FingerPoint[] =>
      Array.from(e.touches)
        .filter((t) => t.target instanceof Node && el.contains(t.target))
        .map((t) => ({ id: t.identifier, x: t.clientX, y: t.clientY }));
    const onStart = (e: TouchEvent) => {
      g.update(points(e));
    };
    const onMove = (e: TouchEvent) => {
      // Cancelled for two fingers only: a list nested in the surface still scrolls under one, `touch-action` stopping at it.
      if (g.update(points(e)) && e.cancelable) e.preventDefault();
    };
    const onEnd = (e: TouchEvent) => {
      g.update(points(e));
    };
    const hadTouchAction = el.style.touchAction;
    el.style.touchAction = "none";
    el.addEventListener("touchstart", onStart, { passive: true });
    el.addEventListener("touchmove", onMove, { passive: false });
    el.addEventListener("touchend", onEnd);
    el.addEventListener("touchcancel", onEnd);
    return () => {
      el.style.touchAction = hadTouchAction;
      el.removeEventListener("touchstart", onStart);
      el.removeEventListener("touchmove", onMove);
      el.removeEventListener("touchend", onEnd);
      el.removeEventListener("touchcancel", onEnd);
    };
  }, [ref, ...deps]);
}

/** A scroll container that two fingers scroll and one finger leaves where it is. */
export function useTwoFingerScroll(
  ref: RefObject<HTMLElement | null>,
  deps: unknown[] = [],
) {
  useTwoFingers(
    ref,
    {
      onStart: () => {
        const el = ref.current;
        return el ? { left: el.scrollLeft, top: el.scrollTop } : false;
      },
      onMove: (s, f) => {
        const el = ref.current;
        if (!el) return;
        el.scrollLeft = s.left - f.dx;
        el.scrollTop = s.top - f.dy;
      },
    },
    deps,
  );
}
