// k7xIssues.test.ts — a finding's key path is placed on the k7x text's own line.

import { describe, expect, it } from "vitest";
import { locateK7x } from "./k7xIssues";

describe("a .k7x buffer", () => {
  it("locates a path to the deepest entry or item that exists", () => {
    const text = 'steps([\n  id(1),\n  id(2).to(["x"]),\n])\n';
    expect(locateK7x(text, ["steps", 1, "to"])).toEqual({ line: 3, col: 9 });
    expect(locateK7x(text, ["steps", 1, "nope"])).toEqual({ line: 3, col: 3 });
    expect(locateK7x(text, [])).toEqual({});
  });
});
