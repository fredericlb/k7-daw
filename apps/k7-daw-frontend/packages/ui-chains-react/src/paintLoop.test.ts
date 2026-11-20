// The shared canvas loop: it paints once sized and dirty, holds to its frame cap, skips a frame
// `canPaint` refuses, and after `stop` schedules nothing. See docs/waveformWidget.md.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { startPaintLoop } from "./paintLoop";

let frames: Map<number, (now: number) => void>;
let nextId: number;
let observers: { cb: ResizeObserverCallback; disconnected: boolean }[];

// Runs every scheduled frame once, at time `now`.
function flush(now: number): void {
  const due = [...frames.values()];
  frames.clear();
  for (const f of due) f(now);
}
// Fires the last observer with a box of `w` x `h`.
function resize(w: number, h: number): void {
  const o = observers[observers.length - 1];
  o.cb(
    [{ contentRect: { width: w, height: h } } as ResizeObserverEntry],
    {} as ResizeObserver,
  );
}

beforeEach(() => {
  frames = new Map();
  nextId = 1;
  observers = [];
  vi.stubGlobal("requestAnimationFrame", (f: (now: number) => void) => {
    const id = nextId++;
    frames.set(id, f);
    return id;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
  vi.stubGlobal(
    "ResizeObserver",
    class {
      entry: { cb: ResizeObserverCallback; disconnected: boolean };
      constructor(cb: ResizeObserverCallback) {
        this.entry = { cb, disconnected: false };
        observers.push(this.entry);
      }
      observe(): void {}
      disconnect(): void {
        this.entry.disconnected = true;
      }
    },
  );
});
afterEach(() => vi.unstubAllGlobals());

const canvas = {} as HTMLCanvasElement;

describe("startPaintLoop", () => {
  it("waits for a size, then paints once and stops", () => {
    const paint = vi.fn();
    const dirty = { current: true };
    const wake = { current: () => {} };
    startPaintLoop(canvas, dirty, wake, paint);
    flush(1000);
    expect(paint).not.toHaveBeenCalled();
    resize(40, 20);
    flush(1000);
    expect(paint).toHaveBeenCalledWith(40, 20);
    expect(dirty.current).toBe(false);
    expect(frames.size).toBe(0);
  });

  it("holds a frame that comes too soon until the cap allows it", () => {
    const paint = vi.fn();
    const dirty = { current: true };
    const wake = { current: () => {} };
    startPaintLoop(canvas, dirty, wake, paint, { fps: 10 });
    resize(10, 10);
    flush(1000);
    dirty.current = true;
    wake.current();
    flush(1050);
    expect(paint).toHaveBeenCalledTimes(1);
    expect(frames.size).toBe(1);
    flush(1100);
    expect(paint).toHaveBeenCalledTimes(2);
  });

  it("skips a frame canPaint refuses, keeping it dirty", () => {
    const paint = vi.fn();
    const dirty = { current: true };
    const wake = { current: () => {} };
    let busy = true;
    const loop = startPaintLoop(canvas, dirty, wake, paint, {
      canPaint: () => !busy,
    });
    resize(10, 10);
    flush(1000);
    expect(paint).not.toHaveBeenCalled();
    expect(dirty.current).toBe(true);
    busy = false;
    loop.wake();
    flush(1100);
    expect(paint).toHaveBeenCalledTimes(1);
  });

  it("stops: no frame left, the observer gone, wakeRef reset", () => {
    const dirty = { current: true };
    const wake = { current: () => {} };
    const loop = startPaintLoop(canvas, dirty, wake, vi.fn());
    const armed = wake.current;
    loop.stop();
    expect(frames.size).toBe(0);
    expect(observers[0].disconnected).toBe(true);
    expect(wake.current).not.toBe(armed);
    loop.wake();
    expect(frames.size).toBe(0);
  });
});
