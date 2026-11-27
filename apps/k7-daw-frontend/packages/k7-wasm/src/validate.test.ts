// validate.test.ts — the editor's check answered by the Rust readers, held to the cases the TypeScript router was tested on.

import { FORMAT_LEVEL } from "@studio/k7-wasm/consts";
import { describe, expect, it } from "vitest";
import { firstIssue, validateFile } from "./validate.ts";

const MODULE = `// a node
extends("@workspace/instruments/synth")
params({
  "Filter Freq": 40,
  Level: -12,
})
`;
const NEXT = FORMAT_LEVEL + 1;
const CHAIN = `start(true)
steps([
  { id: "l1", track: "gtr" },
  { id: "a1", extends: "@k7-core/audio/chorus" },
  { to: [{ chain: "MIX" }] },
])
`;

describe("a .k7x buffer", () => {
  it("passes when the compiler could read it", () => {
    expect(validateFile("song/nodes/pad.k7x", MODULE)).toMatchObject({
      ok: true,
      depth: "schema",
    });
  });

  it("reports a syntax error at its own line and column", () => {
    const v = validateFile("song/nodes/pad.k7x", `${MODULE}bogus(1)\noops`);
    expect(v).toMatchObject({ ok: false, malformed: true, depth: "syntax" });
    expect(v.issues[0]).toMatchObject({ line: 8, col: 5 });
    expect(v.issues[0].message).toBe('expected "(" after oops');
  });

  it("reports a schema finding on the k7x line of the key it is about", () => {
    const v = validateFile("song/nodes/pad.k7x", `${MODULE}nonsense(1)\n`);
    expect(v.ok).toBe(false);
    expect(v.issues).toHaveLength(1);
    expect(v.issues[0]).toMatchObject({ line: 7, col: 1, path: "nonsense" });
  });

  it("places a nested finding inside its object", () => {
    const text = 'project({\n  name: "demo",\n  tempo: "fast",\n})\n';
    const v = validateFile("song/project.k7x", text);
    expect(v.ok).toBe(false);
    expect(v.issues[0]).toMatchObject({
      path: "project.tempo",
      line: 3,
      col: 3,
    });
  });

  it("names a missing key by its path, at the object that lacks it", () => {
    const v = validateFile("song/sequences/riff.k7x", 'to("2:1")\n');
    expect(v.issues[0].path).toBe("notes");
    expect(firstIssue(v)).toContain("notes: missing key");
  });

  it("counts a column in UTF-16 units, as CodeMirror does", () => {
    const text = 'params({ n: "é😀", a: ])\n';
    const v = validateFile("song/nodes/pad.k7x", text);
    expect(v.malformed).toBe(true);
    const bracket = [...text].findIndex((c) => c === "]");
    const units = text.slice(0, text.indexOf("]")).length;
    expect(bracket).toBeLessThan(units);
    expect(Math.abs((v.issues[0].col ?? 0) - 1 - units)).toBeLessThanOrEqual(2);
  });

  it("judges no file that is not a workspace document", () => {
    expect(validateFile("song/chains/gtr.yaml", "steps: [\n")).toMatchObject({
      ok: true,
      depth: "none",
    });
  });
});

describe("the router by folder", () => {
  it("checks chains/<name>.k7x as a chain file", () => {
    expect(validateFile("song/chains/gtr.k7x", CHAIN)).toMatchObject({
      ok: true,
      depth: "schema",
    });
    expect(
      validateFile("chains/gtr.k7x", 'steps([{ id: "l1", track: "gtr" }])\n')
        .ok,
    ).toBe(false);
  });

  it("checks tracks/<lane>.k7x as a module file", () => {
    const v = validateFile("tracks/gtr.k7x", "extends(3)\n");
    expect(v.depth).toBe("schema");
    expect(v.ok).toBe(false);
  });

  it("keeps a library chain folder's graph.k7x and nodes/ as they were read", () => {
    expect(
      validateFile("_libraries/k7-core/chains/verb/graph.k7x", "modules({})\n")
        .ok,
    ).toBe(true);
    expect(
      validateFile("_libraries/k7-core/chains/verb/nodes/v.k7x", "extends(3)\n")
        .ok,
    ).toBe(false);
  });

  it("reads the desk's two files, the presets and a module folder's config", () => {
    expect(validateFile("mix.k7x", 'cues(["records/a.flac"])\n').ok).toBe(true);
    expect(validateFile("mix.k7x", 'cues(["/a.flac"])\n').ok).toBe(false);
    expect(
      validateFile("views/mix.k7x", 'xfader({ module: "a", param: "b" })\n').ok,
    ).toBe(true);
    expect(validateFile("views/mix.k7x", "xfader(null)\n").ok).toBe(true);
    expect(
      validateFile("_presets/k7-core/a/warm.k7x", "params({ gain: 0.5 })\n").ok,
    ).toBe(true);
    expect(validateFile("modules/a/config.k7x", 'namespace("k7")\n').ok).toBe(
      true,
    );
    expect(validateFile("modules/a/config.k7x", "\n").ok).toBe(false);
  });
});

describe("a project root", () => {
  it("refuses a newer project with the update message, on the format line", () => {
    const v = validateFile(
      "song.k7/project.k7x",
      `project({\n  tempo: 120\n})\nformat(${NEXT})\n`,
    );
    expect(v.ok).toBe(false);
    expect(v.issues).toHaveLength(1);
    expect(v.issues[0].message).toMatch(/update k7 to open it/);
    expect(v.issues[0].line).toBe(4);
  });

  it("refuses a newer project even when the rest does not parse", () => {
    const v = validateFile(
      "song.k7/project.k7x",
      `format(${NEXT})\nproject(@@ new syntax)\n`,
    );
    expect(v.ok).toBe(false);
    expect(v.malformed).toBe(false);
    expect(v.issues[0].message).toMatch(/newer k7/);
  });

  it("accepts the current level, written or absent", () => {
    expect(
      validateFile(
        "p.k7/project.k7x",
        `format(${FORMAT_LEVEL})\nproject({ tempo: 120 })\n`,
      ).ok,
    ).toBe(true);
    expect(
      validateFile("p.k7/project.k7x", "project({ tempo: 120 })\n").ok,
    ).toBe(true);
  });

  it("leaves a `format` key in any other file to that file's own schema", () => {
    const v = validateFile("p.k7/clips/a.k7x", `format(${NEXT})\n`);
    expect(v.issues.every((i) => !/newer k7/.test(i.message))).toBe(true);
  });
});

describe("a sequence written with a short form", () => {
  it("passes its schema, and a note that does not read is placed on its line", () => {
    expect(
      validateFile("song/sequences/m.k7x", 'notes("c4 d4 e4")\nto("2:1")\n').ok,
    ).toBe(true);
    const v = validateFile(
      "song/sequences/m.k7x",
      '// a riff\nnotes("c4 x9")\n',
    );
    expect(v.ok).toBe(false);
    expect(v.issues[0]).toMatchObject({ path: "notes", line: 2, col: 1 });
  });
});
