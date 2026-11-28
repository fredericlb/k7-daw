// panel.test.ts — the builder's AI check, prompt and param rows reach the module and answer the shapes the pane draws.
// The rules (every check sentence, prompt section and row union) are song's: modules/ui/ai_cases_tests.rs, rows_tests.rs.

import { describe, expect, it } from "vitest";
import {
  aiDesignBrief,
  aiUserPrompt,
  bindableNames,
  checkAiResult,
  hasLayout,
  paramRows,
  stageFor,
} from "./panel.ts";

const UI =
  'cols(4)\nrows(2)\nwidgets({\n  cut: { type: "knob", col: 0, row: 0, colSpan: 2, rowSpan: 2, bind: "Cutoff", min: 0, max: 1 },\n})\n';

describe("AI organize", () => {
  it("checks a proposal into the panel it reads and the sentences for the model", () => {
    const ok = checkAiResult({ uiText: UI, widgetsJs: "", notes: "" }, [
      "Cutoff",
    ]);
    expect(ok.issues).toEqual([]);
    expect(Object.keys(ok.ui?.widgets ?? {})).toEqual(["cut"]);
    const bad = checkAiResult({ uiText: UI, widgetsJs: "", notes: "" }, [
      "Other",
    ]);
    expect(bad.issues).toEqual([
      "`cut` binds `Cutoff`, which is not a parameter",
    ]);
  });

  it("builds the prompt, offers a rack's taps and tells a laid-out panel from an empty one", () => {
    expect(aiDesignBrief()).toContain("`knob`");
    expect(
      bindableNames({
        params: [{ name: "Volume" }],
        moduleText: 'type("rack")\nrack({ symbols: { BD: { note: 36 } } })\n',
      }),
    ).toEqual(["Volume", "@rack:BD", "@rack+"]);
    expect(hasLayout(UI)).toBe(true);
    expect(hasLayout("")).toBe(false);
    const text = aiUserPrompt({
      module: "filter",
      moduleText: "",
      params: [{ name: "Cutoff" }],
      uiText: UI,
      widgetsJs: "",
      mode: "update",
      direction: "bigger",
    });
    expect(text).toContain("## What to change\nbigger");
  });
});

describe("the param rows", () => {
  it("names the footer a folder gets and unions the rows of every source", () => {
    expect(stageFor("modules/audio/d/module.k7x", null)).toEqual([
      "@input-volume",
      "@mix",
      "@output-volume",
    ]);
    const rows = paramRows({
      content: "params({ Time: 0.5 })\n",
      bound: ["Time"],
      path: "modules/audio/d/module.k7x",
      hintParams: ["Time"],
      hintMeta: {},
      pluginNames: [],
      pluginMeta: { Time: { min: 0, max: 2 } },
      scannedNames: [],
      scannedMeta: {},
      baseBinds: [],
    });
    expect(rows.find((r) => r.name === "Time")).toMatchObject({
      bound: true,
      value: "0.5",
      meta: { min: 0, max: 2 },
    });
    expect(rows.find((r) => r.name === "@mix")).toBeDefined();
  });
});
