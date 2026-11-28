// curve.test.ts — a curve handle through the module: its picture, its healing after close or eviction, its errors and the spec's type.
// The picture's rules (span, samples, lines, playhead, vocabulary, refusals) are song-schema's and k7-wasm's own tests.

import type { CurveSpec as GraphCurveSpec } from "@studio/graph-model";
import { beforeAll, describe, expect, expectTypeOf, it } from "vitest";
import { K7WasmError, wasmReady } from "./abi.ts";
import { CurveHandle } from "./curve.ts";
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

describe("a curve handle", () => {
  it("draws the picture the module answers, the same object on every read", () => {
    const h = new CurveHandle(lfo, 4, 4);
    expect(h.preview.span).toEqual({ beats: 5, solid: 4 });
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

describe("the spec's type", () => {
  it("is what @studio/graph-model's CurveSpec already is, so call sites pass the value they have", () => {
    expectTypeOf<GraphCurveSpec>().toExtend<CurveSpec>();
  });
});
