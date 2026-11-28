// draft.ts — the curve editor's draft from Rust: what it opens on, the fields `curve set` writes, its picture, and where a drawn point lands.
// A reader would get wrong that a draft keeps EVERY field whichever shape is picked: switching shape loses nothing, only `fieldsOf` drops what the shape lacks.

import { call, K7WasmError } from "./abi.ts";
import type {
  CurveDraft,
  CurvePreview,
  CurveSpec,
  DraftCondition,
  DraftPoint,
  DraftShape,
  DraftWhen,
  ParamKind,
} from "./types/index.ts";

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

/** The shapes a param of this kind can follow: a number every curve but on/off, a boolean on/off alone. */
export const shapesFor = (kind?: ParamKind): DraftShape[] =>
  ask("shapes_for", { kind });

/** Whether the shape is made of points drawn by hand: the custom curve and the on/off. */
export const isDrawnShape = (s: DraftShape): boolean =>
  s === "custom" || s === "onoff";

/** A new condition's fields, before the person picks anything. */
export const newCondition = (event = "Note On"): DraftCondition =>
  ask("new_condition", { event });

/** The draft a curve opens on: its own fields over the defaults, or the defaults alone for a new one. */
export const draftOf = (spec?: CurveSpec): CurveDraft => ask("of", { spec });

/** The fields `curve set` writes: the spec, the polarity only when not the default, the conditions when there are any. */
export const fieldsOf = (draft: CurveDraft): Record<string, unknown> =>
  ask("fields", { draft });

/** Why the draft cannot be saved yet, or null: the first of its conditions that cannot be written. */
export const conditionsProblem = (draft: CurveDraft): string | null =>
  ask("conditions_problem", { draft });

/** Why one condition cannot be written, or null: a moment that is not a timecode, an event with no name. */
export const conditionProblem = (condition: DraftCondition): string | null =>
  ask("condition_problem", { condition });

/** Why the duration cannot be saved, or null. */
export const durationProblem = (draft: CurveDraft): string | null =>
  ask("duration_problem", { draft });

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

/** The points in time order, one per moment: the later write wins, as `curve point add` sets rather than duplicates. */
export const withPoint = (
  points: readonly DraftPoint[],
  point: DraftPoint,
  num: number,
  den: number,
  replacing?: string,
): DraftPoint[] =>
  ask("with_point", { points: [...points], point, num, den, replacing });
