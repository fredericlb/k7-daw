// validateModule.test.ts — a module, its panel and a lane's events judged by the Rust readers the compile uses.
// A reader would get wrong that the reader answers its FIRST finding only: a case names the one it provokes.

import { describe, expect, it } from "vitest";
import { validateFile } from "./validate.ts";

const M = (body: string) => body.replace(/^\n/, "");

/** The editor's check of a module file's text. */
const judge = (text: string) =>
  validateFile("modules/audio/x/module.k7x", text);

/** The editor's check of a `ui.k7x`'s text. */
const judgePanel = (text: string) =>
  validateFile("modules/audio/x/ui.k7x", text);

/** The editor's check of a lane's node file. */
const judgeLane = (text: string) => validateFile("nodes/lane.k7x", text);

describe("a module", () => {
  it("accepts a module the app actually writes", () => {
    const v = judge(
      M(`
plugin({ type: "internal", name: "Chorus" })
ui({
  cols: 4,
  rows: 2,
  widgets: { w1: { type: "knob", col: 0, row: 0, label: "Depth", bind: "Depth", min: 0, max: 1 } },
})
`),
    );
    expect(v.issues).toEqual([]);
  });

  it("reads an empty file as an empty module", () => {
    expect(judge("").issues).toEqual([]);
  });

  it("reads a file that extends and states no type", () => {
    expect(judge('extends("@workspace/instruments/kit")\n').issues).toEqual([]);
  });

  it("refuses a `label` — a module is shown by its file name, and there is no second one", () => {
    const { ok, issues } = judge(
      'label("Grande salle")\nplugin({ type: "internal", name: "Reverb" })\n',
    );
    expect(ok).toBe(false);
    expect(issues[0].message).toMatch(/label/);
  });

  it("catches the typo a closed struct is for, and says where", () => {
    const { ok, issues } = judge('plugin({ type: "internal" })\nparamz({})\n');
    expect(ok).toBe(false);
    expect(issues[0].message).toMatch(/paramz/);
    expect(issues[0]).toMatchObject({ line: 2, col: 1 });
  });

  it("reports a syntax error at its line and column", () => {
    const { malformed, issues } = judge('plugin({ type: "internal" })\nui(\n');
    expect(malformed).toBe(true);
    expect(issues).toHaveLength(1);
    expect(issues[0].path).toBe("");
    expect(issues[0].line).toBeGreaterThanOrEqual(2);
  });

  it("reports a widget field that belongs to another type, on its line", () => {
    const { issues } = judge(
      M(`
ui({
  cols: 1,
  rows: 1,
  widgets: {
    w2: { type: "toggle", col: 0, row: 0, bind: "Bypass", min: 0 },
  },
})
`),
    );
    expect(issues).toHaveLength(1);
    expect(issues[0].path).toBe("ui.widgets.w2.min");
    expect(issues[0].message).toMatch(/range/);
    expect(issues[0].line).toBe(5);
  });

  it("accepts the asset dropdown that started all this", () => {
    const { issues } = judge(
      M(`
params({ model: "jcm800.nam" })
ui({
  cols: 4,
  rows: 2,
  widgets: { w2: { type: "dropdown", col: 0, row: 0, bind: "model", library: "model" } },
})
`),
    );
    expect(issues).toEqual([]);
  });

  it("tells a slice rack from a samples one", () => {
    const slice = judge(
      M(`
type("rack")
rack({ rack_type: "slice", source: "break.wav", symbols: { BD: { note: 36, from: 0, to: 0.5 } } })
`),
    );
    expect(slice.issues).toEqual([]);
    const bad = judge(
      M(`
type("rack")
rack({ rack_type: "slice", symbols: { BD: { note: 36, from: 0, to: 0.5 } } })
`),
    );
    expect(bad.ok).toBe(false);
  });

  it("refuses pads written outside `symbols:` rather than reading an empty kit", () => {
    const { ok, issues } = judge(
      `type("rack")\nrack({ Kick: { note: 36, sample: "@k7-core/k.wav" } })\n`,
    );
    expect(ok).toBe(false);
    expect(issues.map((i) => i.message).join("\n")).toMatch(/Kick/);
  });

  it.each(["rack(null)\n", "rack({})\n", "rack({ symbols: null })\n"])(
    "reads an empty rack as a kit with no pads of its own: %j",
    (rack) => {
      expect(judge(`type("rack")\n${rack}`).issues).toEqual([]);
    },
  );

  it("keeps a note name and a param name with spaces", () => {
    const { issues } = judge(
      M(`
params({ "Filter Freq": 69, "Transpose - Bypass": 1 })
content("C4")
`),
    );
    expect(issues).toEqual([]);
  });
});

// Tabs are the PANEL's, not a widget: an ordered list of named pages beside the widgets every tab shows.
describe("a panel's tabs", () => {
  const panel = (tabs: unknown) =>
    `cols(4)\nrows(2)\nwidgets({})\ntabs(${JSON.stringify(tabs)})\n`;
  const page = (name: string) => ({ name, widgets: {} });

  it("takes an ordered list of named pages", () => {
    expect(judgePanel(panel([page("Osc"), page("Filter")])).ok).toBe(true);
  });

  it("refuses a name past ten characters, and two tabs of one name", () => {
    expect(judgePanel(panel([page("Oscillators")])).ok).toBe(false);
    expect(judgePanel(panel([page("Osc"), page("Osc")])).ok).toBe(false);
  });

  it("has no `tabs` WIDGET any more — a page is not something placed on the grid", () => {
    const old = `cols(4)\nrows(2)\nwidgets({ t: { type: "tabs", col: 0, row: 0 } })\n`;
    expect(judgePanel(old).ok).toBe(false);
  });
});

describe("project.layers", () => {
  const root = (layers: string) =>
    validateFile(
      "project.k7x",
      `project({ name: "s", tempo: 120, layers: ${layers} })\n`,
    );

  it("is accepted empty, and read by nothing", () => {
    expect(root("[]").ok).toBe(true);
  });

  it("is refused naming a lane, saying what draws the picture now", () => {
    const r = root('["main"]');
    expect(r.ok).toBe(false);
    expect(r.issues[0]?.message).toMatch(/trackOrder/);
  });
});

// One event schema for sound and shots: each vocabulary accepted, and the rules a shot carries.
describe("an event", () => {
  const lane = (event: string) =>
    judgeLane(`extends("@meta/picture-track")\nevents([${event}])\n`);
  const ok = (event: string) => lane(event).ok;

  it("takes an audio event's words and a shot's", () => {
    expect(ok('{ ref: "riff", start: "3:1", fadeIn: "0:1", loop: 2 }')).toBe(
      true,
    );
    expect(
      ok(
        '{ ref: "shot", start: 4, length: 3, from: 1.5, speed: 2, fadeIn: 0.5, fadeOut: { over: 1, with: "wipe" }, animate: { x: [{ t: 0, v: 0 }] }, silent: true }',
      ),
    ).toBe(true);
  });

  it("takes no ref only as an activation, with nothing that needs a source", () => {
    expect(ok("{ start: 2, length: 4 }")).toBe(true);
    expect(lane("{ start: 2, from: 1 }").issues[0]?.message).toMatch(
      /ACTIVATION/,
    );
  });

  it("refuses a backwards window and a reversed shot that runs out of source", () => {
    expect(lane('{ ref: "a", from: 2, to: 1 }').issues[0]?.message).toMatch(
      /greater than 'from'/,
    );
    expect(
      lane('{ ref: "a", from: 1, speed: -1, length: 3 }').issues[0]?.message,
    ).toMatch(/runs out of source/);
  });

  it("still refuses a key neither side has", () => {
    expect(ok('{ ref: "a", source_from: 1 }')).toBe(false);
  });

  it("starts at a timecode or a non-negative number", () => {
    expect(ok('{ ref: "a", start: 4.5 }')).toBe(true);
    expect(ok('{ ref: "a", start: "3:1" }')).toBe(true);
    expect(ok('{ ref: "a", start: -1 }')).toBe(false);
    expect(ok('{ ref: "a", start: "4.5s" }')).toBe(false);
  });

  it("takes any finite speed on a shot, backwards included, and none on an activation", () => {
    expect(ok('{ ref: "a", start: 0, length: 2, speed: 0.5 }')).toBe(true);
    expect(ok('{ ref: "a", start: 0, length: 2, speed: 0 }')).toBe(true);
    expect(ok('{ ref: "a", start: 0, length: 2, from: 3, speed: -1 }')).toBe(
      true,
    );
    expect(ok('{ ref: "a", start: 0, length: 4, from: 3, speed: -1 }')).toBe(
      false,
    );
    expect(ok("{ start: 0, length: 2, speed: 2 }")).toBe(false);
  });
});
