// eyes.ts — what the page's compositor asks of Rust: a layer's schedule, the built-in transitions, a lens by its label, a LUT, a grip drag.
// A reader would get wrong that these wait for the module: they are sync, and throw `unavailable` before wasmReady() resolves.

import { call, cubeFloats, dragRectHot } from "./abi.ts";
import type { CubeAnswer } from "./types/index.ts";

export * from "./ops/eyes.ts";

/** A 3D LUT ready to upload: `size`³ entries, four floats each; throws `bad_request` naming what is wrong with the file. */
export function parseCube(text: string): { size: number; data: Float32Array } {
  const { size } = call<CubeAnswer>({ op: "eyes.cube", args: { text } });
  return { size, data: cubeFloats() };
}

/** The grips of a drag, in the order the module reads their code. */
const GRIPS = ["body", "n", "s", "e", "w", "ne", "nw", "se", "sw"] as const;

export type GripName = (typeof GRIPS)[number];

/** The rect a drag by `grip` lands on: [x, y, w, h] in whole pixels, a side never under the minimum. */
export function dragRectBy(
  rect: readonly [number, number, number, number],
  grip: GripName,
  dx: number,
  dy: number,
): [number, number, number, number] {
  const out = dragRectHot(rect, GRIPS.indexOf(grip), dx, dy);
  if (!out) throw new Error(`unknown grip ${grip}`);
  return out;
}
