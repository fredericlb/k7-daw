// A double tap with two fingers is undo, with three redo; anything that moves, lasts, or mixes counts is neither.

import { describe, expect, it } from "vitest";
import { MultiTap, type TapPoint } from "./multiTap";

/** A recogniser on a hand-moved clock, and what it fired. */
function rig() {
  let t = 0;
  const fired: number[] = [];
  const g = new MultiTap(
    (n) => fired.push(n),
    () => t,
  );
  const pts = (n: number, dx = 0): TapPoint[] =>
    Array.from({ length: n }, (_, i) => ({ id: i, x: 100 * i + dx, y: 50 }));
  const tap = (n: number, ms = 100) => {
    g.down(pts(n), n);
    t += ms;
    g.up(0);
  };
  return {
    g,
    fired,
    pts,
    tap,
    wait: (ms: number) => {
      t += ms;
    },
  };
}

describe("a multi-finger double tap", () => {
  it("fires 2 for two fingers twice, 3 for three", () => {
    const r = rig();
    r.tap(2);
    r.wait(150);
    r.tap(2);
    r.wait(500);
    r.tap(3);
    r.wait(150);
    r.tap(3);
    expect(r.fired).toEqual([2, 3]);
  });

  it("needs the second tap soon after the first", () => {
    const r = rig();
    r.tap(2);
    r.wait(600);
    r.tap(2);
    expect(r.fired).toEqual([]);
  });

  it("is not a pinch, a slow press, a single finger, or a two then a three", () => {
    const r = rig();
    r.g.down(r.pts(2), 2);
    r.g.move(r.pts(2, 40));
    r.g.up(0);
    r.wait(100);
    r.tap(2);
    expect(r.fired).toEqual([]);
    r.wait(600);
    r.tap(2, 500);
    r.wait(100);
    r.tap(2);
    expect(r.fired).toEqual([]);
    r.wait(600);
    r.tap(1);
    r.wait(100);
    r.tap(1);
    r.wait(600);
    r.tap(2);
    r.wait(100);
    r.tap(3);
    expect(r.fired).toEqual([]);
  });

  it("counts fingers that land one after the other as one tap", () => {
    const r = rig();
    for (let i = 0; i < 2; i++) {
      r.g.down([{ id: 0, x: 0, y: 0 }], 1);
      r.wait(30);
      r.g.down([{ id: 1, x: 100, y: 0 }], 2);
      r.wait(60);
      r.g.up(1);
      r.wait(10);
      r.g.up(0);
      r.wait(120);
    }
    expect(r.fired).toEqual([2]);
  });
});
