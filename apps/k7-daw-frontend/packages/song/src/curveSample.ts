// curveSample.ts — the compiler's curve sampling: a curve's value at a song beat, in TypeScript until the compiler is ported.
// A reader would get wrong that it is not the browser's preview: that one is Rust (apps/k7-daw-frontend/packages/song-schema/docs/wasmBoundary.md).
// A UNIT value is 0..1 before amplitude and polarity. See apps/k7-daw-frontend/packages/song/docs/curves.md.

import type {
  CurveCut,
  CurvePolarity,
  CurveSpec,
  LfoShape,
} from "@studio/graph-model";
import { parsePosition } from "@studio/song-schema/timecode";

/** An oscillator's unit value `cycles` into its run, 0..1. */
function lfoWave(shape: LfoShape, cycles: number): number {
  const f = cycles - Math.floor(cycles); // position within the cycle, 0..1
  switch (shape) {
    case "triangle":
      return f < 0.5 ? 2 * f : 2 * (1 - f);
    case "saw":
      return f;
    case "square":
      return f < 0.5 ? 1 : 0;
    default:
      return (Math.sin(2 * Math.PI * cycles) + 1) / 2;
  }
}

type Adsr = Extract<CurveSpec, { type: "adsr" }>;

/** The envelope's own length, in ms: what one trigger occupies before it is over. */
export const adsrMs = (c: Adsr): number =>
  c.attack + c.decay + c.sustain + c.release;

/** The envelope's unit value `ms` after its trigger, 0..1, flat 0 once over. */
export function adsrUnit(c: Adsr, ms: number): number {
  if (ms <= 0) return 0;
  if (ms < c.attack) return ms / c.attack;
  const d = ms - c.attack;
  if (d < c.decay) return 1 - (1 - c.level) * (d / c.decay);
  const h = d - c.decay;
  if (h < c.sustain) return c.level;
  const r = h - c.sustain;
  if (r < c.release) return c.level * (1 - r / c.release);
  return 0;
}

/** Points at beats, linearly interpolated; flat before the first and after the last. */
function pointsUnit(
  pts: readonly { beat: number; value: number }[],
  beat: number,
): number {
  if (!pts.length) return 0;
  if (beat <= pts[0].beat) return pts[0].value;
  const last = pts[pts.length - 1];
  if (beat >= last.beat) return last.value;
  for (let i = 1; i < pts.length; i++) {
    const b = pts[i];
    if (beat > b.beat) continue;
    const a = pts[i - 1];
    const span = b.beat - a.beat;
    // Coincident points take the later value.
    return span <= 0
      ? b.value
      : a.value + (b.value - a.value) * ((beat - a.beat) / span);
  }
  return last.value;
}

/** Points at beats, HELD: each one's value until the next, the first's before it — an on/off curve's stepping. */
function heldUnit(
  pts: readonly { beat: number; value: number }[],
  beat: number,
): number {
  if (!pts.length) return 0;
  let v = pts[0].value;
  for (const p of pts) {
    if (p.beat > beat) break;
    v = p.value;
  }
  return v;
}

/** A custom curve's points at their beats, in time order. */
function pointBeats(
  points: readonly { time: string; value: number }[],
  num: number,
  den: number,
): { beat: number; value: number }[] {
  return points
    .map((p) => ({ beat: parsePosition(p.time, num, den), value: p.value }))
    .sort((a, b) => a.beat - b.beat);
}

/** What leaves the curve: the unit value scaled by `amplitude`, unipolar 0..amp or bipolar -amp..amp, then `cut`. */
function curveOutput(
  unit: number,
  polarity: CurvePolarity,
  amplitude: number,
  cut?: CurveCut,
): number {
  return cutOutput(
    (polarity === "bipolar" ? unit * 2 - 1 : unit) * amplitude,
    cut,
  );
}

/** An output with its cut applied: `positive` keeps [0,1], `negative` keeps [-1,0], `none` all of it. */
export function cutOutput(v: number, cut?: CurveCut): number {
  if (cut === "positive") return Math.max(0, v);
  if (cut === "negative") return Math.min(0, v);
  return v;
}

/** A curve's output at song beat `beat` — an ADSR has none there, only after a trigger, so 0. */
export function sampleSpec(
  c: CurveSpec,
  beat: number,
  num: number,
  den: number,
): number {
  const barBeats = num * (4 / den);
  const unit =
    c.type === "adsr"
      ? 0
      : c.type === "lfo"
        ? lfoWave(
            c.shape,
            c.frequency * (barBeats > 0 ? beat / barBeats : 0) + c.phase,
          )
        : c.type === "onoff"
          ? heldUnit(pointBeats(c.points, num, den), beat)
          : pointsUnit(pointBeats(c.points, num, den), beat);
  return curveOutput(unit, c.polarity, c.amplitude, c.cut);
}
