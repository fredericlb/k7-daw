// A LANE: a node of one of the three built-in track modules. What is pinned here is that a lane
// is NOT typed — it offers both outputs and each event leaves by the one its material fits — plus
// the one axis that survives (written down or streamed) and the group that is neither. See
// docs/trackModules.md.

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  MODULE_TYPES,
  TRACK_CATEGORY,
  TRACK_REFS,
} from "@studio/song-schema/moduleTypes";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadGraph } from "./graph";
import { loadLanes, loadModuleRefs } from "./loader";
import { writeDoc, writeModule, writeNode } from "./testSupport";
import { isTrack, trackModules } from "./trackModule";

let dir: string;

type Lane = [string, string, Record<string, unknown>?];

/** A project whose lanes are `lanes`, each `[name, extends, extra]`, all wired straight to OUT unless `graph` says otherwise. */
function project(lanes: Lane[], graph?: Record<string, unknown>): string {
  const p = join(dir, "song.k7");
  writeDoc(join(p, "project.k7x"), { project: { name: "song" } });
  writeModule(p, "io", "OUT", { io: "out" });
  for (const [name, ext, extra] of lanes)
    writeNode(p, name, { extends: ext, ...extra });
  writeDoc(
    join(p, "graph.k7x"),
    graph ?? {
      modules: Object.fromEntries(lanes.map(([n]) => [n, ["OUT"]])),
    },
  );
  return p;
}

/** The clip `riff`, two bars of an empty wav. */
const riff = (p: string): void => {
  writeDoc(join(p, "clips", "riff.k7x"), {
    source: "clips/riff.wav",
    to: "2:1",
  });
  writeFileSync(join(p, "clips", "riff.wav"), "");
};
/** The sequence `line`, one note. */
const line = (p: string): void =>
  writeDoc(join(p, "sequences", "line.k7x"), {
    notes: [{ t: 0, d: 1, k: 60 }],
  });

const BOTH = {
  events: [
    { ref: "riff", start: "1:1:1" },
    { ref: "line", start: "3:1:1" },
  ],
};

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "td-lane-"));
});
afterEach(() => rmSync(dir, { recursive: true, force: true }));

describe("the three built-in track modules", () => {
  it("resolve with no file anywhere", () => {
    // A lane must cost ONE file, its own node, so there is nothing an old project can be missing.
    const refs = loadModuleRefs(project([["gtr", "@meta/track"]]));
    for (const ref of TRACK_REFS) {
      expect(refs[ref], ref).toBeDefined();
      expect(isTrack(refs[ref]), ref).toBe(true);
    }
  });

  it("offer BOTH outputs — a lane is not typed by what it carries", () => {
    // Detyping in one assertion: drop either port and a lane holds one kind of material again.
    const m = trackModules();
    for (const ref of ["@meta/track", "@meta/strudel-track"])
      expect(m[ref].outputs, ref).toEqual([
        { name: "audio", type: "audio" },
        { name: "midi", type: "midi" },
      ]);

    // The one axis left: where the events come from.
    expect(m["@meta/track"].generated).toBeUndefined();
    expect(m["@meta/strudel-track"].generated).toBe(true);

    // And the one that is neither: a group carries no signal, so it offers no output.
    expect(m["@meta/group-track"].outputs).toEqual([]);
  });

  it("take NO input — what arrives on a lane is its clips", () => {
    // A live input LANDS on a lane rather than feeding a port, which the engine cannot express.
    for (const m of Object.values(trackModules())) expect(m.inputs).toEqual([]);
    expect(MODULE_TYPES[TRACK_CATEGORY].kind).toBe("track");
  });
});

describe("a lane, read off what it extends", () => {
  it("says where its clips come from — there is no `type` either", () => {
    const p = project([
      ["gtr", "@meta/track"],
      ["kit", "@meta/strudel-track", { strudel: "algo/p.strudel" }],
      ["live", "@meta/group-track", { tracks: ["gtr", "kit"] }],
    ]);
    const lanes = loadLanes(p);
    expect(lanes.gtr.type).toBe("single");
    expect(lanes.kit.type).toBe("strudel");
    expect(lanes.live.type).toBe("group");
    expect(lanes.live.tracks).toEqual(["gtr", "kit"]);
  });

  it("is a chain's SOURCE, not a step in it", () => {
    // `from` says the lane STARTS the run; read wrong, it compiles as a plugin on another track.
    const g = loadGraph(project([["gtr", "@meta/track"]]));
    expect(g.chains.gtr.from).toBe("gtr");
    expect(g.chains.gtr.use).toEqual([]);
    expect(g.warnings).toEqual([]);
  });
});

describe("what a lane may be called", () => {
  it("takes a name a module already has — a lane and its instrument are not one thing", () => {
    // What every import writes: the source DAW named the track after the instrument on it.
    const p = project([["amp", "@meta/track"]]);
    writeModule(p, "audio", "amp", {
      plugin: { type: "internal", name: "Chorus" },
    });
    const lanes = loadLanes(p, true);
    // The bare name is the LANE's: it is the one a graph key names, and the module keeps its ref.
    expect(Object.keys(lanes)).toContain("amp");
  });
});

describe("a mixed lane wired bare", () => {
  it("warns when everything it holds goes through one typed module", () => {
    // A typed first module is handed the other kind too, which is not a crash and not meant.
    const p = project([["gtr", "@meta/track", BOTH]], {
      modules: { gtr: ["synth"], synth: ["OUT"] },
    });
    // A real type folder, so its default ports say it takes midi — a bare node has none of its own.
    writeDoc(join(dir, "_modules", "instruments", "synth", "module.k7x"), {
      plugin: { type: "internal", name: "4osc" },
    });
    writeNode(p, "synth", { extends: "@workspace/instruments/synth" });
    riff(p);
    line(p);
    expect(
      loadGraph(p).warnings.some(
        (x) =>
          x.includes("holds audio AND sequences") && x.includes("gtr#midi"),
      ),
    ).toBe(true);
  });
});
