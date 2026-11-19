import { cpSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { GRAPH_FILE } from "@studio/song-schema/yaml/graphSchema";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadGraph } from "./graph";
import {
  insertChainStepAfter,
  replaceChainStep,
  setDoc,
  writeDoc,
} from "./testSupport";

// A step may name an INHERITED module by its ref, with no local module standing in for it.

const here = dirname(fileURLToPath(import.meta.url)); // packages/song/src
const DEMOS = join(here, "..", "..", "..", "demos");
const SONG = join(DEMOS, "synth-demo.k7");
const REF = "@workspace/audio/shared-delay";

describe("a chain step naming an inherited module by ref", () => {
  let ws: string;
  let dir: string;
  beforeEach(() => {
    ws = mkdtempSync(join(tmpdir(), "td-nsstep-"));
    dir = join(ws, "song");
    cpSync(SONG, dir, { recursive: true });
    // The Examples workspace's own shared library, beside every demo project.
    cpSync(join(DEMOS, "_modules"), join(ws, "_modules"), { recursive: true });
    // The workspace's shared catalogue: a namespace, and one module in it.
    writeDoc(join(ws, "_modules", "audio", "shared-delay", "module.k7x"), {
      plugin: { type: "internal", name: "delay" },
    });
    // …and a shared SINK, for the other position a module name is written in.
    writeDoc(join(ws, "_modules", "io", "desk", "module.k7x"), { io: "out" });
    insertChainStepAfter(dir, "lead", { module: REF });
  });
  afterEach(() => rmSync(ws, { recursive: true, force: true }));

  it("resolves, rather than being reported as a dangling name", () => {
    const g = loadGraph(dir);
    expect(g.warnings.filter((w) => w.includes(REF))).toEqual([]);
    expect(g.modules[REF]).toMatchObject({ plugin: { name: "delay" } });
  });

  // The sink is a different lookup, so a ref reaching it needs its own proof.
  it("works in the sink position too, not only as a step", () => {
    replaceChainStep(dir, "MIX", { module: "@workspace/io/desk" });
    const g = loadGraph(dir);
    expect(g.warnings.filter((w) => w.includes("desk"))).toEqual([]);
    expect(Object.values(g.chains)[0].out[0].to).toBe("@workspace/io/desk");
  });
});

// A node's ref takes `:node`, but on disk it reads like a folder: "is not a module" is useless there.
describe("a ref that spells a node with a slash", () => {
  it("is told which character to change", () => {
    const ws = mkdtempSync(join(tmpdir(), "td-slash-"));
    const dir = join(ws, "song");
    cpSync(SONG, dir, { recursive: true });
    writeDoc(join(ws, "_modules", "io", "IN", "module.k7x"), { io: "in" });
    writeDoc(join(ws, "_modules", "io", "IN", "in-2.k7x"), { channel: [2] });
    setDoc(
      join(dir, GRAPH_FILE),
      ["modules", "@workspace/io/IN/in-2"],
      ["melody"],
    );
    const w = loadGraph(dir).warnings;
    expect(
      w.some((x) => x.includes("did you mean '@workspace/io/IN:in-2'")),
    ).toBe(true);
    rmSync(ws, { recursive: true, force: true });
  });
});
