// intake.ts — the names bytes get when they enter a project: a dropped file's clip id, a recorded take's id and line, the clip declaration.
// A reader would get wrong where the accents go: the browser's `normalize` folds them here and Rust reads the folded name (song-edit's intake.rs).

import { call } from "./abi.ts";
import type {
  DropKind,
  DropLane,
  DropPlan,
  DroppedClip,
} from "./types/index.ts";

export type {
  DropKind,
  DropLane,
  DropPlan,
  DroppedClip,
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

/** Which kind of material a dropped file is, or null: the EXTENSION decides, as it does for the compiler. */
export const droppedKind = (name: string): DropKind | null =>
  ask("kind", { name });

/** A dropped file as it will sit in `clips/`: its own name reduced to what a path may hold, or null when it is neither kind. */
export const droppedClip = (name: string): DroppedClip | null =>
  ask("clip", { name, folded: fold(name) });

/** The declaration a dropped file gets: `source` and nothing else, the window being the whole file. */
export const clipDecl = (upload: string): string => ask("decl", { upload });

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

/** The `source` a declaration names, or null when it has none or does not read. */
export const declaredSource = (text: string): string | null =>
  ask("declared_source", { text });

/** The candidate ids for `id`, in order: itself, then `id-2`, `id-3`… */
export const candidateIds = (id: string, n = MAX_SUFFIX): string[] =>
  ask("candidates", { id, n });

/** The extension a container lands under: the compiler reads a clip's KIND off it. */
export const extForMime = (mime: string | undefined): string =>
  ask("ext_for_mime", { mime });

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

/** A take's clip id: the lane it was recorded on and when, reduced to what a path and a k7x key hold. */
export const takeId = (lane: string, stamp: string): string =>
  ask("take_id", { lane, stamp });

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
