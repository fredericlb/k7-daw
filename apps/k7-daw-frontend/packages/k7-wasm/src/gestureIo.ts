// gestureIo.ts — how a typed call crosses: the scalars it answers in, the numbers it reads, the list and the text it hands back.
// A reader would get wrong that a view dies with the next call: each helper makes its own over the memory as it is NOW.

import { type TypedAbi, trapped, typed } from "./abi.ts";

export type { TypedAbi };
export { trapped, typed };

/** A typed call answered a number it has no place for: a playhead outside the event, a tick that is not there. */
export const NONE = 2;

const decoder = new TextDecoder();

let view: Float64Array | null = null;
let viewOf: ArrayBuffer | null = null;

/** The scalars a call answered in, one view kept while the memory has not grown. */
export function scalars(x: TypedAbi): Float64Array {
  const buffer = x.memory.buffer;
  if (view === null || viewOf !== buffer) {
    view = new Float64Array(buffer, x.k7_scalars_ptr(), 16);
    viewOf = buffer;
  }
  return view;
}

/** The module's input numbers, sized to `n`, to write a call's array arguments into before it. */
export function input(x: TypedAbi, n: number): Float64Array {
  const at = x.k7_f64_in(n);
  return new Float64Array(x.memory.buffer, at, n);
}

/** A copy of the `n` numbers the last call listed. */
export function listed(x: TypedAbi, n: number): number[] {
  if (n === 0) return [];
  return Array.from(new Float64Array(x.memory.buffer, x.k7_f64_out_ptr(), n));
}

/** The `len` bytes of text the last call answered. */
export function text(x: TypedAbi, len: number): string {
  return decoder.decode(
    new Uint8Array(x.memory.buffer, x.k7_output_ptr(), len),
  );
}

/** Runs one typed call, a trap thrown away as the JSON call throws one. */
export function run<R>(f: (x: TypedAbi) => R): R {
  const x = typed();
  try {
    return f(x);
  } catch (e) {
    return trapped(e);
  }
}

/** A number a caller may leave out, sent as NaN. */
export const maybe = (v: number | null | undefined): number =>
  v === null || v === undefined ? Number.NaN : v;

/** A NaN answer read back as none. */
export const orNull = (v: number): number | null =>
  Number.isNaN(v) ? null : v;

/** How a MOVE or a RESIZE lands: on the project's grid, a whole number of steps from where it was, or free. */
export type SnapMode = "grid" | "relative" | "free";

/** The code a typed call names a mode by. */
export const MODE_CODE: Record<SnapMode, number> = {
  grid: 0,
  relative: 1,
  free: 2,
};

/** A mode by its code. */
export const MODES: readonly SnapMode[] = ["grid", "relative", "free"];
