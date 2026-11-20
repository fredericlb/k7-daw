// A module with more than one input, and the chains syntax that feeds the extra ones.
//
// A chain carries ONE signal, so a second input has to arrive from somewhere else: a destination
// names the port it lands on (`crossfade#b`) and the derivation records it on the step as `in`,
// where validate() checks it against the module's port list. See docs/ports.md.

import { cpSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { portRef, splitPort } from "@studio/song-schema/moduleTypes";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from "vitest";
import { deriveChains } from "./chainsGraph";
import { loadGraph } from "./graph";
import { loadModuleRefs, loadModules } from "./loader";
import { writeDoc, writeLane, writeNode } from "./testSupport";

let dir: string;

/** A two-in crossfade, declared by the MODULE rather than by its type folder. */
const CROSSFADE = {
  plugin: { type: "jsdsp" },
  inputs: [
    { name: "a", type: "audio" },
    { name: "b", type: "audio", mandatory: false },
  ],
  outputs: [{ name: "out", type: "audio" }],
  params: { mix: 0.5 },
};

const SONG_PROJECT = { project: { name: "song", tempo: 120 } };

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "td-ports-"));
  writeDoc(join(dir, "project.k7x"), SONG_PROJECT);
});
afterEach(() => rmSync(dir, { recursive: true, force: true }));

describe("a port ref", () => {
  it("splits on the LAST separator, leaving every other one alone", () => {
    // `#`, because `:` and `/` are spoken for by the namespace and the node.
    expect(splitPort("crossfade#b")).toEqual({ node: "crossfade", port: "b" });
    expect(splitPort("@k7-core/audio/crossfade:live#b")).toEqual({
      node: "@k7-core/audio/crossfade:live",
      port: "b",
    });
    expect(splitPort("crossfade")).toEqual({ node: "crossfade" });
  });

  it("treats a half-written ref as a plain name rather than guessing", () => {
    // `x#` is a typo with no reading: an empty port would be one nothing downstream could match.
    expect(splitPort("x#")).toEqual({ node: "x#" });
    expect(portRef("x")).toBe("x");
    expect(portRef("x", "b")).toBe("x#b");
  });

  it("is refused by the schema when either half is empty", () => {
    expect(() => deriveChains({ modules: { a: ["#b"] } })).toThrow(/port ref/);
  });
});

describe("a module's own ports", () => {
  it("override the type folder's defaults", () => {
    writeNode(dir, "crossfade", CROSSFADE);
    const m = loadModules(dir).crossfade;
    expect(m.inputs).toEqual([
      { name: "a", type: "audio" },
      { name: "b", type: "audio", mandatory: false },
    ]);
    expect(m.outputs).toEqual([{ name: "out", type: "audio" }]);
  });

  it("leave a module that declares none with its type's", () => {
    // The overwhelming case — a module declaring no ports at all — which must not move. A type
    // folder's defaults are read at `@workspace`/`@k7-core` now, so this one lives there.
    const ws = mkdtempSync(join(tmpdir(), "td-ports-type-"));
    try {
      const song = join(ws, "song");
      writeDoc(join(ws, "_modules", "audio", "amp", "module.k7x"), {
        plugin: { type: "internal", name: "Chorus" },
      });
      writeDoc(join(song, "project.k7x"), SONG_PROJECT);
      writeNode(song, "amp", { extends: "@workspace/audio/amp" });
      const m = loadModuleRefs(song)["@workspace/audio/amp"];
      expect(m.inputs).toEqual([{ name: "in", type: "audio" }]);
      expect(m.outputs).toEqual([{ name: "out", type: "audio" }]);
    } finally {
      rmSync(ws, { recursive: true, force: true });
    }
  });
});

describe("wiring a named port", () => {
  it("records the feeder on the step, not as the flow", () => {
    const derived = deriveChains(
      {
        modules: {
          deckA: ["crossfade"],
          deckB: ["crossfade#b"],
          crossfade: ["OUT"],
        },
      },
      (n) => n === "deckA" || n === "deckB",
    );
    // deckA FLOWS in, so the crossfade continues its run; a side input must not change that.
    expect(derived.deckA.use).toEqual([{ module: "crossfade", in: ["deckB"] }]);
    expect(derived.deckA.out).toEqual([{ to: "OUT", gain: 1 }]);
    // …and the edge keeps its port, so a renderer can draw the cable to the right jack.
    expect(derived.deckB.out).toEqual([{ to: "crossfade#b", gain: 1 }]);
    // There is no chain NAMED crossfade: it is a step inside deckA's, not a meeting point.
    expect(derived.crossfade).toBeUndefined();
  });

  it("does not let a side input absorb the module into that run", () => {
    // The trap: the run would otherwise swallow the target as a STEP and miss its real flow input.
    const derived = deriveChains(
      { modules: { deckB: ["crossfade#b"], crossfade: ["OUT"] } },
      (n) => n === "deckB",
    );
    expect(derived.deckB.use).toEqual([]); // deckB's run ends at the port
    expect(derived.crossfade.use[0].module).toBe("crossfade");
  });

  it("unions the destinations of a module keyed with a port on the source side", () => {
    // Keying on the raw string would split `deck#cue` and `deck` into two nodes that never met.
    const derived = deriveChains({
      modules: { deck: ["OUT"], "deck#cue": ["phones"], phones: [] },
    });
    expect(derived.deck.out.map((o) => o.to).sort()).toEqual(["OUT", "phones"]);
    expect(Object.keys(derived).sort()).toEqual(["deck", "phones"]);
    // …but the union must not cost the cable's origin: the source jack has to survive it.
    expect(derived.deck.out.find((o) => o.to === "phones")?.fromPort).toBe(
      "cue",
    );
    expect(
      derived.deck.out.find((o) => o.to === "OUT")?.fromPort,
    ).toBeUndefined();
  });

  it("ends the run at a module that splits across its own outputs", () => {
    // Counting out-degree per KEY would call each of a separator's four edges an only child.
    const derived = deriveChains({
      modules: {
        "sep#drums": ["gate"],
        "sep#vocals": ["reverb"],
        gate: ["OUT"],
        reverb: ["OUT"],
      },
    });
    expect(derived.sep.use.map((s) => s.module)).toEqual(["sep"]);
    expect(derived.sep.out.map((o) => [o.fromPort, o.to]).sort()).toEqual([
      ["drums", "gate"],
      ["vocals", "reverb"],
    ]);
    // Each target starts a chain of its own rather than being swallowed by sep's run.
    expect(Object.keys(derived).sort()).toEqual(["gate", "reverb", "sep"]);
  });
});

describe("what validate says", () => {
  const lane = (name: string) => writeLane(dir, name, "track", { events: [] });
  const wire = (modules: Record<string, string[]>) => {
    writeNode(dir, "crossfade", CROSSFADE);
    lane("a");
    lane("b");
    writeDoc(join(dir, "graph.k7x"), { modules });
    return loadGraph(dir);
  };
  const BOTH = { a: ["crossfade"], b: ["crossfade#b"], crossfade: ["OUT"] };

  it("accepts both inputs fed, and says nothing at all", () => {
    // EVERY warning: filtering to /port|input/ hides two the raw-string walk produces.
    const g = wire(BOTH);
    expect(g.warnings).toEqual([]);
  });

  it("follows a side input through to the sink, and back for a cycle", () => {
    // A port lands INSIDE another run, so `reachesSink` and `hasCycle` both have to follow it there.
    const g = wire(BOTH);
    expect(
      g.warnings.filter((m) => /dead end|reaches OUT|cycle/.test(m)),
    ).toEqual([]);
  });

  it("refuses more sources than the module has ports", () => {
    // Arity THROWS rather than warns: the compiler cannot lay out a port that is not there.
    lane("c");
    expect(() => wire({ ...BOTH, c: ["crossfade#b"] })).toThrow(/only 2 port/);
  });
});

describe("the crossfade-demo song", () => {
  // The shipped demo, copied with its workspace's modules: a compile rebuilds a stale jsdsp.json, and never in the checkout.
  const DEMOS = join(
    dirname(fileURLToPath(import.meta.url)),
    "..",
    "..",
    "..",
    "..",
    "..",
    "demos",
  );
  let ws: string;
  let SONG: string;
  beforeAll(() => {
    ws = mkdtempSync(join(tmpdir(), "td-xfade-demo-"));
    SONG = join(ws, "crossfade-demo.k7");
    cpSync(join(DEMOS, "crossfade-demo.k7"), SONG, { recursive: true });
    cpSync(join(DEMOS, "_modules"), join(ws, "_modules"), { recursive: true });
  });
  afterAll(() => rmSync(ws, { recursive: true, force: true }));

  it("opens clean", () => {
    expect(loadGraph(SONG).warnings).toEqual([]);
  });
});
