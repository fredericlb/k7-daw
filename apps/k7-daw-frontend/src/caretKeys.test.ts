import { describe, expect, it } from "vitest";
import { caretKeyDelta } from "./caretKeys";

describe("caretKeyDelta", () => {
  it("reads hjkl and the arrows as the same steps", () => {
    for (const [a, b] of [
      ["h", "ArrowLeft"],
      ["l", "ArrowRight"],
      ["k", "ArrowUp"],
      ["j", "ArrowDown"],
    ])
      expect(caretKeyDelta({ key: a })).toEqual(caretKeyDelta({ key: b }));
  });

  it("steps one cell, columns for h/l and rows for k/j", () => {
    expect(caretKeyDelta({ key: "h" })).toEqual({ dRow: 0, dCol: -1 });
    expect(caretKeyDelta({ key: "j" })).toEqual({ dRow: 1, dCol: 0 });
  });

  it("answers nothing for any other key", () => {
    expect(caretKeyDelta({ key: "Escape" })).toBeNull();
    expect(caretKeyDelta({ key: "x" })).toBeNull();
  });
});
