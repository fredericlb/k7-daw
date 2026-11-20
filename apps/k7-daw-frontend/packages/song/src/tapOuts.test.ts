// tapOuts.test.ts — a rack node's tap routed to a subchain (docs/racks.md): the instrument carries the tap's note
// on a pair of its own, the subchain's run becomes a track fed by that pair, and a route to nowhere is refused.

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { readChainProject, readLayout, toSegments } from "./chainProject";
import { tapPairs } from "./tapOuts";
import { writeDoc } from "./testSupport";

let root: string;
let dir: string;

const RACK = {
  id: "drums",
  type: "rack",
  rack: {
    symbols: {
      Kick: { note: 36, clip: "kick.wav" },
      Snare: { note: 38, clip: "snare.wav" },
      Hat: { note: 42, clip: "hat.wav" },
    },
  },
};

/** The kit's chain file: its instrument `node` with `extra` on it, then one subchain `sub`. */
const putKit = (
  extra: Record<string, unknown>,
  sub = "snare-fx",
  node: Record<string, unknown> = RACK,
): void =>
  writeDoc(join(dir, "chains", "kit.k7x"), {
    start: true,
    steps: [
      { id: "lane-kit", track: "kit" },
      { ...node, ...extra },
      { to: [{ chain: "MIX" }] },
    ],
    subchains: [
      {
        id: sub,
        steps: [
          { id: "snare-verb", plugin: { type: "internal", name: "Chorus" } },
          { to: [{ chain: "MIX" }] },
        ],
      },
    ],
  });

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "tap-outs-"));
  dir = join(root, "song");
  writeDoc(join(dir, "project.k7x"), { project: { name: "t", tempo: 120 } });
  writeDoc(join(dir, "tracks", "kit.k7x"), {
    extends: "@meta/track",
    events: [],
  });
  for (const f of ["kick.wav", "snare.wav", "hat.wav"])
    writeFileSync(join(dir, f), "");
});
afterEach(() => rmSync(root, { recursive: true, force: true }));

describe("a tap routed to a subchain", () => {
  it("is a feed of that subchain's first node, which makes the subchain part of the song", () => {
    putKit({ tap_outs: { Snare: "snare-fx" } });
    const g = toSegments(readChainProject(dir), readLayout(dir)).graph;
    expect(g.tap_feeds).toEqual([
      { node: "drums", tap: "Snare", to: "snare-verb" },
    ]);
    expect(g.modules?.["snare-verb"]).toEqual(["MIX"]);
  });

  it("numbers the pairs by tap name, so a rebuild lands every tap where it was", () => {
    const pairs = tapPairs([
      { node: "drums", tap: "Snare", to: "a" },
      { node: "drums", tap: "Hat", to: "b" },
    ]).get("drums");
    expect(pairs?.map((t) => [t.tap, t.pair])).toEqual([
      ["Hat", 1],
      ["Snare", 2],
    ]);
  });
});
