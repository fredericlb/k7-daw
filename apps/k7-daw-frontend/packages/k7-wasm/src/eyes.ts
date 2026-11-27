// eyes.ts — what the page's compositor asks of Rust: a layer's schedule, the built-in transitions, a lens by its label, a LUT, a grip drag.
// A reader would get wrong that these wait for the module: they are sync, and throw `unavailable` before wasmReady() resolves.

import { call, cubeFloats, dragRectHot } from "./abi.ts";
import type {
  CameraDevice,
  CameraMatch,
  CubeAnswer,
  LayerSchedule,
  SceneClip,
  SceneTransition,
} from "./types/index.ts";

/** The table of spans a layer's clips make, which the compositor looks up per frame instead of working the rule out. */
export const layerSchedule = (clips: readonly SceneClip[]): LayerSchedule =>
  call<LayerSchedule>({ op: "eyes.schedule", args: { clips } });

/** The two transitions the compositor owns, `dissolve` and `over`, keyed by name. */
export const builtinTransitions = (): Record<string, SceneTransition> =>
  call<Record<string, SceneTransition>>({ op: "eyes.builtins", args: {} });

/** Which lens a layer's label names among the machine's devices; a lens that is not here falls back, with the reason. */
export const matchCamera = (
  label: string,
  devices: readonly CameraDevice[],
): CameraMatch =>
  call<CameraMatch>({ op: "eyes.match_camera", args: { label, devices } });

/** A 3D LUT ready to upload: `size`³ entries, four floats each; throws `bad_request` naming what is wrong with the file. */
export function parseCube(text: string): { size: number; data: Float32Array } {
  const { size } = call<CubeAnswer>({ op: "eyes.cube", args: { text } });
  return { size, data: cubeFloats() };
}

/** The grips of a drag, in the order the module reads their code. */
export const GRIPS = [
  "body",
  "n",
  "s",
  "e",
  "w",
  "ne",
  "nw",
  "se",
  "sw",
] as const;

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
