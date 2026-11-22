// A placement id read back: the material and the start, a repeat's `#n` aside (docs/laneEdits.md).

import { describe, expect, it } from "vitest";
import {
  materialRefOf,
  placementId,
  placementStartOf,
  withoutRepeat,
} from "./placementId";

describe("a placement id", () => {
  it("names the material and the start", () => {
    expect(placementId("riff", 16.5)).toBe("riff@16.5");
    expect(materialRefOf("riff@16.5")).toBe("riff");
    expect(placementStartOf("riff@16.5")).toBe(16.5);
  });

  it("reads a repeat's id as its first placement's", () => {
    expect(withoutRepeat("riff@20#2")).toBe("riff@20");
    expect(materialRefOf("riff@20#2")).toBe("riff");
    expect(materialRefOf("verse@8>kick@0#3")).toBe("kick");
    expect(placementStartOf("verse@8>kick@0#3")).toBe(0);
  });

  it("keeps a name that only looks like a repeat", () => {
    expect(withoutRepeat("#2")).toBe("#2");
    expect(materialRefOf("a#2")).toBe("a#2");
    expect(materialRefOf("a@b#2")).toBe("a@b#2");
    expect(placementStartOf("riff")).toBeNaN();
  });
});
