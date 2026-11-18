// Every k7-core `jsdsp.ts`, run by @studio/dsp's builder, against its committed `jsdsp.json`: the graph a node host plays
// for the module's `dsp.k7x`, which the song crate's vectors hold to the same builder (docs/dsp-literal-plan.md).
// A failure is fixed by printing the module's graph again and committing the new `jsdsp.json`.

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { graph } from "@studio/dsp";
import { describe, expect, it } from "vitest";

const here = dirname(fileURLToPath(import.meta.url));
const K7_CORE = join(here, "..", "..", "..", "k7-core");

const sources = readdirSync(K7_CORE, { recursive: true, encoding: "utf8" })
  .filter((f) => f.replaceAll("\\", "/").endsWith("/jsdsp.ts"))
  .map((f) => join(K7_CORE, f))
  .sort();

describe("k7-core's DSP modules", () => {
  it("are all found", () => {
    expect(sources.length).toBeGreaterThan(10);
  });

  it.each(
    sources.map((s) => [
      relative(K7_CORE, dirname(s)).replaceAll("\\", "/"),
      s,
    ]),
  )(
    "%s prints its committed jsdsp.json and has a dsp.k7x",
    async (_rel, source) => {
      const m = await import(pathToFileURL(source).href);
      const committed = readFileSync(source.replace(/\.ts$/, ".json"), "utf8");
      expect(`${JSON.stringify(graph(m.default), null, 2)}\n`).toBe(committed);
      expect(readFileSync(join(dirname(source), "dsp.k7x"), "utf8")).toContain(
        "graph(",
      );
    },
  );
});
