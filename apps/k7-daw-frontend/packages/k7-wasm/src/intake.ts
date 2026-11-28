// intake.ts — the names bytes get when they enter a project: a dropped file's clip id, a recorded take's id and line, the clip declaration.
// A reader would get wrong where the accents go: the browser's `normalize` folds them here and Rust reads the folded name (song-edit's intake.rs).

import { call } from "./abi.ts";
import type { DropLane, DropPlan } from "./types/index.ts";

export * from "./ops/intake.ts";

export type {
  DropKind,
  DropLane,
  DropPlan,
} from "./types/index.ts";

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `intake.${op}`, args });

/** A number JSON can carry as it is, and as its word when it is NaN or infinite. */
const jsNumber = (n: number): number | string =>
  Number.isFinite(n)
    ? n
    : Number.isNaN(n)
      ? "NaN"
      : n > 0
        ? "Infinity"
        : "-Infinity";

/** The name without its accents: NFD, then the combining marks off. */
const fold = (name: string): string =>
  name.normalize("NFD").replace(/[\u0300-\u036f]/g, "");

/** How many `-n` suffixes are tried before giving up: a project holding fifty files called `take` has another problem. */
export const MAX_SUFFIX = 50;

/** Resolves a drop: `at` names the lane under the pointer, and its kind decides which files can be placed. */
export const planDrop = (
  names: readonly string[],
  lanes: readonly DropLane[],
  at: string,
): DropPlan =>
  ask("plan", {
    names: names.map((name) => ({ name, folded: fold(name) })),
    lanes: [...lanes],
    at,
  });

/** The candidate ids for `id`, in order: itself, then `id-2`, `id-3`… */
export const candidateIds = (id: string, n = MAX_SUFFIX): string[] =>
  ask("candidates", { id, n });

/** A wall-clock stamp for a filename (`260814T194357`), the shape a recorded audio take already uses. */
export const fileStamp = (d: Date): string =>
  ask("stamp", {
    year: d.getFullYear(),
    month: d.getMonth(),
    day: d.getDate(),
    hour: d.getHours(),
    minute: d.getMinutes(),
    second: d.getSeconds(),
  });

/** The `picture add` line a finished take becomes; the length is MEASURED, since a fresh WebM states none. */
export const placeLine = (
  lane: string,
  id: string,
  atSec: number,
  lengthSec: number,
): string =>
  ask("place_line", {
    lane,
    id,
    atSec: jsNumber(atSec),
    lengthSec: jsNumber(lengthSec),
  });
