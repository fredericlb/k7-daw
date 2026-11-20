import { cpSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { GRAPH_FILE } from "@studio/song-schema/yaml/graphSchema";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadGraph } from "./graph";
import { setDoc, writeDoc, writeNode } from "./testSupport";

// An input's track is named by the CHAIN, not by a field: a shared io/IN node cannot name one.

const here = dirname(fileURLToPath(import.meta.url)); // packages/song/src
const DEMOS = join(here, "..", "..", "..", "demos");
const SONG = join(DEMOS, "synth-demo.k7");
const IN = "@workspace/io/IN:in-1";

/** Set each module's destinations in `dir`'s graph, by key path, leaving the other modules as they are. */
const wireIn = (dir: string, modules: Record<string, string[]>): void => {
  for (const [from, to] of Object.entries(modules))
    setDoc(join(dir, GRAPH_FILE), ["modules", from], to);
};

/** The demo song in `ws/song`, its workspace's `_modules` with a generic IN and one input, and a lane `mic`. */
function liveSong(ws: string): string {
  const dir = join(ws, "song");
  cpSync(SONG, dir, { recursive: true });
  // The Examples workspace's own shared library, beside every demo project.
  cpSync(join(DEMOS, "_modules"), join(ws, "_modules"), { recursive: true });
  // The shared catalogue as `module populate` leaves it: a generic IN and one node per input.
  writeDoc(join(ws, "_modules", "io", "IN", "module.k7x"), { io: "in" });
  writeDoc(join(ws, "_modules", "io", "IN", "in-1.k7x"), { channel: [1] });
  // An audio lane of its own, called `mic`: the fixture already has a group called `live`.
  writeNode(dir, "mic", { extends: "@meta/track" });
  wireIn(dir, { mic: [IN], [IN]: ["OUT"] });
  return dir;
}

describe("a live input, wired by its chain", () => {
  let ws: string;
  let dir: string;
  const wire = (seg: { modules: Record<string, string[]> }): void =>
    wireIn(dir, seg.modules);
  beforeEach(() => {
    ws = mkdtempSync(join(tmpdir(), "td-livein-"));
    dir = liveSong(ws);
  });
  afterEach(() => rmSync(ws, { recursive: true, force: true }));

  // Signal comes FROM an input INTO a lane, and the file draws that direction.
  describe("an input that FEEDS a track", () => {
    beforeEach(() => wire({ modules: { mic: [], [IN]: ["mic"] } }));

    it("does not read the track as a dangling destination", () => {
      // A track is neither a chain nor a sink: it is the one destination that is a LANE.
      expect(
        loadGraph(dir).warnings.filter((w) =>
          w.includes("not a chain or io sink"),
        ),
      ).toEqual([]);
    });

    it("refuses the same arrow from a module that is not an input", () => {
      // A track's material is its clips, so an ordinary module aimed at one is discarded.
      wire({ modules: { [IN]: ["OUT"], lead: ["mic"] } });
      expect(
        loadGraph(dir).warnings.some((x) =>
          x.includes(
            "only a live input's or a MIDI device's signal lands on a lane",
          ),
        ),
      ).toBe(true);
    });
  });

  it("no longer refuses a lane holding sequences — every lane has an audio output", () => {
    // A detyped lane offers both outputs whatever it holds, so overdubbing needs no second lane.
    wire({ modules: { mic: [], melody: [IN], [IN]: ["lead"] } });
    expect(
      loadGraph(dir).warnings.some((x) =>
        x.includes("an interface input carries audio"),
      ),
    ).toBe(false);
  });

  // An input at the END of a run reads as somewhere to route TO, which is what it is not.
  it("is not mistaken for a sink when it ends the run", () => {
    wire({ modules: { [IN]: [] } }); // `mic` still feeds it, and it feeds nothing
    expect(
      loadGraph(dir).warnings.some(
        (x) => x.includes(IN) && x.includes("signal comes FROM it"),
      ),
    ).toBe(true);
  });
});
