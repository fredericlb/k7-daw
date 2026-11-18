// A DSP module whose graph cannot play must be SAID, and must not play as the engine's default graph (an auto-wah),
// which is what a plugin with no `dspGraph` becomes. Node has no `dsp.k7x` decoder: it reads the `jsdsp.json` beside it.

import { cpSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { PASS_THROUGH_GRAPH } from "./compile/jsDspGraph";
import { jsDspBuildIssues } from "./diagnostics";
import { compileNodeProject } from "./nodeProject";
import { writeDoc } from "./testSupport";

const here = dirname(fileURLToPath(import.meta.url));
const DEMOS = join(here, "..", "..", "..", "demos");
const GRAPH = '{"params":[{"name":"Cutoff","min":0,"max":1}],"nodes":[]}\n';

let ws: string;
let dir: string;
let folder: string;

/** The demo's instrument as a DSP module of this type, with the files given beside it. */
function asModule(type: "dsp" | "jsdsp", files: Record<string, string>): void {
  writeDoc(join(folder, "module.k7x"), { plugin: { type, name: "Synth" } });
  for (const [name, text] of Object.entries(files))
    writeFileSync(join(folder, name), text, "utf8");
}

const lead = () =>
  compileNodeProject(dir)
    .tracks.flatMap((t) => t.plugins ?? [])
    .find((p) => p.id.endsWith(":lead"));

beforeEach(() => {
  ws = mkdtempSync(join(tmpdir(), "td-jsdsp-fail-"));
  dir = join(ws, "synth-demo.k7");
  cpSync(join(DEMOS, "synth-demo.k7"), dir, { recursive: true });
  cpSync(join(DEMOS, "_modules"), join(ws, "_modules"), { recursive: true });
  folder = join(ws, "_modules", "instruments", "synth");
});
afterEach(() => rmSync(ws, { recursive: true, force: true }));

describe("a dsp module", () => {
  it("plays the jsdsp.json beside its dsp.k7x where the host cannot decode", () => {
    asModule("dsp", { "dsp.k7x": "graph([])\n", "jsdsp.json": GRAPH });
    expect(lead()?.dspGraph).toBe(GRAPH);
    expect(jsDspBuildIssues(dir)).toEqual([]);
  });

  it("is reported on each node's file when its dsp.k7x is missing, and passes the sound through", () => {
    asModule("dsp", { "jsdsp.json": GRAPH });
    expect(lead()?.dspGraph).toBe(PASS_THROUGH_GRAPH);
    const found = jsDspBuildIssues(dir);
    expect(found.map((x) => x.file).sort()).toEqual([
      "nodes/lead.k7x",
      "nodes/synth-pad.k7x",
    ]);
    expect(found[0]).toMatchObject({ severity: "error" });
    expect(found[0]?.message).toContain("its dsp.k7x is missing");
    expect(found[0]?.message).toContain("passes the sound through");
  });

  it("says what is missing when the host cannot decode and no jsdsp.json is there", () => {
    asModule("dsp", { "dsp.k7x": "graph([])\n" });
    expect(lead()?.dspGraph).toBe(PASS_THROUGH_GRAPH);
    expect(jsDspBuildIssues(dir)[0]?.message).toContain(
      "cannot decode dsp.k7x",
    );
  });
});

describe("a legacy jsdsp module", () => {
  it("plays its committed jsdsp.json and is never rebuilt, whatever the source says", () => {
    asModule("jsdsp", {
      "jsdsp.ts": "export default (g) => { g.nope(); };\n",
      "jsdsp.json": GRAPH,
    });
    expect(lead()?.dspGraph).toBe(GRAPH);
    expect(jsDspBuildIssues(dir)).toEqual([]);
  });

  it("is reported when it has no jsdsp.json", () => {
    asModule("jsdsp", { "jsdsp.ts": "export default {};\n" });
    expect(lead()?.dspGraph).toBe(PASS_THROUGH_GRAPH);
    expect(jsDspBuildIssues(dir)[0]?.message).toContain(
      "its jsdsp.json is missing",
    );
  });
});
