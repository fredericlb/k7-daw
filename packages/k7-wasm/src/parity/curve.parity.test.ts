// curve.parity.test.ts — the Rust curves and timecodes against the frozen TypeScript, over a seeded random sweep.
// A reader would get wrong that it is a working-port check: x and y agree to 1e-9, never to the bit; the seed prints on failure.

import type { CurveSpec as GraphCurveSpec } from "@studio/graph-model";
import { beforeAll, describe, expect, it } from "vitest";
import { scaled } from "../../../../scripts/vitestScale.ts";
import { call, K7WasmError, wasmReady } from "../abi.ts";
import { CurveHandle, curvePreview } from "../curve.ts";
import {
  previewBeats,
  previewParts,
  previewPlayhead,
  previewSpan,
} from "./curveMath.old.ts";
import {
  beatsToDuration,
  beatsToPosition,
  parseDuration,
  parsePosition,
} from "./timecode.old.ts";

const SEED = 0x6b7e57;
// 300 specs cover every kind and signature; K7_PARITY_SPECS=2000 is the opt-in wide sweep.
const SPECS = Number(process.env.K7_PARITY_SPECS) || 300;
const TIMECODES = 500;
const SIGS: [number, number][] = [
  [4, 4],
  [3, 4],
  [6, 8],
  [7, 8],
  [5, 4],
  [2, 2],
  [12, 8],
];
const XY_TOLERANCE = 1e-9;
const SPAN_TOLERANCE = 1e-12;

beforeAll(async () => {
  expect(await wasmReady()).toBe(true);
});

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** A tiny toolkit over one generator, so a failing index reproduces from the seed alone. */
function toolkit(rand: () => number) {
  const int = (lo: number, hi: number) =>
    lo + Math.floor(rand() * (hi - lo + 1));
  const pick = <T>(xs: readonly T[]): T => xs[int(0, xs.length - 1)];
  const num = (lo: number, hi: number) => lo + rand() * (hi - lo);
  /** A timecode of 1 to 5 fields, sometimes with ticks, zero-padded or spaced. */
  const timecode = (zeroBased: boolean): string => {
    const fields = Array.from({ length: int(1, 5) }, () =>
      String(zeroBased ? int(0, 12) : int(1, 12)),
    );
    let text = fields.map((f) => (rand() < 0.1 ? `0${f}` : f)).join(":");
    if (rand() < 0.3) text += `.${int(0, 1500)}`;
    if (rand() < 0.1) text = ` ${text}\t`;
    return text;
  };
  return { int, pick, num, timecode, rand };
}

type Kit = ReturnType<typeof toolkit>;

function randomSpec(k: Kit): GraphCurveSpec {
  const polarity = k.pick(["unipolar", "bipolar"] as const);
  const amplitude = k.rand() < 0.2 ? 1 : k.num(0, 1.5);
  const cut = k.pick([undefined, "none", "positive", "negative"] as const);
  const base = { polarity, amplitude, ...(cut ? { cut } : {}) };
  const kind = k.pick(["lfo", "custom", "onoff", "adsr"] as const);
  if (kind === "lfo") {
    return {
      ...base,
      type: "lfo",
      shape: k.pick(["sine", "triangle", "saw", "square"] as const),
      // Only [0.05, 16]: below 0 the port draws 1 where the TypeScript drew infinity, and a tiny one has beat lines the port thins.
      frequency:
        k.rand() < 0.3 ? k.pick([0.25, 0.5, 1, 2, 4, 16]) : k.num(0.05, 16),
      phase: k.rand() < 0.3 ? 0 : k.rand(),
      ...(k.rand() < 0.5 ? { duration: k.timecode(true) } : {}),
    };
  }
  if (kind === "adsr") {
    const ms = () => (k.rand() < 0.15 ? 0 : k.num(0, 2000));
    return {
      ...base,
      type: "adsr",
      attack: ms(),
      decay: ms(),
      sustain: ms(),
      release: ms(),
      level: k.rand(),
    };
  }
  const points = Array.from({ length: k.int(1, 12) }, () => ({
    time: k.timecode(false),
    value: kind === "onoff" ? k.int(0, 1) : k.num(-1, 2),
  }));
  // Coincident points: the same time twice, as an editor leaves them.
  if (points.length > 1 && k.rand() < 0.3) points[1].time = points[0].time;
  return { ...base, type: kind, points };
}

let worst = 0;
let comparisons = 0;

function close(got: number, want: number, tolerance: number, what: string) {
  comparisons++;
  const diff = Math.abs(got - want);
  worst = Math.max(worst, diff);
  expect(diff, `${what}: ${got} vs ${want}`).toBeLessThanOrEqual(tolerance);
}

function sameLine(
  got: { x: number; y: number }[],
  want: { x: number; y: number }[],
  what: string,
) {
  expect(got.length, `${what} length`).toBe(want.length);
  comparisons++;
  for (let i = 0; i < want.length; i++) {
    close(got[i].x, want[i].x, XY_TOLERANCE, `${what}[${i}].x`);
    close(got[i].y, want[i].y, XY_TOLERANCE, `${what}[${i}].y`);
  }
}

describe(`curves against the frozen TypeScript, seed 0x${SEED.toString(16)}`, () => {
  it(
    `agrees on ${SPECS} random specs, every picture and playhead`,
    () => {
      const k = toolkit(mulberry32(SEED));
      for (let index = 0; index < SPECS; index++) {
        const spec = randomSpec(k);
        const [num, den] = k.pick(SIGS);
        const n = k.rand() < 0.5 ? 160 : k.int(1, 400);
        const at = (what: string) =>
          `spec ${index} ${what} ${JSON.stringify(spec)} ${num}/${den} n=${n}`;
        const beats = [
          k.num(-5, 5),
          k.num(0, 64),
          k.num(0, 400),
          0,
          k.num(0, 8),
        ];

        const got = curvePreview(spec, num, den, { n, playhead: beats[0] });
        const span = previewSpan(spec, num, den);
        const parts = previewParts(spec, num, den, n);
        const lines = previewBeats(spec, num, den);

        comparisons += 2;
        close(
          got.span.beats,
          span.beats,
          SPAN_TOLERANCE * Math.max(1, Math.abs(span.beats)),
          at("span.beats"),
        );
        close(
          got.span.solid,
          span.solid,
          SPAN_TOLERANCE * Math.max(1, Math.abs(span.solid)),
          at("span.solid"),
        );
        sameLine(got.solid, parts.solid, at("solid"));
        sameLine(got.faded, parts.faded, at("faded"));
        expect(got.beats.length, at("beat lines")).toBe(lines.length);
        for (let i = 0; i < lines.length; i++) {
          expect(got.beats[i].bar, at(`line ${i} bar`)).toBe(lines[i].bar);
          close(got.beats[i].x, lines[i].x, XY_TOLERANCE, at(`line ${i} x`));
        }

        const handle = new CurveHandle(spec, num, den, n);
        expect(handle.preview, at("handle")).toEqual(
          curvePreview(spec, num, den, { n }),
        );
        for (const [j, beat] of beats.entries()) {
          const want = previewPlayhead(spec, beat, num, den);
          const viaHandle = handle.playhead(beat);
          expect(viaHandle === null, at(`playhead ${beat}`)).toBe(
            want === null,
          );
          if (j === 0)
            expect(got.playhead === null, at("playhead in preview")).toBe(
              want === null,
            );
          if (!want || !viaHandle) continue;
          close(viaHandle.x, want.x, XY_TOLERANCE, at(`playhead ${beat} x`));
          close(viaHandle.y, want.y, XY_TOLERANCE, at(`playhead ${beat} y`));
          if (j === 0 && got.playhead) {
            close(
              got.playhead.x,
              want.x,
              XY_TOLERANCE,
              at("preview playhead x"),
            );
            close(
              got.playhead.y,
              want.y,
              XY_TOLERANCE,
              at("preview playhead y"),
            );
          }
        }
        handle.close();
      }
      console.log(
        `curve parity: ${SPECS} specs, ${comparisons} comparisons, worst difference ${worst}`,
      );
    },
    scaled(120_000),
  );
});

/** Random text that is sometimes a timecode and sometimes not. */
function randomTimecodeText(k: Kit): string {
  const r = k.rand();
  if (r < 0.55) return k.timecode(k.rand() < 0.5);
  if (r < 0.65)
    return `${k.pick(["", " ", "﻿", " ", "\t"])}${k.timecode(false)}${k.pick(["", " ", " "])}`;
  const alphabet = [
    "0",
    "1",
    "2",
    "9",
    ":",
    ".",
    "-",
    " ",
    "a",
    "٣",
    "\n",
    "1:1",
    "x",
  ];
  return Array.from({ length: k.int(0, 8) }, () => k.pick(alphabet)).join("");
}

function outcome(run: () => unknown): { ok: unknown } | { error: string } {
  try {
    return { ok: run() };
  } catch (e) {
    return { error: (e as Error).message };
  }
}

describe(`timecodes against the frozen TypeScript, seed 0x${SEED.toString(16)}`, () => {
  it(`agrees on ${TIMECODES} random timecodes, invalid ones included`, () => {
    const k = toolkit(mulberry32(SEED + 1));
    let invalid = 0;
    for (let index = 0; index < TIMECODES; index++) {
      const tc = randomTimecodeText(k);
      const [num, den] = k.pick(SIGS);
      const position = k.rand() < 0.5;
      const old = outcome(() =>
        (position ? parsePosition : parseDuration)(tc, num, den),
      );
      const now = outcome(() =>
        call<number>({
          op: "timecode.parse",
          args: { tc, num, den, position },
        }),
      );
      const what = `timecode ${index} ${JSON.stringify(tc)} ${num}/${den} position=${position}`;
      if ("error" in old) {
        invalid++;
        expect(now, what).toEqual(old);
        continue;
      }
      expect(now, what).toHaveProperty("ok");
      const got = (now as { ok: number }).ok;
      const want = old.ok as number;
      expect(
        Math.abs(got - want),
        `${what}: ${got} vs ${want}`,
      ).toBeLessThanOrEqual(SPAN_TOLERANCE * Math.max(1, Math.abs(want)));
    }
    expect(invalid).toBeGreaterThan(0);
  });

  it(`writes the same text for ${TIMECODES} random beats`, () => {
    const k = toolkit(mulberry32(SEED + 2));
    for (let index = 0; index < TIMECODES; index++) {
      const [num, den] = k.pick(SIGS);
      const position = k.rand() < 0.5;
      const beats = k.pick([
        () => k.num(0, 200),
        () => k.int(0, 400) / 16,
        () => k.int(0, 4000) / 960,
        () => -k.num(0, 5),
        () => k.num(0, 1e5),
      ])();
      const want = (position ? beatsToPosition : beatsToDuration)(
        beats,
        num,
        den,
      );
      const got = call<string>({
        op: "timecode.format",
        args: { beats, num, den, position },
      });
      expect(got, `beats ${beats} ${num}/${den} position=${position}`).toBe(
        want,
      );
    }
  });

  it("refuses a time that is too large for a float, where the TypeScript answers infinity", () => {
    const args = { tc: `${"9".repeat(400)}:1`, num: 4, den: 4, position: true };
    const e = outcome(() => call({ op: "timecode.parse", args }));
    expect(e).toHaveProperty("error");
    expect(() => call({ op: "timecode.parse", args })).toThrow(K7WasmError);
  });
});
