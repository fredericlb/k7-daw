// modulePreview.test.ts — one module as a graph: the source it hangs off, its type folder, the grid it draws, its bases.
// How the build draws that graph (the bases dropdown, the card's size, the MIDI cable) is the `chains` crate's: src/tests/vertical.rs.

import { describe, expect, it } from "vitest";
import {
  moduleCategoryOf,
  modulePreviewGraph,
  PREVIEW_CHAIN,
  PREVIEW_MIDI_SOURCE,
  PREVIEW_SOURCE,
  visibleGrid,
} from "./modulePreview";

const ui = (over: Record<string, unknown> = {}) => ({
  cols: 2,
  rows: 1,
  widgets: {
    drive: { type: "knob" as const, col: 0, row: 0, bind: "drive" },
  },
  ...over,
});

describe("one module as a graph", () => {
  it("hangs the module off a source and routes it out", () => {
    const g = modulePreviewGraph("fuzz", "modules/audio/fuzz", ui(), {});
    expect(g.tracks).toEqual([PREVIEW_SOURCE]);
    expect(g.chains[PREVIEW_CHAIN].from).toBe(PREVIEW_SOURCE);
    expect(g.chains[PREVIEW_CHAIN].out).toEqual([{ to: "OUT", gain: 1 }]);
  });

  it("feeds a module that takes notes from a MIDI source, with a MIDI cable", () => {
    const synth = modulePreviewGraph(
      "jun",
      "modules/instruments/synth/jun",
      ui(),
      {},
    );
    expect(synth.tracks).toEqual([PREVIEW_MIDI_SOURCE]);
    expect(synth.trackSignals).toEqual({ [PREVIEW_MIDI_SOURCE]: "midi" });
    expect(synth.chains[PREVIEW_CHAIN].use[0].emits).toBe("audio");
    const arp = modulePreviewGraph("arp", "modules/transformers/arp", ui(), {});
    expect(arp.chains[PREVIEW_CHAIN].from).toBe(PREVIEW_MIDI_SOURCE);
    expect(arp.chains[PREVIEW_CHAIN].use[0].emits).toBe("midi");
  });

  it("gives the node an engine address rather than leaving it decorative", () => {
    const g = modulePreviewGraph("fuzz", "modules/audio/fuzz", ui(), {});
    expect(g.chains[PREVIEW_CHAIN].use[0].pluginId).toBe(
      `${PREVIEW_CHAIN}:fuzz`,
    );
  });

  it("carries the grid and the values the file sets", () => {
    const g = modulePreviewGraph("fuzz", "modules/audio/fuzz", ui(), {
      drive: 0.7,
    });
    expect(g.chains[PREVIEW_CHAIN].use[0].ui?.cols).toBe(2);
    expect(g.chains[PREVIEW_CHAIN].use[0].params).toEqual({ drive: 0.7 });
  });

  it("carries the module's bases, and says nothing when it has none", () => {
    // The absence is read as "no base control here", so an empty folder must not arrive as [].
    const none = modulePreviewGraph("fuzz", "modules/audio/fuzz", ui(), {}, []);
    expect(none.chains[PREVIEW_CHAIN].use[0].bases).toBeUndefined();
    const some = modulePreviewGraph("fuzz", "modules/audio/fuzz", ui(), {}, [
      "warm",
    ]);
    expect(some.chains[PREVIEW_CHAIN].use[0].bases).toEqual(["warm"]);
  });
});

describe("the grid the card draws", () => {
  it("drops hidden widgets", () => {
    // `hidden` is resolved by the compiler's merge, and there is no merge on this path.
    const grid = visibleGrid(
      ui({
        widgets: {
          drive: { type: "knob", col: 0, row: 0, bind: "drive" },
          secret: { type: "knob", col: 1, row: 0, bind: "x", hidden: true },
        },
      }),
    );
    expect(Object.keys(grid.widgets)).toEqual(["drive"]);
  });

  it("keeps the grid's own size, not the visible widgets' extent", () => {
    // The card is sized from cols × rows, so a hidden widget must not shrink it.
    const grid = visibleGrid(ui({ cols: 4, rows: 3, widgets: {} }));
    expect([grid.cols, grid.rows]).toEqual([4, 3]);
  });
});

// The four path shapes the two hosts hand over, for which no fixed index is right. docs/graphBuild.md.
describe("reading a module's type folder", () => {
  it("takes the segment after modules/", () => {
    expect(moduleCategoryOf("modules/instruments/synth")).toBe("instruments");
    expect(moduleCategoryOf("modules/audio/reverb/module.k7x")).toBe("audio");
  });

  it("reads a shared library, one folder deeper", () => {
    expect(moduleCategoryOf("_modules/audio/delay/tape-delay/module.k7x")).toBe(
      "audio",
    );
  });

  it("reads one behind a project prefix, with backslashes", () => {
    // What k7' file tree hands over on Windows in a multi-project workspace.
    expect(
      moduleCategoryOf(
        "LEMO - DANSE\\modules\\instruments\\drum_1\\module.k7x",
      ),
    ).toBe("instruments");
  });

  it("falls back rather than inventing a category the graph cannot draw", () => {
    expect(moduleCategoryOf("modules/whatever/thing")).toBe("audio");
    expect(moduleCategoryOf("nonsense")).toBe("audio");
    expect(moduleCategoryOf("")).toBe("audio");
  });
});
