import { describe, expect, it } from "vitest";
import type { GitState } from "./git/gitTypes";
import { historyOffers, restoredKept, rollbackTo } from "./historySource";

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

describe("rollbackTo", () => {
  const rows = [
    { id: 1, ts: 10, undone: false },
    { id: 2, ts: 20, undone: false },
    { id: 3, ts: 30, undone: false },
    { id: 4, ts: 40, undone: true },
    { id: 5, ts: 50, undone: true },
  ];
  it("undoes every applied row after an applied one, and nothing for the newest applied", () => {
    expect(rollbackTo(rows, 1)).toEqual({ redo: false, steps: 2 });
    expect(rollbackTo(rows, 3)).toEqual({ redo: false, steps: 0 });
  });
  it("redoes an undone row and the undone ones before it", () => {
    expect(rollbackTo(rows, 4)).toEqual({ redo: true, steps: 1 });
    expect(rollbackTo(rows, 5)).toEqual({ redo: true, steps: 2 });
  });
  it("walks nothing for a row it does not know", () => {
    expect(rollbackTo(rows, 9)).toEqual({ redo: false, steps: 0 });
  });
});
