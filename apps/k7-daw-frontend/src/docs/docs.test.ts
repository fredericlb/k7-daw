// Does the documentation still describe the software? Two failures, both of which have happened: an example
// goes stale (caught by judging every example with the loader's own reader), and a field is added and never
// written down (caught by walking each kind's keys and asking whether any example mentions them).
// Both read the markdown under k7/docs/ itself. See ../../docs/docExamples.md.

import { fieldsAt } from "@studio/k7-wasm/fields";
import { validateFile } from "@studio/k7-wasm/validate";
import { describe, expect, it } from "vitest";
import { DOC_EXAMPLES } from "./examples";

// The name an entity entry uses → the path whose reader the loader judges that file with: Rust's, through the editor's check.
const READERS: Record<string, string> = {
  ProjectFile: "project.k7x",
  Settings: "settings.k7x",
  // No `Track`: a lane is a NODE of a `@meta/…-track` module, so its examples declare `schema=Module` like every other node.
  Clip: "clips/x.k7x",
  Sequence: "sequences/x.k7x",
  Curve: "curves/x.k7x",
  Timeline: "timelines/x.k7x",
  ModuleConfig: "modules/x/config.k7x",
  PresetFile: "_presets/k7-core/x/p.k7x",
  Surface: "surfaces/x.k7x",
  GraphFile: "graph.k7x",
  Module: "nodes/x.k7x",
  UiFile: "modules/audio/x/ui.k7x",
};

describe("the documented file formats", () => {
  it("names a reader for every entry, or says why not", () => {
    // Without this an entry could dodge both checks by simply not declaring a schema.
    for (const e of DOC_EXAMPLES) {
      if (e.schema) {
        expect(
          READERS[e.schema],
          `${e.where}: unknown schema "${e.schema}"`,
        ).toBeDefined();
      } else {
        expect(
          e.exempt,
          `${e.where}: no schema and no exemption reason`,
        ).toBeTruthy();
      }
    }
  });

  for (const e of DOC_EXAMPLES.filter((x) => x.schema)) {
    it(`${e.where} is valid k7x the loader would accept`, () => {
      const v = validateFile(READERS[e.schema as string], e.code);
      // The reader's first finding only: it stops where it cannot read.
      const why = v.issues
        .map((i) => `  ${i.path || "(root)"}: ${i.message}`)
        .join("\n");
      expect(v.ok, `${e.where}\n${why}`).toBe(true);
    });
  }
});

// The readers whose editor vocabulary lists their keys; a timeline, a settings file, a config and a preset have none.
const KEYED = new Set(["ProjectFile", "Clip", "Sequence", "Curve", "Module"]);

/** The keys a kind's reader declares at its top level, from the editor's field vocabulary. */
const readerKeys = (name: string): string[] | null =>
  KEYED.has(name) ? fieldsAt(READERS[name], []) : null;

describe("documentation coverage", () => {
  // A field a reader accepts but no example mentions is one nobody can discover except by reading the source.
  const names = Object.keys(READERS);
  for (const name of names) {
    const examples = DOC_EXAMPLES.filter((e) => e.schema === name);
    if (!examples.length) continue;
    const keys = readerKeys(name);
    if (!keys) continue;
    it(`${name}: every field appears in an example`, () => {
      // The prose counts as documentation too: a field is often explained in the sentence above the block rather than shown in it.
      const text = examples.map((e) => `${e.code}\n${e.context}`).join("\n");
      const undocumented = keys.filter((k) => !text.includes(k));
      expect(undocumented, `undocumented ${name} fields`).toEqual([]);
    });
  }
});
