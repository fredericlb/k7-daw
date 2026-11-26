// The Camelot wheel: the pair everyone checks, the walk in fifths, and 24 distinct codes.

import { describe, expect, it } from "vitest";
import { camelot, type DetectedKey } from "./key";

describe("the Camelot wheel", () => {
  const key = (tonic: number, mode: "major" | "minor"): DetectedKey => ({
    tonic,
    mode,
    margin: 1,
  });

  it("puts C major at 8B and A minor at 8A — the pair everyone checks", () => {
    expect(camelot(key(0, "major"))).toBe("8B");
    expect(camelot(key(9, "minor"))).toBe("8A");
  });

  it("walks in FIFTHS, which is what makes ±1 mean anything", () => {
    // G is a fifth above C, so it must be the next hour along — not the next semitone.
    expect(camelot(key(7, "major"))).toBe("9B");
    expect(camelot(key(5, "major"))).toBe("7B");
  });

  it("gives all 24 keys a distinct code", () => {
    const all = new Set<string>();
    for (let t = 0; t < 12; t++)
      for (const m of ["major", "minor"] as const) all.add(camelot(key(t, m)));
    expect(all.size).toBe(24);
  });

  it("says nothing for nothing", () => {
    expect(camelot(null)).toBe("");
  });
});
