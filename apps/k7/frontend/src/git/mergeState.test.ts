import { describe, expect, it } from "vitest";
import type { Conflict } from "./gitTypes";
import {
  byFile,
  canFinish,
  choicesAfter,
  emptyChoices,
  offered,
  remaining,
  resolveChoices,
} from "./mergeState";

const c = (id: string, over: Partial<Conflict> = {}): Conflict => ({
  id,
  path: "song.k7x",
  kind: "k7x",
  base: null,
  mine: "1",
  theirs: "2",
  both: false,
  choice: null,
  ...over,
});

const loaded = (cs: Conflict[]) =>
  choicesAfter(emptyChoices, { type: "load", conflicts: cs });

describe("what a conflict offers", () => {
  it("offers both only where the route says so", () => {
    expect(offered(c("a"))).toEqual(["mine", "theirs"]);
    expect(offered(c("a", { both: true }))).toEqual(["mine", "theirs", "both"]);
  });

  it("offers keep or remove for a removed file, never a side", () => {
    expect(offered(c("a", { kind: "presence" }))).toEqual(["keep", "remove"]);
  });

  it("refuses a choice the conflict does not offer", () => {
    const s = loaded([c("a"), c("p", { kind: "presence" })]);
    expect(choicesAfter(s, { type: "pick", id: "a", choice: "both" })).toBe(s);
    expect(choicesAfter(s, { type: "pick", id: "p", choice: "mine" })).toBe(s);
    expect(
      choicesAfter(s, { type: "pick", id: "p", choice: "keep" }).chosen.p,
    ).toBe("keep");
  });
});

describe("finishing the merge", () => {
  it("waits for every conflict", () => {
    let s = loaded([c("a"), c("b")]);
    expect(canFinish(s)).toBe(false);
    s = choicesAfter(s, { type: "pick", id: "a", choice: "mine" });
    expect(remaining(s)).toBe(1);
    expect(canFinish(s)).toBe(false);
    s = choicesAfter(s, { type: "pick", id: "b", choice: "theirs" });
    expect(canFinish(s)).toBe(true);
    expect(resolveChoices(s)).toEqual([
      { id: "a", choice: "mine" },
      { id: "b", choice: "theirs" },
    ]);
  });

  it("keeps what the device saved and what was chosen across a reload", () => {
    let s = loaded([c("a", { choice: "theirs" }), c("b")]);
    s = choicesAfter(s, { type: "pick", id: "b", choice: "mine" });
    s = choicesAfter(s, {
      type: "load",
      conflicts: [c("a", { choice: "theirs" }), c("b")],
    });
    expect(s.chosen).toEqual({ a: "theirs", b: "mine" });
  });

  it("drops a saved choice the conflict no longer offers", () => {
    expect(loaded([c("a", { choice: "both" })]).chosen).toEqual({});
  });

  it("leaves a binary and a removed file to the person when a side is picked for all", () => {
    const s = choicesAfter(
      loaded([
        c("a"),
        c("b", { kind: "binary" }),
        c("p", { kind: "presence" }),
      ]),
      { type: "pickAll", choice: "theirs" },
    );
    expect(s.chosen).toEqual({ a: "theirs" });
  });

  it("groups the cards by file", () => {
    const g = byFile([c("a"), c("b", { path: "x.txt" }), c("c")]);
    expect(g.map((x) => [x.path, x.items.length])).toEqual([
      ["song.k7x", 2],
      ["x.txt", 1],
    ]);
  });
});
