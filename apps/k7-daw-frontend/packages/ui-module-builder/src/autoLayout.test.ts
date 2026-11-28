// The panel facade's wiring for Auto organize: the rules and their cases are the song crate's (modules/ui/auto_cases_tests.rs).

import { autoOrganize, familyOf, groupParams } from "@studio/k7-wasm/panel";
import { describe, expect, it } from "vitest";

describe("auto organize through the facade", () => {
  it("answers a family and the pages of a list, and a panel with one knob per parameter", () => {
    expect(familyOf("Reverb Room Size")).toBe("Reverb");
    const params = [{ name: "Filter Cutoff", meta: { min: 20, max: 20000 } }];
    expect(groupParams(params).map((g) => g.page)).toEqual(["Filter"]);
    const ui = autoOrganize(params);
    expect(Object.values(ui.widgets).map((w) => w.bind)).toEqual([
      "Filter Cutoff",
    ]);
    expect(Object.values(ui.widgets)[0]).toMatchObject({
      type: "knob",
      min: 20,
      max: 20000,
    });
  });
});
