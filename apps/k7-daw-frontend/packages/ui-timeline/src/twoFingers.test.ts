// The two-finger gesture: their midpoint pans and their spread zooms, both from the start; one finger, or three, is no gesture.

import { describe, expect, it } from "vitest";
import {
  type FingerPoint,
  holdTwoFingers,
  PINCH_SLOP,
  type TwoFingerFrame,
  TwoFingerGesture,
} from "./twoFingers";

function rig(refuse = false) {
  const starts: number[] = [];
  const frames: TwoFingerFrame[] = [];
  let n = 0;
  const g = new TwoFingerGesture<number>(() => ({
    onStart: () => {
      if (refuse) return false;
      starts.push(++n);
      return n;
    },
    onMove: (_s, f) => frames.push(f),
  }));
  return { g, starts, frames };
}

const f = (id: number, x: number, y: number): FingerPoint => ({ id, x, y });

describe("TwoFingerGesture", () => {
  it("one finger is no gesture: nothing starts and the browser is not told", () => {
    const { g, starts, frames } = rig();
    expect(g.update([f(1, 10, 10)])).toBe(false);
    expect(g.update([f(1, 90, 90)])).toBe(false);
    expect(starts).toEqual([]);
    expect(frames).toEqual([]);
  });

  it("the midpoint's travel from the start is the pan, with no zoom while the spread holds", () => {
    const { g, starts, frames } = rig();
    expect(g.update([f(1, 0, 0), f(2, 100, 0)])).toBe(true);
    g.update([f(1, 30, 20), f(2, 130, 20)]);
    g.update([f(1, 50, -10), f(2, 150, -10)]);
    expect(starts).toEqual([1]);
    expect(frames.map((x) => [x.dx, x.dy, x.ratio])).toEqual([
      [30, 20, 1],
      [50, -10, 1],
    ]);
    expect(frames[1].origin).toEqual({ x: 50, y: 0 });
  });

  it("a spread that wavers inside the slop does not zoom; past it, the ratio is against the start", () => {
    const { g, frames } = rig();
    g.update([f(1, 0, 0), f(2, 100, 0)]);
    g.update([f(1, 0, 0), f(2, 100 * (1 + PINCH_SLOP / 2), 0)]);
    g.update([f(1, 0, 0), f(2, 200, 0)]);
    g.update([f(1, 0, 0), f(2, 100, 0)]);
    expect(frames.map((x) => x.ratio)).toEqual([1, 2, 1]);
  });

  it("the fingers' order does not matter, a third finger ends it, and a new pair starts afresh", () => {
    const { g, starts, frames } = rig();
    g.update([f(1, 0, 0), f(2, 100, 0)]);
    g.update([f(2, 110, 0), f(1, 10, 0)]);
    expect(g.update([f(1, 0, 0), f(2, 100, 0), f(3, 50, 50)])).toBe(false);
    g.update([f(1, 0, 0), f(3, 100, 0)]);
    expect(starts).toEqual([1, 2]);
    expect(frames.length).toBe(1);
  });

  it("lifting one finger ends it; putting it back starts from where they are", () => {
    const { g, starts, frames } = rig();
    g.update([f(1, 0, 0), f(2, 100, 0)]);
    g.update([f(1, 0, 0)]);
    g.update([f(1, 0, 0), f(2, 200, 0)]);
    g.update([f(1, 10, 0), f(2, 210, 0)]);
    expect(starts).toEqual([1, 2]);
    expect(frames.map((x) => x.dx)).toEqual([10]);
  });

  it("a refused start is asked once per pair, and leaves the touch to the browser", () => {
    const { g, frames } = rig(true);
    expect(g.update([f(1, 0, 0), f(2, 100, 0)])).toBe(false);
    expect(g.update([f(1, 10, 0), f(2, 110, 0)])).toBe(false);
    expect(frames).toEqual([]);
  });

  it("no gesture starts while a held drag owns the touch", () => {
    const { g, starts } = rig();
    const release = holdTwoFingers();
    expect(g.update([f(1, 0, 0), f(2, 100, 0)])).toBe(false);
    g.update([f(1, 0, 0)]);
    release();
    release();
    expect(g.update([f(1, 0, 0), f(2, 100, 0)])).toBe(true);
    expect(starts).toEqual([1]);
  });
});
