// A binding's depth: what a knob draws as the stretch its curve sweeps.

import { describe, expect, it } from "vitest";
import { decodeDepth, depthSpan, encodeDepth } from "./automationDepth";

describe("automation depth", () => {
  it("sweeps 0..amplitude unipolar, and ±amplitude/2 around the set value bipolar, inside the travel", () => {
    expect(depthSpan({ amplitude: 0.6, bipolar: false }, 0.9)).toEqual([
      0, 0.6,
    ]);
    const [lo, hi] = depthSpan({ amplitude: 0.4, bipolar: true }, 0.5);
    expect(lo).toBeCloseTo(0.3);
    expect(hi).toBeCloseTo(0.7);
    const clipped = depthSpan({ amplitude: 1, bipolar: true }, 0.8);
    expect(clipped[0]).toBeCloseTo(0.3);
    expect(clipped[1]).toBe(1);
  });

  it("keeps one side of the set value when the curve is cut", () => {
    const d = { amplitude: 0.4, bipolar: true } as const;
    const up = depthSpan({ ...d, cut: "positive" }, 0.5);
    expect(up[0]).toBe(0.5);
    expect(up[1]).toBeCloseTo(0.7);
    const down = depthSpan({ ...d, cut: "negative" }, 0.5);
    expect(down[0]).toBeCloseTo(0.3);
    expect(down[1]).toBe(0.5);
    expect(
      depthSpan({ amplitude: 1, bipolar: false, cut: "negative" }, 0.5),
    ).toEqual([0, 0]);
  });

  it("survives the string a memoised card is handed", () => {
    const d = { amplitude: 0.25, bipolar: true };
    expect(decodeDepth(encodeDepth(d))).toEqual(d);
    const cut = { ...d, cut: "negative" as const };
    expect(decodeDepth(encodeDepth(cut))).toEqual(cut);
    expect(decodeDepth("")).toBeUndefined();
  });
});
