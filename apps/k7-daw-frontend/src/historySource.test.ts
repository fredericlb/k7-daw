import { describe, expect, it } from "vitest";
import type { GitState } from "./git/gitTypes";
import { historyOffers, restoredKept } from "./historySource";

describe("historyOffers", () => {
  it("offers writes to a workspace no merge waits on", () => {
    expect(historyOffers(null).write).toBe(true);
    expect(historyOffers({ mode: "none" } as GitState).write).toBe(true);
    expect(historyOffers({ mode: "connected" } as GitState).write).toBe(true);
  });
  it("withholds writes while a merge waits", () => {
    expect(
      historyOffers({ mode: "connected", merge: 1 } as GitState).write,
    ).toBe(false);
  });
});

describe("restoredKept", () => {
  it("names the binary files a restore left, and none for texts alone", () => {
    expect(restoredKept({ entry: null, kept: ["take.wav"] })).toEqual([
      "take.wav",
    ]);
    expect(restoredKept({ entry: null })).toEqual([]);
  });
});
