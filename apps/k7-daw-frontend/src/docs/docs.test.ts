// Does the documentation still describe the software? Two failures, both of which have happened: an example
// goes stale (caught by judging every example with the loader's own reader), and a field is added and never
// written down (caught by walking each kind's keys and asking whether any example mentions them).
// Both read the markdown under k7/docs/ itself. See ../../docs/docExamples.md.

import { fieldsAt } from "@studio/k7-wasm/fields";
import { validateFile } from "@studio/k7-wasm/validate";
import { parse } from "@studio/k7x/document";
import { GraphFileSchema } from "@studio/song-schema/yaml/graphSchema";
import {
  ModuleSchema,
  UiFileSchema,
} from "@studio/song-schema/yaml/moduleSchema";
import { SurfaceSchema } from "@studio/song-schema/yaml/surfaceSchema";
import { describe, expect, it } from "vitest";
import type { z } from "zod";
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
};

// The entries still read by a TypeScript schema, until their readers move to Rust.
const SCHEMAS: Record<string, z.ZodType> = {
  Surface: SurfaceSchema,
  GraphFile: GraphFileSchema,
  Module: ModuleSchema,
  UiFile: UiFileSchema,
};

describe("the documented file formats", () => {
  it("names a reader for every entry, or says why not", () => {
    // Without this an entry could dodge both checks by simply not declaring a schema.
    for (const e of DOC_EXAMPLES) {
      if (e.schema) {
        expect(
          READERS[e.schema] ?? SCHEMAS[e.schema],
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
      const path = READERS[e.schema as string];
      if (path) {
        const v = validateFile(path, e.code);
        const why = v.issues
          .map((i) => `  ${i.path || "(root)"}: ${i.message}`)
          .join("\n");
        expect(v.ok, `${e.where}\n${why}`).toBe(true);
        return;
      }
      const result = SCHEMAS[e.schema as string].safeParse(parse(e.code));
      // The whole issue list, not just the first: a stale example is usually stale in more than one place.
      const why = result.success
        ? ""
        : result.error.issues
            .map((i) => `  ${i.path.join(".") || "(root)"}: ${i.message}`)
            .join("\n");
      expect(result.success, `${e.where}\n${why}`).toBe(true);
    });
  }
});

// Every top-level key a schema declares, for the coverage check below — a union contributes the keys of every branch.
function topLevelKeys(schema: z.ZodType): string[] {
  const def = (schema as unknown as { def?: Record<string, unknown> }).def;
  if (!def) return [];
  if (def.type === "object")
    return Object.keys((def.shape ?? {}) as Record<string, unknown>);
  if (def.type === "union")
    return [
      ...new Set(
        ((def.options ?? []) as z.ZodType[]).flatMap((o) => topLevelKeys(o)),
      ),
    ];
  return [];
}

// The readers whose editor vocabulary lists their keys; a timeline, a settings file, a config and a preset have none.
const KEYED = new Set(["ProjectFile", "Clip", "Sequence", "Curve"]);

/** The keys a kind's reader declares at its top level, from the editor's field vocabulary. */
const readerKeys = (name: string): string[] | null =>
  KEYED.has(name) ? fieldsAt(READERS[name], []) : null;

describe("documentation coverage", () => {
  // A field a reader accepts but no example mentions is one nobody can discover except by reading the source.
  const names = [...Object.keys(READERS), ...Object.keys(SCHEMAS)];
  for (const name of names) {
    const examples = DOC_EXAMPLES.filter((e) => e.schema === name);
    if (!examples.length) continue;
    const keys = SCHEMAS[name] ? topLevelKeys(SCHEMAS[name]) : readerKeys(name);
    if (!keys) continue;
    it(`${name}: every field appears in an example`, () => {
      // The prose counts as documentation too: a field is often explained in the sentence above the block rather than shown in it.
      const text = examples.map((e) => `${e.code}\n${e.context}`).join("\n");
      const undocumented = keys.filter((k) => !text.includes(k));
      expect(undocumented, `undocumented ${name} fields`).toEqual([]);
    });
  }
});
