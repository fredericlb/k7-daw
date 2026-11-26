import { describe, expect, it } from "vitest";
import { fieldsAt, fileKindFor, valuesAt } from "./fields.ts";

const PATHS = {
  module: "modules/audio/cab/module.k7x",
  chains: "chains.k7x",
  chain: "song/chains/gtr.k7x",
  curve: "curves/lfo.k7x",
} as const;
const fields = (kind: keyof typeof PATHS, at: string[]) =>
  fieldsAt(PATHS[kind], at);
const values = (kind: keyof typeof PATHS, at: string[], field: string) =>
  valuesAt(PATHS[kind], at, field);

// These tests pin the completions the editor derives from the Rust readers — see apps/k7-daw-frontend/packages/song-schema/docs/fields.md.

describe("derived module fields", () => {
  it("matches the hand-kept table, plus the field the table was missing", () => {
    // Every module field, in the order the completions offer them (docs/fields.md).
    expect(fields("module", [])).toEqual([
      "extends",
      "plugin",
      "io",
      // A module's OWN ports, overriding its type folder's defaults — see docs/ports.md.
      "inputs",
      "outputs",
      "type",
      // Which GLSL program a shader node runs, out of its own `shaders/` — a FIELD, not a param.
      "shader",
      // …and which primitive a shape module draws, and the family a `text` one is set in.
      "shape",
      "font",
      // …and how a text shape's lines are set — apps/k7-daw-wasm/verbs/docs/picture/titles.md.
      "align",
      "background",
      "shadow",
      "params",
      "rack",
      // An SFZ drum kit's pads, name → note, drawn as a rack's taps — apps/k7-daw-frontend/packages/song/docs/racks.md.
      "tap_outs",
      "kit",
      // A COLD module works only on audio already on disk — see docs/coldModules.md.
      "cold",
      // …and where the model a file-valued param names can be downloaded from — apps/k7-daw-frontend/packages/song/docs/moduleAssets.md.
      "model_url",
      // `ir`, `nam` and `jsdsp` carry no field of their own, so there is nothing to complete against (docs/fields.md).
      "channel",
      // …and WHICH lens an io "camera" opens, plus where in the viewport it draws — docs/camera.md.
      "camera",
      // …and WHICH MIDI device an io "midi" records from — apps/k7-daw-frontend/packages/song/docs/midiIn.md.
      "device",
      "frame",
      // No `track`: where an input LANDS is the chain's business (docs/fields.md).
      "source", // …and the OTHER kind of source: an io "file" reads audio off disk
      "monitor",
      "gain", // the other thing an input does to its signal, beside whether you hear it
      "mix", // the engine's dry/wet around an effect — apps/k7-daw-engine/k7-runtime/docs/mix.md
      "input_volume", // …and its levels on the same stage
      "output_volume",
      // The named .vstpreset a module starts from, out of its own `base/` — see docs/userPresets.md.
      "base",
      "ui",
      "automations",
      // No `docs` or `todos`: no panel reads them — see docs/docModules.md.
      "content",
      // …and a LANE's own fields, a lane being a module rather than a second entity (docs/trackModules.md).
      "events",
      "color",
      "disabled",
      "tracks",
      "strudel",
      // The PICTURE lane a lane plays the sound of — apps/k7-daw-frontend/packages/song/docs/videoSound.md.
      "video_source",
      // A shader's 3D LUT — apps/k7-daw-wasm/verbs/docs/picture/effects.md.
      "lut",
      // FROZEN at this module: the render standing in for everything upstream of it — see docs/freeze.md.
      "frozen",
      "engine",
      "engines",
      // …and the module's other attribute SETS, last because they are ABOUT the fields above (docs/fields.md).
      "alternatives",
      "alternative",
      // …and how the COMPLETIONS should treat this module, outside MODULE_FIELDS so an alternative cannot restate it.
      "status",
    ]);
  });

  it("answers for a nested struct, which the CUE version could not", () => {
    // A nested struct answers from its own shape, however deep it sits (docs/fields.md).
    expect(fields("module", ["ui"])).toEqual([
      "cols",
      "rows",
      "widgets",
      "tabs",
      "vertical",
    ]);
    // A tab is a name and its widgets, and a widget in it is a widget like any other.
    expect(fields("module", ["ui", "tabs", "0"])).toEqual(["name", "widgets"]);
    expect(fields("module", ["ui", "tabs", "0", "widgets", "w1"])).toContain(
      "bind",
    );
    expect(fields("module", ["ui", "widgets", "w1"])).toContain("library");
    expect(fields("module", ["ui", "widgets", "w1"])).toContain("bind");
    expect(fields("module", ["rack"])).toEqual(
      expect.arrayContaining(["rack_type", "source", "symbols"]),
    );
    expect(fields("module", ["rack", "symbols", "BD"])).toEqual([
      "note",
      "sample",
      "clip",
      "from",
      "to",
      "mute",
    ]);
  });

  it("offers nothing inside an open map", () => {
    // `params` is a free map and `plugin` is deliberately open; offering names there would invent a vocabulary.
    expect(fields("module", ["params"])).toEqual([]);
    expect(fields("module", ["plugin"])).toEqual([]);
  });
});

describe("derived lane fields", () => {
  it("has no kind of its own — a lane is a module file", () => {
    // No `track` FileKind and no per-kind narrowing: a lane file answers as a module — see docs/fields.md.
    expect(fileKindFor("nodes/melody.k7x")).toBe("module");
    // …and so does a chain project's `tracks/<lane>.k7x`, the lane's own file there.
    expect(fileKindFor("tracks/melody.k7x")).toBe("module");
    expect(fields("module", [])).toEqual(
      expect.arrayContaining(["events", "tracks", "strudel", "color"]),
    );
  });

  it("answers inside an ALTERNATIVE with the module's own fields", () => {
    // A set is a module partial plus a name — derived, so the two cannot drift.
    const inside = fields("module", ["alternatives", "0"]);
    expect(inside).toContain("name");
    expect(inside).toContain("params");
    expect(inside).toContain("events");
    // A set may name a module of its own — it is then another module entirely (apps/k7-daw-frontend/packages/song/docs/alternatives.md).
    expect(inside).toContain("extends");
    expect(inside).not.toContain("alternatives");
    // The list itself has no field vocabulary — its entries are positions, not names.
    expect(fields("module", ["alternatives"])).toEqual([]);
  });

  it("answers inside a placement", () => {
    expect(fields("module", ["events"])).toEqual([
      "ref",
      "start",
      // The event's own window — the placement-level half of the material's pair.
      "from",
      "to",
      "fadeIn",
      "fadeOut",
      "loop",
      "gain",
      "disabled",
      // Whether this event plays the material's stretch map — `event status warp` writes it.
      "warp",
      // Its other performances and which plays where — the takes view writes them (song docs/takes.md).
      "takes",
      "comp",
      "crossfades",
      // A SHOT's own, offered on every lane since only the compile knows a ref's material.
      "length",
      "animate",
      "speed",
      "effect",
      "silent",
      "pitch",
    ]);
  });
});

describe("derived values", () => {
  it("offers a closed set where the schema has one", () => {
    expect(values("module", [], "type")).toEqual([
      "plugin",
      "rack",
      "video_effect",
      "video_transition",
    ]);
    expect(values("module", [], "monitor")).toEqual(["off", "auto", "on"]);
    // Both DERIVED from the schema's own constants — this table had already fallen two values
    // behind `io:` before they were, which is the drift the two `as const`s now prevent.
    expect(values("module", [], "io")).toEqual([
      "out",
      "in",
      "video_out",
      "file",
      "camera",
      "midi",
    ]);
    // …and the two live taps a waveform draws.
    expect(values("module", ["ui", "widgets", "w1"], "source")).toEqual([
      "input",
      "output",
    ]);
    expect(values("module", ["ui", "widgets", "w1"], "type")).toContain(
      "dropdown",
    );
  });

  it("offers nothing where the value is free text", () => {
    // A param name, a module name, a docs string — a list there would read as the only allowed values.
    expect(values("module", [], "extends")).toEqual([]);
    expect(values("module", [], "docs")).toEqual([]);
  });
});

describe("chain fields", () => {
  it("offers the file's five sections, and nothing inside them", () => {
    // `bus_only:` is NOT a synonym of `bypassed:` — see apps/k7-daw-frontend/packages/song/docs/busOnly.md.
    expect(fields("chains", [])).toEqual([
      "modules",
      "bypassed",
      "bus_only",
      "tap_feeds",
      "positions",
    ]);
    // Inside a section the keys are node NAMES, a project's own vocabulary the schema cannot know.
    expect(fields("chains", ["modules"])).toEqual([]);
  });

  it("offers every curve type the file format takes", () => {
    expect(values("curve", [], "type")).toEqual([
      "lfo",
      "custom",
      "adsr",
      "onoff",
    ]);
    expect(fields("curve", [])).toEqual(
      expect.arrayContaining(["type", "points", "attack"]),
    );
  });
});

describe("the kind a path names", () => {
  it("is the kind of file its folder and name say", () => {
    expect(fileKindFor("song/nodes/pad.k7x")).toBe("module");
    expect(fileKindFor("song/project.k7x")).toBe("project");
    expect(fileKindFor("song/chains/main.k7x")).toBe("chain");
    expect(fileKindFor("song/notes.txt")).toBeNull();
    expect(fileKindFor(undefined)).toBeNull();
  });
});

describe("chain project files", () => {
  it("names a chain file, a lane file and the older segments files", () => {
    expect(fileKindFor("chains/gtr.k7x")).toBe("chain");
    expect(fileKindFor("tracks/gtr.k7x")).toBe("module");
    // A picture lane is a lane node, its fields the module's.
    expect(fileKindFor("nodes/cam.k7x")).toBe("module");
    expect(fileKindFor("chains.k7x")).toBe("chains");
    expect(fileKindFor("chains/verb/graph.k7x")).toBe("chains");
    expect(fileKindFor("chains/gtr.yaml")).toBeNull();
    expect(fileKindFor("project.yaml")).toBeNull();
  });

  it("offers a chain's keys, then a node's, a link's and a module's", () => {
    expect(fields("chain", [])).toEqual([
      "start",
      "ports",
      "bus_only",
      "hidden",
      "input",
      "steps",
      "subchains",
    ]);
    expect(fields("chain", ["subchains"])).toEqual(["id", "steps"]);
    expect(fields("chain", ["subchains", "steps"])).toEqual(
      fields("chain", ["steps"]).filter((f) => f !== "track" && f !== "sub"),
    );
    expect(fields("chain", ["subchains", "steps", "to"])).toEqual([
      "chain",
      "level",
      "port",
    ]);
    expect(fields("chain", ["steps"])).toEqual(
      expect.arrayContaining([
        "id",
        "track",
        "to",
        "bypass",
        "extends",
        "params",
      ]),
    );
    expect(fields("chain", ["steps", "to"])).toEqual([
      "chain",
      "level",
      "port",
    ]);
    expect(fields("chain", ["steps", "events"])).toEqual(
      fields("module", ["events"]),
    );
    expect(values("chain", [], "start")).toEqual(["true", "MIX", "RENDER"]);
    expect(values("chain", ["steps"], "io")).toEqual(
      values("module", [], "io"),
    );
  });
});
