// schemaPage.test.ts — the rules the page asks of a name, a plugin, a bind and a level, each answered by the Rust crates.
// A reader would get wrong that the supervisor's level is a second copy: it cannot link the crates, so a test holds it equal.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { beforeAll, describe, expect, it } from "vitest";
import { wasmReady } from "./abi.ts";
import { LIBRARY_LEVEL, RACK_ADD_BIND, RACK_BIND } from "./consts.generated.ts";
import { rackAddBind, rackAddBindAt, rackBindSymbol } from "./rackBinds.ts";
import {
  builtinPluginFor,
  CAPTION_SOURCE,
  docPattern,
  IMAGE_SOURCE,
  isPresetName,
  placementStartOf,
  VIDEO_SOURCE,
} from "./schema.ts";

beforeAll(async () => {
  expect(await wasmReady()).toBe(true);
});

describe("docPattern", () => {
  it("builds a pattern around the extension", () => {
    const re = docPattern("(^|/)curves/[^/]+");
    expect(re.test("p/curves/a.k7x")).toBe(true);
    expect(re.test("curves/a.K7X")).toBe(true);
    expect(re.test("curves/a.yaml")).toBe(false);
    expect(re.test("p/curves/x/a.k7x")).toBe(false);
    expect(docPattern("^clips/[^/]+", "").test("clips/a.K7X")).toBe(false);
  });
});

describe("a clip's source", () => {
  it("is a picture, a still or captions by its extension", () => {
    expect(VIDEO_SOURCE.test("a/b.MP4")).toBe(true);
    expect(IMAGE_SOURCE.test("a.jpeg")).toBe(true);
    expect(CAPTION_SOURCE.test("a.vtt")).toBe(true);
    expect(VIDEO_SOURCE.test("a.wav")).toBe(false);
  });
});

describe("a placement id", () => {
  it("names its start, a repeat's `#n` aside", () => {
    expect(placementStartOf("riff@16.5")).toBe(16.5);
    expect(placementStartOf("verse@8>kick@0#3")).toBe(0);
    expect(placementStartOf("riff")).toBeNaN();
  });
});

describe("a preset name", () => {
  it("is held to what every filesystem agrees on", () => {
    expect(isPresetName("Warm pad 2")).toBe(true);
    expect(isPresetName("")).toBe(false);
    expect(isPresetName("-lead")).toBe(false);
    expect(isPresetName("a/b")).toBe(false);
  });
});

describe("a built-in plugin", () => {
  it("is found by any spelling of its name, and only when it is internal", () => {
    expect(
      builtinPluginFor({ type: "internal", name: "4OSC" })?.params.length,
    ).toBeGreaterThan(0);
    expect(builtinPluginFor({ type: "vst3", name: "4OSC" })).toBeUndefined();
    expect(builtinPluginFor(undefined)).toBeUndefined();
  });
});

describe("a rack bind", () => {
  it("names a pad, or adds one on a note", () => {
    expect(rackBindSymbol(`${RACK_BIND}BD`)).toBe("BD");
    expect(rackBindSymbol("Mix")).toBeNull();
    expect(rackAddBind(RACK_ADD_BIND)).toEqual({ note: null });
    expect(rackAddBind(rackAddBindAt(40))).toEqual({ note: 40 });
    expect(rackAddBind(`${RACK_ADD_BIND}200`)).toBeNull();
    expect(rackAddBind("@rack:BD")).toBeNull();
  });
});

describe("LIBRARY_LEVEL", () => {
  it("is the level the supervisor reports", () => {
    const supervisor = join(
      dirname(fileURLToPath(import.meta.url)),
      "../../../../k7-daw-backend/packages/k7-supervisor/src/parts/registry/state.rs",
    );
    const m = /const LIBRARY_LEVEL: i64 = (\d+);/.exec(
      readFileSync(supervisor, "utf8"),
    );
    expect(Number(m?.[1])).toBe(LIBRARY_LEVEL);
  });
});
