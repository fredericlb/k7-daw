// The sets the surfaces test against: one wiring case for Rust's marks, and the page's own `nodeMarked` rule.
// Which file lights what is tested in the chains crate: apps/k7-daw-wasm/chains/src/history_marks/tests.rs.

import type { WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import { historyMarks, isEmptyMark, nodeMarked } from "./historyMarks";

const graph = {
  chains: {
    "be.bass": {
      from: "be.bass",
      use: [{ module: "5ee626e6" }, { module: "fb4235d5" }],
      out: [],
    },
  },
  tracks: ["be.bass"],
  warnings: [],
} as unknown as WebGraph;

describe("historyMarks", () => {
  it("answers Rust's marks as sets, the label's node lit inside its chain", () => {
    const m = historyMarks(
      ["tracks/da.main.k7x", "chains/be.bass.k7x"],
      'node param "5ee626e6" "model" "@k7-core/x.nam"',
      graph,
    );
    expect([...m.lanes]).toEqual(["da.main"]);
    expect([...m.modules]).toEqual(["5ee626e6"]);
    expect(m.chains.size).toBe(0);
    expect(isEmptyMark(historyMarks(["project.k7x"]))).toBe(true);
  });
});

describe("nodeMarked", () => {
  const m = historyMarks(["tracks/da.main.k7x", "chains/be.bass.k7x"]);
  it("lights a lane's own card, and every card of a lit chain", () => {
    expect(nodeMarked(m, { kind: "track", module: "da.main" })).toBe(true);
    expect(
      nodeMarked(m, { kind: "module", module: "x", chain: "be.bass" }),
    ).toBe(true);
    expect(
      nodeMarked(m, { kind: "module", module: "y", chains: ["a", "be.bass"] }),
    ).toBe(true);
    expect(nodeMarked(m, { kind: "module", module: "z", chain: "other" })).toBe(
      false,
    );
  });
});
