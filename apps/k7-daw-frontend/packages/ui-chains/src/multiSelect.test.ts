// The wiring of the selection rules: Sets and cards in, names across, cards back. The rules are tested in the chains crate
// (apps/k7-daw-wasm/chains/src/select/tests.rs).

import { describe, expect, it } from "vitest";
import type { GraphNode } from "./graph";
import {
  linkedNodes,
  linkedValue,
  nodesInBox,
  toggleSelection,
} from "./multiSelect";

const node = (module: string, over: Partial<GraphNode> = {}): GraphNode =>
  ({
    id: `mod:${module}`,
    label: module,
    kind: "module",
    module,
    uses: "@k7-core/audio/amp",
    x: 0,
    y: 0,
    w: 100,
    h: 100,
    dim: false,
    ...over,
  }) as GraphNode;

describe("multiSelect", () => {
  it("toggles a Set, growing from the single selection", () => {
    const two = toggleSelection(new Set(), "amp_1", "amp_2");
    expect([...two]).toEqual(["amp_1", "amp_2"]);
    expect([...toggleSelection(two, "amp_1", "amp_2")]).toEqual(["amp_1"]);
  });

  it("hands back the very nodes it links, the baked one left out", () => {
    const [a, b] = [node("amp_1"), node("amp_2")];
    const frozen = node("amp_3", { frozen: "at" });
    const sel = new Set(["amp_1", "amp_2", "amp_3"]);
    expect(linkedNodes([a, b, frozen], sel, a)).toEqual([b]);
    expect(linkedNodes([a, b], new Set(["amp_1"]), a)).toEqual([]);
  });

  it("catches the cards a box touches", () => {
    const nodes = [node("a", { h: 50 }), node("b", { x: 200, h: 50 })];
    expect(nodesInBox(nodes, { x0: 150, y0: 10, x1: 250, y1: 20 })).toEqual([
      "b",
    ]);
  });
});

describe("linkedValue", () => {
  const knob = {
    type: "knob",
    col: 0,
    row: 0,
    bind: "Drive",
    min: 0,
    max: 10,
  } as const;

  it("moves a linked knob by the same TRAVEL from its own start, not to the same value", () => {
    // The knob in hand goes 0.2 → 0.5 of its throw; one that stood at 0.6 lands at 0.9, i.e. 9.
    expect(linkedValue(knob, 0.6, 0.2, 0.5)).toBeCloseTo(9);
  });

  it("stops at the end of its own throw", () => {
    expect(linkedValue(knob, 0.9, 0.2, 0.5)).toBe(10);
    expect(linkedValue(knob, 0.1, 0.5, 0.2)).toBe(0);
  });
});
