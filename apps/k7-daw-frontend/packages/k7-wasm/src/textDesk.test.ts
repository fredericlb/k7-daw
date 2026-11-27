// The cue sheet, the playlists, the library pins, an assembly template and a chain file: text in, the Rust writers' text out.
// What is pinned hardest is what a hand-edited file keeps: comments, order, one line per change.

import { describe, expect, it } from "vitest";
import { parse } from "./k7x/index.ts";
import { parsePlaylist } from "./schema.ts";
import {
  applySugared,
  editAssembly,
  editMix,
  editPlaylist,
  expandSugar,
  mixCues,
  parseLibraryPins,
  playlistStub,
  readAssemblyTemplate,
  readChain,
  readMixView,
  stringifyLibraryPins,
} from "./text.ts";

const A = "records/Daft Punk/Around the world.flac";
const B = "records/Alain Bashung/La nuit je mens.flac";

const PLAIN = `// where this set drops the needle
cues([
  "${A}",
  "${B}",
])
`;

/** A song's cue points written over a whole file's text, as the desk does. */
const write = (text: string, ref: string, markers: (number | null)[]) =>
  editMix(text, [{ op: "withMarkers", ref, markers }]).text;

describe("reading a cue sheet", () => {
  it("still reads one written before markers existed", () => {
    expect(mixCues(PLAIN)).toEqual({ markers: {}, issues: [] });
  });

  it("reads a ref that carries cues, beside one that does not", () => {
    const got = mixCues(
      `cues([\n  "${A}",\n  ref("${B}").markers([12.5, null, 48]),\n])\n`,
    );
    expect(got.issues).toEqual([]);
    expect(got.markers).toEqual({ [B]: [12.5, null, 48] });
  });

  it("is empty, not broken, for a sheet with nothing cued or no file at all", () => {
    expect(mixCues("cues([])\n").issues).toEqual([]);
    expect(mixCues("").issues).toEqual([]);
  });

  it("refuses more cues than buttons, a cue before the start, a ref the set list cannot name, an unknown key", () => {
    for (const text of [
      `cues([ref("${A}").markers([1, 2, 3, 4, 5, 6, 7, 8, 9])])\n`,
      `cues([ref("${A}").markers([-3])])\n`,
      'cues([\n  "@k7-core/songs/a/b/c",\n])\n',
      "cues([])\nsongs([])\n",
      "cues([\n",
    ]) {
      const got = mixCues(text);
      expect(got.issues.length).toBeGreaterThan(0);
      expect(got.markers).toEqual({});
    }
  });
});

describe("writing a cue back", () => {
  it("KEEPS the comments and the other entries, a one-line diff", () => {
    const out = write(PLAIN, B, [12.5]);
    expect(out).toContain("// where this set drops the needle");
    expect(out).toContain(`  "${A}",\n`);
    expect(mixCues(out).markers).toEqual({ [B]: [12.5] });
  });

  it("holds a slot open, so button 3 stays button 3", () => {
    expect(mixCues(write(PLAIN, A, [null, null, 48])).markers).toEqual({
      [A]: [null, null, 48],
    });
  });

  it("drops the trailing empties rather than writing eight nulls", () => {
    const out = write(PLAIN, A, [12.5, null, null, null, null, null, null]);
    expect(mixCues(out).markers).toEqual({ [A]: [12.5] });
    expect(out).not.toContain("null");
  });

  it("goes back to a BARE ref when the last cue is cleared", () => {
    const cued = write(PLAIN, A, [12.5]);
    expect(write(cued, A, [null])).toBe(PLAIN);
  });

  it("replaces a song's cues rather than adding a second entry", () => {
    const twice = write(write(PLAIN, A, [10]), A, [10, 20]);
    expect(mixCues(twice).markers).toEqual({ [A]: [10, 20] });
    expect(twice.match(/Around the world/g)).toHaveLength(1);
  });

  it("APPENDS a ref the file does not name, and makes the key from nothing at all", () => {
    expect(mixCues(write("cues([])\n", A, [5])).markers).toEqual({ [A]: [5] });
    const fresh = write("", A, [5]);
    expect(mixCues(fresh)).toEqual({ markers: { [A]: [5] }, issues: [] });
  });

  it("writes the cues on ONE line, with the ref they belong to", () => {
    expect(write(PLAIN, A, [1, 2, 3])).toContain(
      `  ref("${A}").markers([1, 2, 3]),\n`,
    );
  });

  it("leaves a sheet that does not read as it is, and says so", () => {
    expect(() => write("cues([", A, [1])).toThrow();
  });
});

describe("the desk's bindings", () => {
  it("reads which control drives which param, null for one not wired yet", () => {
    const got = readMixView(
      'fade({ module: "xf", param: "Level", invert: true })\nfree(null)\n',
    );
    expect(got.issues).toEqual([]);
    expect(got.value).toEqual({
      fade: { module: "xf", param: "Level", invert: true },
      free: null,
    });
  });

  it("is empty for a project that wired nothing, and says why for one that is wrong", () => {
    expect(readMixView("")).toEqual({ value: {}, issues: [] });
    expect(readMixView('fade({ module: "", param: "x" })\n').value).toBeNull();
  });
});

describe("a playlist", () => {
  it("round-trips its own stub, so a file it creates opens on a playlist", () => {
    const stub = playlistStub("répèt du jeudi");
    expect(stub).toBe(
      '// répèt du jeudi\ntitle("répèt du jeudi")\nitems([])\n',
    );
    expect(parsePlaylist(stub)).toEqual({ title: "répèt du jeudi", items: [] });
  });

  const append = (text: string, song: string, variant?: string) =>
    editPlaylist(text, [{ op: "append", song, variant }]);

  it("adds to the end of the list, a bare song when it names no variant", () => {
    expect(
      append('title("Set")\nitems([\n  "Lithium",\n])\n', "Hells Bells").text,
    ).toBe('title("Set")\nitems([\n  "Lithium",\n  "Hells Bells",\n])\n');
  });

  it("fills the stub's empty list, and adds the key when the file has none", () => {
    const out = append(playlistStub("Set"), "Lithium").text;
    expect(out.startsWith("// Set\n")).toBe(true);
    expect(parsePlaylist(out)?.items).toEqual([{ song: "Lithium" }]);
    expect(parsePlaylist(append('title("Set")\n', "Lithium").text)).toEqual({
      title: "Set",
      items: [{ song: "Lithium" }],
    });
  });

  it("writes the variant beside the song", () => {
    const out = append('items(["Lithium"])\n', "Nevermind/Lithium", "inst");
    expect(parsePlaylist(out.text)?.items[1]).toEqual({
      song: "Nevermind/Lithium",
      variant: "inst",
    });
  });

  it("keeps the comments and the order", () => {
    const before = [
      "// la répèt du jeudi",
      'title("Répèt")',
      "items([",
      "  // on ouvre là-dessus",
      '  "Hells Bells",',
      '  "Lithium",',
      "])",
      "",
    ].join("\n");
    const out = append(before, "Rock 'n' Roll: Part 2").text;
    expect(out).toContain("// la répèt du jeudi");
    expect(out).toContain("  // on ouvre là-dessus");
    expect(parsePlaylist(out)?.items.map((i) => i.song)).toEqual([
      "Hells Bells",
      "Lithium",
      "Rock 'n' Roll: Part 2",
    ]);
  });

  it("refuses an `items` that is no list rather than guessing, leaving the text alone", () => {
    const text = 'items("nonsense")\n';
    expect(append(text, "x")).toMatchObject({ text, results: [false] });
  });

  const SET =
    'items([\n  "a",\n  // the second\n  song("b").variant("v"),\n  "c",\n])\n';
  const songs = (text: string) => parsePlaylist(text)?.items.map((i) => i.song);

  it("drops one line by its position, and refuses one that is not there", () => {
    expect(songs(editPlaylist(SET, [{ op: "remove", index: 0 }]).text)).toEqual(
      ["b", "c"],
    );
    expect(editPlaylist(SET, [{ op: "remove", index: 3 }]).results).toEqual([
      false,
    ]);
    expect(
      editPlaylist('title("x")\n', [{ op: "remove", index: 0 }]).results,
    ).toEqual([false]);
  });

  it("moves a line with its comment", () => {
    const out = editPlaylist(SET, [{ op: "move", from: 1, to: 2 }]).text;
    expect(songs(out)).toEqual(["a", "c", "b"]);
    expect(out).toContain("// the second");
    expect(editPlaylist(SET, [{ op: "move", from: 0, to: 5 }]).results).toEqual(
      [false],
    );
  });

  it("refuses a text that does not read", () => {
    expect(() => append("items([", "x")).toThrow();
  });
});

describe("the library pins", () => {
  it("reads pins, dropping what is no version, and writes them in name order", () => {
    const pins = parseLibraryPins(
      '"k7-core"(7)\ndrums("draft")\nbass("soon")\nkeys(0)\n',
    );
    expect(pins).toEqual({ "k7-core": 7, drums: "draft" });
    expect(stringifyLibraryPins(pins)).toBe('drums("draft")\nk7-core(7)\n');
    expect(parseLibraryPins(stringifyLibraryPins(pins))).toEqual(pins);
    expect(parseLibraryPins("drums(")).toEqual({});
  });
});

const TEMPLATE = `// A TURNTABLE — the deck and its strip, as one box.
label("Turntable")
outputs([
  // The channel's own fade, which is one half of the crossfader.
  name("mix").type("audio").node("fade"),
])
// The five a hand actually reaches for.
params({
  Trim: { node: "trim", param: "Trim" },
})
`;
const LEVEL = { node: "level", param: "Gain" };

describe("an assembly template", () => {
  const promote = (text: string, name: string, p = LEVEL) =>
    editAssembly(text, [{ op: "setParam", name, ...p }]).text;
  const drop = (text: string, name: string) =>
    editAssembly(text, [{ op: "dropParam", name }]);

  it("reads what it promotes, and places a finding on its line", () => {
    expect(readAssemblyTemplate(TEMPLATE).params).toEqual({
      Trim: { node: "trim", param: "Trim" },
    });
    const bad = readAssemblyTemplate('label("x")\nplugs([])\n');
    expect(bad.params).toBeNull();
    expect(bad.issues[0]?.line).toBe(2);
  });

  it("adds a promoted param and touches nothing else", () => {
    const text = promote(TEMPLATE, "Level");
    expect(text).toContain(
      "// A TURNTABLE — the deck and its strip, as one box.",
    );
    expect(text).toContain(
      "// The channel's own fade, which is one half of the crossfader.",
    );
    expect(text).toContain("// The five a hand actually reaches for.");
    expect(readAssemblyTemplate(text).params).toEqual({
      Trim: { node: "trim", param: "Trim" },
      Level: LEVEL,
    });
  });

  it("replaces what that name already promoted, and writes the block on a template that had none", () => {
    expect(readAssemblyTemplate(promote(TEMPLATE, "Trim")).params).toEqual({
      Trim: LEVEL,
    });
    expect(readAssemblyTemplate(promote('label("Box")\n', "T")).params).toEqual(
      { T: LEVEL },
    );
  });

  it("takes one back off: the round trip leaves the file as it was", () => {
    const two = promote(TEMPLATE, "Level");
    const out = drop(two, "Level");
    expect(out.results).toEqual([true]);
    expect(out.text).toBe(TEMPLATE);
  });

  it("takes the empty block with the LAST one, and is a no-op for a name nothing promoted", () => {
    const last = drop(TEMPLATE, "Trim").text;
    expect(last).not.toContain("params");
    expect(readAssemblyTemplate(last).params).toBeNull();
    expect(drop(TEMPLATE, "Nope")).toMatchObject({
      text: TEMPLATE,
      results: [false],
    });
  });

  it("refuses a file that does not read as a template", () => {
    expect(() => promote('label("x")\nplugs([])\n', "Level")).toThrow(
      /assembly\.k7x/,
    );
  });
});

describe("a chain file's items", () => {
  const CHAIN = `// the guitar chain
start(true)
steps([
  id("in").extends("@k7-core/io/in"),
  // the drive, set by ear
  id("drive").extends("@k7-core/audio/overdrive").params({ Amount: 5 }).bypass(true),
  [id("verb").extends("@k7-core/audio/reverb"), id("echo").extends("@k7-core/audio/delay")],
  to([chain("OUT")]),
])
`;

  it("lists the items of its steps, a parallel step's included, the end step left out", () => {
    expect(readChain(CHAIN)).toEqual({
      items: [
        { id: "in", extends: "@k7-core/io/in", bypass: false },
        { id: "drive", extends: "@k7-core/audio/overdrive", bypass: true },
        { id: "verb", extends: "@k7-core/audio/reverb", bypass: false },
        { id: "echo", extends: "@k7-core/audio/delay", bypass: false },
      ],
      issues: [],
    });
  });

  it("checks a node item as a module file, at the item's line", () => {
    const got = readChain(
      'start(true)\nsteps([\n  id("in").extends("@k7-core/io/in"),\n  id("x").paramz({}),\n])\n',
    );
    expect(got.items).toEqual([]);
    expect(got.issues[0]?.line).toBe(4);
  });

  it("reports a syntax error with its line", () => {
    expect(readChain("steps([\n").issues[0]?.line).toBeGreaterThanOrEqual(1);
  });
});

describe("the short forms", () => {
  it("expands a sequence's notes and a graph's runs, and leaves a kind with none", () => {
    expect(expandSugar("song/sequences/m.k7x", { notes: "c4 d4 ~ e4" })).toEqual(
      {
        notes: [
          { t: 0, d: 1, k: 60 },
          { t: 1, d: 1, k: 62 },
          { t: 3, d: 1, k: 64 },
        ],
      },
    );
    expect(
      expandSugar("song/graph.k7x", {
        modules: { MIX: ["OUT"] },
        runs: [
          ["melody", "lead", "MIX"],
          ["pad", "MIX"],
        ],
      }),
    ).toEqual({
      modules: { MIX: ["OUT"], melody: ["lead"], lead: ["MIX"], pad: ["MIX"] },
    });
    const plain = { notes: "c4" };
    expect(expandSugar("song/project.k7x", plain)).toEqual(plain);
    expect(expandSugar("song/chains/chains.k7x", plain)).toEqual(plain);
  });

  it("names what does not read", () => {
    expect(() =>
      expandSugar("song/sequences/m.k7x", { notes: "c4 h4" }),
    ).toThrow('notes: "h4" is neither a note name');
    expect(() =>
      expandSugar("song/sequences/m.k7x", { notes: "[c4 d4" }),
    ).toThrow('"]" missing');
  });

  const SEQ = '// the riff\nnotes("c4 d4 e4 f4")\nto("2:1")\n';
  const path = "song/sequences/riff.k7x";
  const four = [
    { t: 0, d: 1, k: 60 },
    { t: 1, d: 1, k: 62 },
    { t: 2, d: 1, k: 64 },
    { t: 3, d: 1, k: 65 },
  ];

  it("keeps the short form through an edit that leaves the notes alone", () => {
    expect(applySugared(path, SEQ, { notes: four, to: "3:1" }).text).toBe(
      SEQ.replace('"2:1"', '"3:1"'),
    );
  });

  it("is written as the full note list once a note changes", () => {
    const notes = four.map((n, i) => (i === 1 ? { ...n, v: 0.5 } : n));
    const out = applySugared(path, SEQ, { notes, to: "2:1" }).text;
    expect(out).toContain("// the riff");
    expect(out).not.toContain('notes("');
    expect(expandSugar(path, parse(out))).toEqual(
      expandSugar(path, { notes, to: "2:1" }),
    );
  });

  const GRAPH = 'runs([["melody", "lead", "MIX", "OUT"]])\nbypassed([])\n';
  const graph = "song/graph.k7x";
  const modules = {
    melody: ["lead"],
    lead: ["MIX"],
    MIX: ["OUT"],
  };

  it("keeps the runs through an edit elsewhere in the graph", () => {
    expect(
      applySugared(graph, GRAPH, { modules, bypassed: ["lead"] }).text,
    ).toBe(GRAPH.replace("bypassed([])", 'bypassed(["lead"])'));
  });

  it("writes the segments in full once one changes, and never brings a removed one back", () => {
    const out = applySugared(graph, GRAPH, {
      modules: { ...modules, lead: [] },
      bypassed: [],
    }).text;
    expect(out).not.toContain("runs(");
    expect(out).toContain("lead");
  });

  it("is unchanged by a write that changes nothing", () => {
    expect(applySugared(path, SEQ, { notes: four, to: "2:1" }).text).toBe(SEQ);
  });
});
