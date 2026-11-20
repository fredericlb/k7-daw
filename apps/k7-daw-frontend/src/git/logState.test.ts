import { describe, expect, it } from "vitest";
import type { CommitInfo } from "./gitTypes";
import { cursor, emptyLog, logAfter, PAGE } from "./logState";

const commit = (id: string): CommitInfo => ({
  id,
  subject: id,
  author: "a",
  time: 1,
  device: null,
  files_changed: 1,
});
const page = (from: number, n: number) =>
  Array.from({ length: n }, (_, i) => commit(`c${from + i}`));

describe("the commits list", () => {
  it("is at its end when a page comes back short", () => {
    expect(
      logAfter(emptyLog, { type: "first", commits: page(0, PAGE) }).end,
    ).toBe(false);
    expect(
      logAfter(emptyLog, { type: "first", commits: page(0, PAGE - 1) }).end,
    ).toBe(true);
  });

  it("appends the next page after the oldest commit read", () => {
    const one = logAfter(emptyLog, { type: "first", commits: page(0, PAGE) });
    expect(cursor(one)).toBe(`c${PAGE - 1}`);
    const two = logAfter(one, { type: "more", commits: page(PAGE, 3) });
    expect(two.commits).toHaveLength(PAGE + 3);
    expect(two.end).toBe(true);
  });

  it("adds a commit once when a page overlaps", () => {
    const one = logAfter(emptyLog, { type: "first", commits: page(0, PAGE) });
    const two = logAfter(one, { type: "more", commits: page(PAGE - 2, 5) });
    expect(
      two.commits.map((c) => c.id).filter((i) => i === `c${PAGE - 1}`),
    ).toHaveLength(1);
  });

  it("restarts at the head on a refresh", () => {
    const one = logAfter(emptyLog, { type: "first", commits: page(0, PAGE) });
    const more = logAfter(one, { type: "more", commits: page(PAGE, PAGE) });
    expect(
      logAfter(more, { type: "first", commits: page(100, 2) }).commits,
    ).toHaveLength(2);
  });
});
