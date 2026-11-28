// The wiring of the group's edge: widgets in, a Map of insets out. The rule is tested in the chains crate
// (apps/k7-daw-wasm/chains/src/group_edge.rs).

import type { PageWidget } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import { groupEdgeInsets } from "./groupEdge";

describe("groupEdgeInsets", () => {
  it("answers a Map by widget id, in the order Rust met them", () => {
    const widgets = {
      g: { type: "group", col: 2, row: 0, colSpan: 6, rowSpan: 3, label: "F" },
      a: { type: "knob", col: 2, row: 0, colSpan: 2, rowSpan: 2, bind: "A" },
      d: { type: "dropdown", col: 2, row: 2, colSpan: 2, bind: "T" },
      out: { type: "knob", col: 0, row: 0, colSpan: 2, bind: "G" },
    } as unknown as Record<string, PageWidget>;
    expect([...groupEdgeInsets(widgets)]).toEqual([
      ["a", 12],
      ["d", 16],
    ]);
  });
});
