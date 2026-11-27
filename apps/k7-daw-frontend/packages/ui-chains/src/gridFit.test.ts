// gridFit.test.ts — a card built with a fit is sized to its folded grid, and a row sizes it to the tab it has open.
// The fold itself is tested in Rust: apps/k7-daw-wasm/chains/src/grid_fit/tests.rs.

import type { PageModuleUI, PageWidget, WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import { buildGraph, FOOTER_H, moduleRow, nodeSize } from "./graph";

const knob = (
  col: number,
  row: number,
  extra: Partial<PageWidget> = {},
): PageWidget => ({
  type: "knob",
  col,
  row,
  ...extra,
});

/** A 2-wide grid, one knob per cell, `rows` tall. */
function column(rows: number): PageModuleUI {
  const widgets: Record<string, PageWidget> = {};
  for (let r = 0; r < rows; r++)
    for (let c = 0; c < 2; c++) widgets[`k${r}${c}`] = knob(c, r);
  return { cols: 2, rows, widgets };
}

describe("buildGraph with a fit", () => {
  const g = (ui: PageModuleUI): WebGraph => ({
    chains: {
      lead: {
        from: "keys",
        use: [{ module: "big", ui }],
        out: [{ to: "OUT", gain: 1 }],
      },
    },
    tracks: ["keys"],
    warnings: [],
  });
  const card = (fit?: Parameters<typeof buildGraph>[1]) =>
    buildGraph(g(column(9)), fit).nodes.find((n) => n.module === "big");

  it("folds a card's panel and sizes the card to the folded grid", () => {
    const n = card({ fit: { axis: "rows", max: 6 } });
    expect(n?.ui).toMatchObject({ rows: 6, cols: 4 });
    const folded = nodeSize({ cols: 4, rows: 6, widgets: {} });
    expect(n?.w).toBe(folded.w);
    expect(n?.h).toBe(folded.h + FOOTER_H);
  });

  it("leaves every card as written without one", () => {
    expect(card()?.ui).toMatchObject({ rows: 9, cols: 2 });
  });
});

// A card sized to its OPEN tab: the fold records each tab's own width, the layout reads the one showing.
describe("a tab's own width", () => {
  const wide = (
    name: string,
    cols: number,
  ): NonNullable<PageModuleUI["tabs"]>[number] => ({
    name,
    widgets: Object.fromEntries(
      Array.from({ length: 8 }, (_, r) =>
        Array.from({ length: cols }, (_, c) => [`${name}${r}${c}`, knob(c, r)]),
      ).flat(),
    ),
  });
  const ui: PageModuleUI = {
    cols: 6,
    rows: 8,
    widgets: {},
    tabs: [wide("narrow", 2), wide("broad", 6)],
  };

  it("sizes the card to the open tab, and moves the next card with it", () => {
    const graph: WebGraph = {
      chains: {
        c: {
          use: [{ module: "synth", ui }, { module: "after" }],
          out: [{ to: "MIX", gain: 1 }],
        },
      },
      tracks: [],
      warnings: [],
    };
    const fit = { axis: "rows" as const, max: 4 };
    const at = (tab: number) => {
      const built = buildGraph(graph, { fit }).nodes.find(
        (n) => n.module === "synth",
      );
      const laid = moduleRow(graph, ["synth", "after"], {
        asDrawn: true,
        fit,
        tabOf: built ? { [built.id]: tab } : {},
      });
      const [synth, after] = laid.nodes;
      return { w: synth.w, next: after.x };
    };
    const narrow = at(0);
    const broad = at(1);
    expect(narrow.w).toBeLessThan(broad.w);
    expect(broad.next - narrow.next).toBe(broad.w - narrow.w);
  });
});
