// The params a module's widgets may bind, unioned from every source that knows some: the cases the builder's pane relies on.

import { describe, expect, it } from "vitest";
import { paramRows, paramValues, readParams, stageFor } from "./panel.ts";

const MODULE = `// a delay
plugin({ type: "internal", name: "delay" })
params({ Time: 0.5, "Wet Level": -3.0, Mode: "ping" })
`;

describe("the footer a folder gets", () => {
  it("is an effect's three, an instrument's one, and nothing for an io module", () => {
    expect(stageFor("modules/audio/d/module.k7x", null)).toEqual([
      "@input-volume",
      "@mix",
      "@output-volume",
    ]);
    expect(
      stageFor("_libraries/x/_modules/instruments/s/module.k7x", null),
    ).toEqual(["@output-volume"]);
    expect(stageFor("modules/audio/d/module.k7x", "in")).toEqual([]);
    expect(stageFor("modules/audiox/d/module.k7x", null)).toEqual([]);
  });
});

describe("the file's own params", () => {
  it("reads them as the text they are written in, and as numbers where they are one", () => {
    expect(readParams(MODULE)).toEqual({
      Time: "0.5",
      "Wet Level": "-3.0",
      Mode: "ping",
    });
    expect(paramValues(MODULE)).toEqual({ Time: 0.5, "Wet Level": -3 });
  });
});

describe("the rows", () => {
  it("union every source once, the most specific range winning, the engine's own binds on an effect", () => {
    const rows = paramRows({
      content: MODULE,
      bound: ["Time"],
      path: "modules/audio/d/module.k7x",
      hintParams: ["Time", "Wobble"],
      hintMeta: { Wobble: { min: 0, max: 1 }, Time: { min: 0, max: 9 } },
      pluginNames: ["Wobble"],
      pluginMeta: { Time: { min: 0, max: 2 } },
      scannedNames: [],
      scannedMeta: {},
      baseBinds: [],
    });
    const by = Object.fromEntries(rows.map((r) => [r.name, r]));
    expect(rows.map((r) => r.name).filter((n) => n === "Time")).toHaveLength(1);
    expect(by.Time).toMatchObject({
      bound: true,
      value: "0.5",
      meta: { min: 0, max: 2 },
    });
    expect(by.Wobble.meta).toEqual({ min: 0, max: 1 });
    // The footer an effect has, and a lane's intrinsics, are offered too.
    for (const n of [
      "@mix",
      "@input-volume",
      "@output-volume",
      "@pan",
      "@mute",
    ])
      expect(by[n]).toBeDefined();
    expect(by["@mix"].meta).toEqual({ min: 0, max: 1, def: 1 });
    // The built-in the file names knows its own params without any engine.
    expect(rows.length).toBeGreaterThan(8);
  });

  it("offers an io module its own settings and reads one the file states at its top level", () => {
    const rows = paramRows({
      content: 'io("in")\nmonitor: auto\n',
      bound: [],
      io: "in",
      path: "modules/io/in/module.k7x",
      hintParams: [],
      hintMeta: {},
      pluginNames: [],
      scannedNames: [],
      scannedMeta: {},
      baseBinds: [],
    });
    const monitor = rows.find((r) => r.name === "@monitor");
    expect(monitor?.value).toBe("auto");
    expect(rows.find((r) => r.name === "@gain")?.meta).toEqual({
      min: -24,
      max: 24,
    });
    expect(rows.find((r) => r.name === "@mix")).toBeUndefined();
  });
});
