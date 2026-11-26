// A shot's animation as written: a split whose tail no longer plays what the shot did is what would go wrong silently.
// Its value at an instant is the Rust crate's, tested there.

import { valueAt } from "@studio/k7-wasm/schema";
import { describe, expect, it } from "vitest";
import { type Animate, AnimateSchema, keysOf, splitAnimate } from "./keyframes";

// `x` of a shot at `t`, through the Rust crate's evaluation.
const xAt = (a: Animate, t: number): number => valueAt(keysOf(a.x ?? 0), t);

describe("splitAnimate", () => {
  const animate = {
    x: [[0, 0], { t: 4, v: 400, ease: "inout" as const }, [8, 0]] as const,
    scale: 1.2,
  };

  it("keeps a constant on both halves", () => {
    const { head, tail } = splitAnimate(animate as never, 5, 8);
    expect(head.scale).toBe(1.2);
    expect(tail.scale).toBe(1.2);
  });

  it("cuts exactly: each half plays what the shot played over its span", () => {
    const { head, tail } = splitAnimate(animate as never, 5, 8);
    for (const t of [0, 1, 2.5, 4, 4.99])
      expect(xAt(head, t)).toBeCloseTo(xAt(animate as never, t), 9);
    for (const t of [0, 0.5, 1.5, 3])
      expect(xAt(tail, t)).toBeCloseTo(xAt(animate as never, t + 5), 9);
  });

  it("keeps only the keys that still shape each half", () => {
    const { head, tail } = splitAnimate(animate as never, 5, 8);
    // The head needs the key at 8 to know where 4 → 8 was heading; the tail needs the one at 4.
    expect(head.x).toEqual([[0, 0], { t: 4, v: 400, ease: "inout" }, [8, 0]]);
    expect(tail.x).toEqual([{ t: -1, v: 400, ease: "inout" }, [3, 0]]);
  });
});

describe("AnimateSchema", () => {
  it("names a property it does not know rather than ignoring it", () => {
    const r = AnimateSchema.safeParse({ opactiy: 1 });
    expect(r.success).toBe(false);
  });

  it("takes pairs, maps and constants", () => {
    expect(
      AnimateSchema.safeParse({
        x: [[0, 0], { t: 1, v: 5, ease: "out" }],
        opacity: 0.5,
      }).success,
    ).toBe(true);
  });
});
