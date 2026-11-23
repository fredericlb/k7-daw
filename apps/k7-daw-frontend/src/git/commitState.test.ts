import { describe, expect, it } from "vitest";
import { commitView, formatSize, identityReady } from "./commitState";
import type { StatusEntry } from "./gitTypes";

const row = (over: Partial<StatusEntry> = {}): StatusEntry => ({
  path: "a.k7x",
  change: "modified",
  size: 10,
  binary: false,
  lfs: false,
  ...over,
});

describe("what a save takes", () => {
  it("takes nothing when nothing changed", () => {
    expect(commitView([]).sent).toEqual([]);
  });

  it("leaves a too-large file out and names it", () => {
    const big = row({ path: "big.wav", size: 3e9, tooLarge: true });
    const v = commitView([row(), big]);
    expect(v.sent.map((e) => e.path)).toEqual(["a.k7x"]);
    expect(v.leftOut.map((e) => e.path)).toEqual(["big.wav"]);
  });

  it("has nothing to take when only too-large files remain", () => {
    const big = row({ path: "big.wav", size: 3e9, tooLarge: true });
    expect(commitView([big]).sent).toEqual([]);
  });

  it("counts the large files that go through LFS", () => {
    expect(
      commitView([row({ lfs: true }), row(), row({ lfs: true })]).lfs,
    ).toBe(2);
  });

  it("needs a name and an email to sign", () => {
    expect(identityReady(null)).toBe(false);
    expect(identityReady({ name: "Ana", email: " " })).toBe(false);
    expect(identityReady({ name: "Ana", email: "a@x.org" })).toBe(true);
  });
});

describe("a size", () => {
  it("reads in decimal units", () => {
    expect(formatSize(800)).toBe("800 B");
    expect(formatSize(1_500_000)).toBe("1.5 MB");
    expect(formatSize(250_000_000)).toBe("250 MB");
  });
});
