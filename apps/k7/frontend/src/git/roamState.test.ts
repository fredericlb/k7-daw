import { describe, expect, it } from "vitest";
import {
  cloneName,
  emptyRoam,
  offered,
  type RoamAnswer,
  type RoamEntry,
  roamAfter,
} from "./roamState";

const entry = (id: string, name: string, remote: string | null): RoamEntry => ({
  id,
  name,
  remote,
  addedAt: 1,
});

const answer = (entries: RoamEntry[], here: string[] = []): RoamAnswer => ({
  available: true,
  entries,
  here,
});

const load = (a: RoamAnswer) =>
  roamAfter(emptyRoam, { type: "loaded", answer: a });

describe("what the section offers", () => {
  it("lists a connected workspace this device lacks, by name", () => {
    const s = load(
      answer([
        entry("b", "Zed", "git@h:o/z.git"),
        entry("a", "Band", "git@h:o/b.git"),
      ]),
    );
    expect(offered(s).map((e) => e.name)).toEqual(["Band", "Zed"]);
  });

  it("never lists a workspace with no remote: only git carries a workspace", () => {
    const s = load(answer([entry("a", "Local", null)]));
    expect(offered(s)).toEqual([]);
  });

  it("never lists a workspace already on this device", () => {
    const s = load(answer([entry("a", "Band", "git@h:o/b.git")], ["a"]));
    expect(offered(s)).toEqual([]);
  });

  it("lists nothing where the host has no list", () => {
    const s = load({ available: false, entries: [], here: [] });
    expect(offered(s)).toEqual([]);
    expect(offered(emptyRoam)).toEqual([]);
  });

  it("drops an entry once its tombstone makes the route stop answering it", () => {
    const e = entry("a", "Band", "git@h:o/b.git");
    let s = load(answer([e]));
    expect(offered(s)).toHaveLength(1);
    s = roamAfter(s, { type: "loaded", answer: answer([]) });
    expect(offered(s)).toEqual([]);
  });
});

describe("the reducer", () => {
  it("hides a cloned entry at once, and forgets it when a read says it is here", () => {
    const e = entry("a", "Band", "git@h:o/b.git");
    let s = load(answer([e]));
    s = roamAfter(s, { type: "cloned", id: "a" });
    expect(offered(s)).toEqual([]);
    s = roamAfter(s, { type: "loaded", answer: answer([e]) });
    expect(offered(s)).toEqual([]);
    expect(s.cloned).toEqual(["a"]);
    s = roamAfter(s, { type: "loaded", answer: answer([e], ["a"]) });
    expect(s.cloned).toEqual([]);
  });

  it("keeps the last answer when a read fails", () => {
    const s = load(answer([entry("a", "Band", "git@h:o/b.git")]));
    expect(roamAfter(s, { type: "failed" })).toBe(s);
  });
});

describe("the name a clone starts with", () => {
  it("is the workspace's own, then the next free one", () => {
    expect(cloneName("Band", [])).toBe("Band");
    expect(cloneName("Band", ["band"])).toBe("Band 2");
    expect(cloneName("Band", ["Band", "Band 2"])).toBe("Band 3");
  });
});
