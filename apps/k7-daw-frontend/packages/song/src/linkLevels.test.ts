// A chain project's link levels: a send to a bus plays at its level, and a level no wire field
// carries is named in the graph's warnings rather than dropped. See docs/chainFiles.md.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadGraph } from "./graph";
import { writeDoc } from "./testSupport";

let dir: string;

function project(gtrEnd: Record<string, unknown>[]): void {
  writeDoc(join(dir, "project.k7x"), { project: { name: "t", tempo: 120 } });
  writeDoc(join(dir, "tracks", "gtr.k7x"), { extends: "@meta/track" });
  writeDoc(join(dir, "chains", "gtr.k7x"), {
    start: true,
    steps: [
      { id: "l1", track: "gtr" },
      { id: "a1", plugin: { type: "internal", name: "Chorus" } },
      { to: gtrEnd },
    ],
  });
  writeDoc(join(dir, "chains", "verb.k7x"), {
    steps: [
      { id: "v1", plugin: { type: "internal", name: "Compressor" } },
      { to: [{ chain: "MIX" }] },
    ],
  });
}

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "link-levels-"));
});
afterEach(() => rmSync(dir, { recursive: true, force: true }));

describe("a link's level", () => {
  it("is drawn on the graph's out, with no warning when every level is played", () => {
    project([
      { chain: "MIX", level: 1 },
      { chain: "verb", level: 0.3 },
    ]);
    const g = loadGraph(dir);
    expect(g.chains.gtr.out.find((o) => o.to === "v1")?.gain).toBe(0.3);
    expect(g.warnings.filter((w) => w.includes("level"))).toEqual([]);
  });
});
