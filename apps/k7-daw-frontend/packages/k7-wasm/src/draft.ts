// draft.ts — the curve editor's draft from Rust: what it opens on, the fields `curve set` writes, its picture, and where a drawn point lands.
// A reader would get wrong that a draft keeps EVERY field whichever shape is picked: switching shape loses nothing, only `fieldsOf` drops what the shape lacks.

import { call, K7WasmError } from "./abi.ts";
import type {
  CurveDraft,
  CurvePreview,
  DraftCondition,
  DraftPoint,
  DraftShape,
  DraftWhen,
} from "./types/index.ts";

export * from "./ops/draft.ts";

export type {
  CurveDraft,
  DraftCondition,
  DraftPoint,
  DraftShape,
  DraftWhen,
  ParamKind,
} from "./types/index.ts";

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `draft.${op}`, args });

/** One of WHEN the curve runs, in the order the select offers them. */
export const DRAFT_WHENS: readonly DraftWhen[] = [
  "start",
  "end",
  "between",
  "event",
];

/** Whether the shape is made of points drawn by hand: the custom curve and the on/off. */
export const isDrawnShape = (s: DraftShape): boolean =>
  s === "custom" || s === "onoff";

/** A new condition's fields, before the person picks anything. */
export const newCondition = (event = "Note On"): DraftCondition =>
  ask("new_condition", { event });

/** The whole picture in one call: span, solid and faded lines, beat lines. */
const draftPreview = (
  draft: CurveDraft,
  num: number,
  den: number,
): CurvePreview => ask("preview", { draft, num, den });

/** The picture of nothing: what a draft the module refuses is drawn as. */
const EMPTY_PREVIEW: CurvePreview = {
  span: { beats: 0, solid: 0 },
  solid: [],
  faded: [],
  beats: [],
  playhead: null,
};

/** `draftPreview`, or the empty picture where the module refuses the draft (a time half typed). */
export function draftPreviewOrEmpty(
  draft: CurveDraft,
  num: number,
  den: number,
): CurvePreview {
  try {
    return draftPreview(draft, num, den);
  } catch (e) {
    if (e instanceof K7WasmError) return EMPTY_PREVIEW;
    throw e;
  }
}

/** A drawn point at plot position (x 0..1, unit value 0..1), snapped to a sixteenth, and with `binary` to off or on. */
export const pointAt = (
  x: number,
  unit: number,
  spanBeats: number,
  num: number,
  den: number,
  binary = false,
): DraftPoint => ask("point_at", { x, unit, spanBeats, num, den, binary });
