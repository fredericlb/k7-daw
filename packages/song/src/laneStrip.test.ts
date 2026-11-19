// A lane's strip. Two things would be wrong silently: a mixer spliced anywhere but LAST (the level
// would sit before what makes the sound, and `trackMixer` reads the last one), and a run that ends
// somewhere other than the sum being rewritten on a guess.

import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadGraph } from "./graph";
import {
  addLaneStrip,
  hasLaneStrip,
  mixerNameFor,
  planLaneStrip,
} from "./laneStrip";
import { trackMixer } from "./project";
import { SHARED_STRIP } from "./scaffoldLive";
import {
  librariesFixture,
  writeDoc,
  writeLane,
  writeNode,
} from "./testSupport";
import { wsData } from "./yaml/workspaceDoc";

let dir: string;

const graph = (modules: Record<string, string[]>, header = ""): void =>
  writeFileSync(
    join(dir, "graph.k7x"),
    `${header}modules(${JSON.stringify(modules)})\n`,
    "utf8",
  );
const lane = (id: string): void => {
  writeLane(dir, id, "track", { events: [] });
};
const module_ = (name: string, plugin: string): void => {
  writeNode(dir, name, { plugin: { type: "internal", name: plugin } });
};
const graphText = () => readFileSync(join(dir, "graph.k7x"), "utf8");
const graphModules = () => wsData(join(dir, "graph.k7x")).modules;

let ws: string;
beforeEach(() => {
  // A project in a WORKSPACE, because a strip is a use of the LIBRARY's mixer and that is where the library is mounted.
  ws = mkdtempSync(join(tmpdir(), "td-strip-"));
  dir = join(ws, "song.k7");
  writeDoc(join(dir, "project.k7x"), { project: { name: "t", tempo: 120 } });
  const mixer = join(
    librariesFixture(ws),
    "modules",
    "audio",
    "utility",
    "mixer",
  );
  // The plugin inline in the FIXTURE's library, so nothing here waits on an engine scan.
  writeDoc(join(mixer, "module.k7x"), {
    plugin: { type: "internal", name: "Mixer" },
  });
});
afterEach(() => rmSync(ws, { recursive: true, force: true }));

describe("mixerNameFor", () => {
  it("names the lane, and steps aside for a name already taken", () => {
    expect(mixerNameFor("pad", () => false)).toBe("mix-pad");
    expect(mixerNameFor("pad", (n) => n === "mix-pad")).toBe("mix-pad-2");
  });
});

describe("planLaneStrip", () => {
  it("wires an ORPHAN lane straight to the sum, since it is in no chain at all", () => {
    lane("kit");
    graph({ MIX: ["OUT"] });
    const plan = planLaneStrip(dir, "kit", loadGraph(dir));
    expect(plan).toMatchObject({ name: "mix-kit", after: null });
  });

  it("splices after the LAST module of a run, so the level sits past the sound", () => {
    lane("pad");
    module_("synth-pad", "4osc");
    graph({ pad: ["synth-pad"], "synth-pad": ["MIX"], MIX: ["OUT"] });
    expect(planLaneStrip(dir, "pad", loadGraph(dir))).toMatchObject({
      name: "mix-pad",
      after: "synth-pad",
    });
  });

  it("answers null for a lane that already ends on one — the migration is idempotent", () => {
    lane("pad");
    module_("mix-pad", "Mixer");
    graph({ pad: ["mix-pad"], "mix-pad": ["MIX"], MIX: ["OUT"] });
    const g = loadGraph(dir);
    expect(hasLaneStrip(g, "pad")).toBe(true);
    expect(planLaneStrip(dir, "pad", g)).toBeNull();
  });

  it("refuses a run that ends somewhere other than the sum rather than guessing", () => {
    lane("pad");
    lane("kit");
    module_("verb", "Reverb");
    module_("bus", "Mixer");
    // A real BUS: two lanes MEET at it, so neither run reaches the sum itself, and a splice would rewrite the arrangement.
    graph({
      pad: ["verb"],
      kit: ["verb"],
      verb: ["bus"],
      bus: ["MIX"],
      MIX: ["OUT"],
    });
    expect(() => planLaneStrip(dir, "pad", loadGraph(dir))).toThrow(
      /does not run into MIX/,
    );
  });
});

describe("addLaneStrip", () => {
  it("is a no-op the second time", () => {
    lane("kit");
    graph({ MIX: ["OUT"] });
    addLaneStrip(dir, "kit", loadGraph(dir));
    const after = graphText();
    expect(addLaneStrip(dir, "kit", loadGraph(dir))).toBeNull();
    expect(graphText()).toBe(after);
  });
});
