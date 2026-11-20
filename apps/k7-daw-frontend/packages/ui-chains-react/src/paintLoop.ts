// paintLoop.ts — the draw loop a live canvas widget shares: sized by a ResizeObserver, steered by a
// dirty flag, capped at WOBBLE_FPS, started by `wake()` and stopped by stillness.
// Called inside the widget's own effect, so the effect keeps what it attaches beside it — docs/waveformWidget.md.

import { WOBBLE_FPS } from "@studio/ui-chains/cableCanvas";
import type { RefObject } from "react";

const NOOP = (): void => {};

export interface PaintLoopOptions {
  /** Checked on every frame before the cap: false skips the frame without rescheduling it. */
  canPaint?: () => boolean;
  fps?: number;
}

export interface PaintLoop {
  /** Schedules one frame unless one is already scheduled or the loop is stopped. */
  wake: () => void;
  /** Disconnects the observer, cancels the pending frame and resets `wakeRef`. */
  stop: () => void;
}

// Starts the loop on `canvas`: `paint` gets the CSS box and runs only when dirty, sized and allowed.
export function startPaintLoop(
  canvas: HTMLCanvasElement,
  dirtyRef: RefObject<boolean>,
  wakeRef: RefObject<() => void>,
  paint: (W: number, H: number) => void,
  { canPaint, fps = WOBBLE_FPS }: PaintLoopOptions = {},
): PaintLoop {
  let W = 0;
  let H = 0;
  let raf = 0;
  let last = 0;
  let live = true;
  const minDt = 1000 / fps;

  const tick = (now: number): void => {
    raf = 0;
    if (!dirtyRef.current || (canPaint && !canPaint()) || W <= 0 || H <= 0)
      return;
    if (now - last < minDt) {
      raf = requestAnimationFrame(tick);
      return;
    }
    last = now;
    dirtyRef.current = false;
    paint(W, H);
  };
  const wake = (): void => {
    if (!raf && live) raf = requestAnimationFrame(tick);
  };
  wakeRef.current = wake;

  // The box comes from a ResizeObserver: measuring inside the draw flushes style every frame.
  const ro = new ResizeObserver((entries) => {
    const box = entries[0]?.contentRect;
    if (!box) return;
    W = box.width;
    H = box.height;
    dirtyRef.current = true;
    wake();
  });
  ro.observe(canvas);
  // The observer's first callback is async; a mounted widget still owes one paint.
  wake();

  return {
    wake,
    stop: () => {
      live = false;
      ro.disconnect();
      if (raf) cancelAnimationFrame(raf);
      wakeRef.current = NOOP;
    },
  };
}
