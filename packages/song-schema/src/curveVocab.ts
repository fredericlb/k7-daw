// curveVocab.ts — the curve vocabulary as plain values: the oscillator shapes, the cuts and an envelope's default level.
// A reader would get wrong why these are not generated: ts-rs emits types, not values (packages/song-schema/docs/wasmBoundary.md).

import type { CurveCut, LfoShape } from "@studio/graph-model";

export const LFO_SHAPES: readonly LfoShape[] = [
  "sine",
  "triangle",
  "saw",
  "square",
];

/** The cuts a curve can take, the default first. */
export const CURVE_CUTS: readonly CurveCut[] = ["none", "positive", "negative"];

/** Where an ADSR's decay lands when the file does not say — half up, the least opinionated envelope. */
export const ADSR_DEFAULT_LEVEL = 0.5;
