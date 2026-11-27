// The colours a track's category says it with: one hue per category, shades within one, so two guitars
// are both green and still two. The category itself is the Rust crate's (song's arrange_tests.rs).

import { describe, expect, it } from "vitest";
import {
  arrangeByCategory,
  autoColors,
  categoryShade,
  type TrackCategory,
} from "./trackColor";

describe("the colours", () => {
  it("gives one category one hue, and two of it two shades", () => {
    const c = autoColors([
      { id: "a", category: "guitar" },
      { id: "b", category: "vox" },
      { id: "c", category: "guitar" },
    ]);
    expect(c.get("a")).not.toBe(c.get("c"));
    expect(c.get("a")).toMatch(/^#[0-9a-f]{6}$/);
    // Green: the green channel leads.
    const g = c.get("a") ?? "";
    expect(Number.parseInt(g.slice(3, 5), 16)).toBeGreaterThan(
      Number.parseInt(g.slice(1, 3), 16),
    );
  });
  it("draws a single track of a category at its middle shade", () => {
    expect(categoryShade("drums", 0, 1)).toBe(categoryShade("drums", 0, 1));
    expect(categoryShade("drums", 0, 3)).not.toBe(categoryShade("drums", 2, 3));
  });
});

describe("the order", () => {
  it("arranges vox, drums, basses, guitars, keys, synths, then the rest — stable within one", () => {
    const category: Record<string, TrackCategory> = {
      "sy.a": "synth",
      "track-1": "other",
      "ge.b": "guitar",
      basse: "bass",
      "vo.c": "vox",
      "da.d": "drums",
      "pi.e": "keys",
      "ge.f": "guitar",
    };
    const names = Object.keys(category);
    expect(arrangeByCategory(names, (n) => category[n])).toEqual([
      "vo.c",
      "da.d",
      "basse",
      "ge.b",
      "ge.f",
      "pi.e",
      "sy.a",
      "track-1",
    ]);
  });
});
