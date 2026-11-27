// curve.test.ts — a curve's picture through the module, the handle's healing, the errors, the vocabulary and the spec's type.

import type { CurveSpec as GraphCurveSpec } from "@studio/graph-model";
import {
  ADSR_DEFAULT_LEVEL,
  CURVE_CUTS,
  LFO_SHAPES,
} from "@studio/k7-wasm/consts";
import { beforeAll, describe, expect, expectTypeOf, it } from "vitest";
import { call, K7WasmError, wasmReady } from "./abi.ts";
import { CurveHandle, curvePreview } from "./curve.ts";
import type { CurveSpec } from "./types/index.ts";

beforeAll(async () => {
  expect(await wasmReady()).toBe(true);
});

const lfo: CurveSpec = {
  type: "lfo",
  polarity: "unipolar",
  amplitude: 1,
  shape: "saw",
  frequency: 1,
  phase: 0,
};

const adsr: CurveSpec = {
  type: "adsr",
  polarity: "unipolar",
  amplitude: 1,
  attack: 1,
  decay: 1,
  sustain: 1,
  release: 1,
  level: 1,
};

function refused(run: () => unknown): K7WasmError {
  try {
    run();
  } catch (e) {
    expect(e).toBeInstanceOf(K7WasmError);
    return e as K7WasmError;
  }
  throw new Error("expected the call to be refused");
}

describe("a curve's picture", () => {
  it("spans a cycle and a quarter of the next for an endless oscillator, the quarter not solid", () => {
    expect(curvePreview(lfo, 4, 4).span).toEqual({ beats: 5, solid: 4 });
  });

  it("spans the whole duration when it has one, all of it solid", () => {
    const spec: CurveSpec = { ...lfo, duration: "2:0" };
    expect(curvePreview(spec, 4, 4).span).toEqual({ beats: 8, solid: 8 });
  });

  it("samples the output across the span, wrapping into the next cycle", () => {
    const { solid, faded } = curvePreview(lfo, 4, 4, { n: 10 });
    // A saw at one cycle a bar: 0.4 of the span is half a cycle; the last sample is a quarter into the next.
    expect(solid[4].y).toBeCloseTo(0.5);
    expect(faded[faded.length - 1].y).toBeCloseTo(0.25);
  });

  it("puts a saw's drop in the solid part however coarse the sampling, the boundary in both", () => {
    const saw: CurveSpec = {
      ...lfo,
      amplitude: 0.8,
      cut: "positive",
      frequency: 0.5,
    };
    // 48 samples never land on the boundary at 0.8: the solid part must still end at it, on the wrapped value.
    const { solid, faded } = curvePreview(saw, 4, 4, { n: 48 });
    const end = solid[solid.length - 1];
    expect(end.x).toBeCloseTo(0.8);
    expect(end.y).toBeCloseTo(0);
    expect(solid[solid.length - 2].y).toBeGreaterThan(0.7);
    expect(faded[0]).toEqual(end);
  });

  it("draws a line per beat across the span, a bar on every fourth in 4/4, and none for an envelope", () => {
    const sine: CurveSpec = { ...lfo, shape: "sine" };
    const lines = curvePreview(sine, 4, 4).beats;
    expect(lines.map((l) => l.bar)).toEqual([
      true,
      false,
      false,
      false,
      true,
      false,
    ]);
    expect(lines[5].x).toBeCloseTo(1);
    expect(curvePreview(adsr, 4, 4).beats).toEqual([]);
  });

  it("draws an envelope over its own length, with no span and no playhead", () => {
    const picture = curvePreview(adsr, 4, 4, { playhead: 3 });
    expect(picture.span).toEqual({ beats: 0, solid: 0 });
    expect(picture.faded).toEqual([]);
    expect(picture.playhead).toBeNull();
  });

  it("answers the playhead in the same call when asked", () => {
    expect(curvePreview(lfo, 4, 4).playhead).toBeNull();
    // Bar 3 and a half: half a cycle in, at x 2/5 of a span of five beats.
    const at = curvePreview(lfo, 4, 4, { playhead: 10 }).playhead;
    expect(at?.x).toBeCloseTo(0.4);
    expect(at?.y).toBeCloseTo(0.5);
  });

  it("ignores the fields of a spec it does not know", () => {
    const withConditions = {
      ...lfo,
      conditions: [{ type: "start", at: "1:1" }],
    } as GraphCurveSpec;
    expect(curvePreview(withConditions, 4, 4)).toEqual(curvePreview(lfo, 4, 4));
  });
});

describe("a curve handle", () => {
  it("draws the same picture as a single call, the same object on every read", () => {
    const h = new CurveHandle(lfo, 4, 4);
    expect(h.preview).toEqual(curvePreview(lfo, 4, 4));
    expect(h.preview).toBe(h.preview);
  });

  it("folds the song into an endless oscillator's cycle, holds a finite one at its end", () => {
    const at = new CurveHandle(lfo, 4, 4).playhead(10);
    expect(at?.x).toBeCloseTo(0.4);
    expect(at?.y).toBeCloseTo(0.5);
    const finite = new CurveHandle({ ...lfo, duration: "1:0" }, 4, 4);
    expect(finite.playhead(30)?.x).toBe(1);
  });

  it("has no playhead for an envelope or a beat before the start", () => {
    expect(new CurveHandle(adsr, 4, 4).playhead(3)).toBeNull();
    expect(new CurveHandle(lfo, 4, 4).playhead(-1)).toBeNull();
  });

  it("opens itself again when it is used after close()", () => {
    const h = new CurveHandle(lfo, 4, 4);
    const before = h.playhead(10);
    h.close();
    h.close();
    expect(h.playhead(10)).toEqual(before);
  });

  it("opens itself again when its slot was evicted", () => {
    const first = new CurveHandle(lfo, 4, 4);
    const before = first.playhead(10);
    const crowd = Array.from({ length: 300 }, () => new CurveHandle(lfo, 4, 4));
    for (const h of crowd) h.playhead(1);
    expect(first.playhead(10)).toEqual(before);
    for (const h of crowd) h.close();
    first.close();
  });

  it("refuses a spec the module cannot resolve, on the first read", () => {
    const bad: CurveSpec = {
      type: "custom",
      polarity: "unipolar",
      amplitude: 1,
      points: [{ time: "x", value: 1 }],
    };
    expect(refused(() => new CurveHandle(bad, 4, 4).preview).code).toBe(
      "bad_timecode",
    );
  });
});

describe("what the module refuses", () => {
  it("answers a bad timecode with the message the parser throws", () => {
    const bad: CurveSpec = {
      type: "custom",
      polarity: "unipolar",
      amplitude: 1,
      points: [{ time: "1:a", value: 1 }],
    };
    const e = refused(() => curvePreview(bad, 4, 4));
    expect(e.code).toBe("bad_timecode");
    expect(e.message).toBe(
      `bad timecode '1:a' — expected "mesure:noire:croche:doublecroche", optionally ".ticks"`,
    );
  });

  it("refuses a sample count outside 1..4096", () => {
    expect(refused(() => curvePreview(lfo, 4, 4, { n: 0 })).code).toBe(
      "bad_request",
    );
    expect(refused(() => curvePreview(lfo, 4, 4, { n: 4097 })).code).toBe(
      "bad_request",
    );
    expect(curvePreview(lfo, 4, 4, { n: 4096 }).solid.length).toBeGreaterThan(
      0,
    );
  });

  it("refuses a time signature with a zero or a fraction in it", () => {
    expect(refused(() => curvePreview(lfo, 4, 0)).code).toBe("bad_request");
    expect(refused(() => curvePreview(lfo, 0, 4)).code).toBe("bad_request");
    expect(refused(() => curvePreview(lfo, 4.5, 4)).code).toBe("bad_request");
  });

  it("refuses a request that is not one", () => {
    const e = refused(() => call({ op: "curve.nope" } as never));
    expect(e.code).toBe("bad_request");
  });

  it("reads and writes timecodes", () => {
    const args = { num: 4, den: 4, position: true };
    expect(call({ op: "timecode.parse", args: { tc: "2:1", ...args } })).toBe(
      4,
    );
    expect(call({ op: "timecode.format", args: { beats: 4, ...args } })).toBe(
      "2:1",
    );
  });
});

describe("the vocabulary", () => {
  it("is accepted by the module, every shape and every cut", () => {
    for (const shape of LFO_SHAPES)
      expect(
        curvePreview({ ...lfo, shape }, 4, 4).solid.length,
      ).toBeGreaterThan(0);
    for (const cut of CURVE_CUTS)
      expect(curvePreview({ ...lfo, cut }, 4, 4).solid.length).toBeGreaterThan(
        0,
      );
    const level = ADSR_DEFAULT_LEVEL;
    expect(
      curvePreview({ ...adsr, level } as CurveSpec, 4, 4).solid.length,
    ).toBeGreaterThan(0);
  });
});

describe("the spec's type", () => {
  it("is what @studio/graph-model's CurveSpec already is, so call sites pass the value they have", () => {
    expectTypeOf<GraphCurveSpec>().toExtend<CurveSpec>();
  });
});
