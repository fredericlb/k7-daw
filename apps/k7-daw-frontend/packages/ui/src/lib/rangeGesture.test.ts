import { describe, expect, it } from "vitest";
import { rangeGesture } from "./rangeGesture";

describe("rangeGesture", () => {
  it("commits nothing when a gesture ends without a change (mount, then blur)", () => {
    const seen: number[] = [];
    const g = rangeGesture((v) => seen.push(v));
    g.end();
    g.end();
    expect(seen).toEqual([]);
  });

  it("commits the last value shown, once, however many events end the gesture", () => {
    const seen: number[] = [];
    const g = rangeGesture((v) => seen.push(v));
    g.change(-12);
    g.change(-6.5);
    g.end();
    g.end();
    expect(seen).toEqual([-6.5]);
  });

  it("starts afresh after a commit", () => {
    const seen: number[] = [];
    const g = rangeGesture((v) => seen.push(v));
    g.change(1);
    g.end();
    g.end();
    g.change(2);
    g.end();
    expect(seen).toEqual([1, 2]);
  });
});
