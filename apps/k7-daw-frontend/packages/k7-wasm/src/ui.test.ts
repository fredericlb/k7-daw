// The adapter over the `ui.*` ops: what crosses the ABI keeps its meaning, and a keystroke's ranking stays far inside a frame.

import { describe, expect, it } from "vitest";
import {
  emptyBarRows,
  fuzzyScores,
  rankCommands,
  rankOptions,
  setCommandPref,
  stepSentence,
  verbCatalogue,
} from "./ui.ts";

type Info = { command: string; readable?: { en: string; fr: string } };
const catalogue = verbCatalogue() as Info[];
const rows = catalogue.map((c) => ({
  canonical: c.command,
  readable: c.readable?.en ?? c.command,
}));

describe("the bar's rankings over the real catalogue", () => {
  it("finds a command by its sentence and by a typo-free fragment of its name", () => {
    const names = rankCommands(rows, "mute").map((c) => c.canonical);
    expect(names).toContain("track mute");
  });

  it("answers a keystroke on the whole catalogue in well under a frame", () => {
    const queries = ["t", "tr", "track m", "mute", "a", "ev", "e s"];
    for (const q of queries) rankCommands(rows, q, ["track mute"]);
    const t0 = performance.now();
    const reps = 100;
    for (let r = 0; r < reps; r++)
      for (const q of queries) rankCommands(rows, q, ["track mute"]);
    const each = (performance.now() - t0) / (reps * queries.length);
    // A frame is 16 ms: the budget is a third of a frame, which the measure beats by a wide margin.
    expect(each).toBeLessThan(5);
  });
});

describe("what crosses the ABI", () => {
  it("scores a subsequence and refuses what is not one", () => {
    const [near, far] = fuzzyScores("tm", ["track mute", "xyz"]);
    expect(near).toBeGreaterThan(0);
    expect(far).toBe(-1);
  });

  it("ranks options on the label as on the value", () => {
    const opts = [{ value: "0b7c", label: "auto-wah" }, { value: "gtr" }];
    expect(rankOptions(opts, "wah")).toEqual([opts[0]]);
  });

  it("lists the recents the catalogue holds, then the starters", () => {
    const cmds = [{ canonical: "track new" }, { canonical: "ui help" }];
    expect(emptyBarRows(cmds, ["ui help"]).map((r) => r.recent)).toEqual([
      true,
      false,
    ]);
  });

  it("sends an undefined field as a taken-back one", () => {
    const prefs = { a: { icon: "Scissors", order: 0 } };
    expect(setCommandPref(prefs, "a", { icon: undefined })).toEqual({
      a: { order: 0 },
    });
  });

  it("reads a step as its sentence then its subject, named by the page", () => {
    const commands = [
      {
        command: "track mute",
        readable: { en: "Mute a track" },
        args: [{ provider: "tracks" }],
      },
    ];
    expect(
      stepSentence("track mute gtr", commands, "en", {
        lane: (id) => id.toUpperCase(),
      }),
    ).toBe("Mute a track · GTR");
  });
});
