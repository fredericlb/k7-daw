// A placement id read back: the start, a repeat's `#n` aside (docs/laneEdits.md); the material is the Rust crate's.

import { describe, expect, it } from "vitest";
import { placementId, placementStartOf, withoutRepeat } from "./placementId";

describe("a placement id", () => {
  it("names the material and the start", () => {
    expect(placementId("riff", 16.5)).toBe("riff@16.5");
    expect(placementStartOf("riff@16.5")).toBe(16.5);
  });

  it("reads a repeat's id as its first placement's", () => {
    expect(withoutRepeat("riff@20#2")).toBe("riff@20");
    expect(placementStartOf("verse@8>kick@0#3")).toBe(0);
  });

  it("keeps a name that only looks like a repeat", () => {
    expect(withoutRepeat("#2")).toBe("#2");
    expect(placementStartOf("riff")).toBeNaN();
  });
});
