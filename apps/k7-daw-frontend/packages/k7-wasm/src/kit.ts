// kit.ts — the rack builder's rules from Rust: which pad fires which note, where the samples go, what stops a CREATE, and the KIT node it commits.
// A reader would get wrong that a refusal is a sentence: Rust answers a message key and its parameters, the words are the page's (paraglide).

import { call } from "./abi.ts";
import { KIT_GRID } from "./consts.generated.ts";
import type { KitGrid, RackIssue, RackPad } from "./types/index.ts";

export * from "./ops/kit.ts";

export type { KitDestination, RackIssue, RackPad } from "./types/index.ts";

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `kit.${op}`, args });

/** The grid's side: 4×4 today, sixteen pads, the drum-machine page. */
export const GRID = KIT_GRID;

let grid: KitGrid["cells"] | undefined;

/** The grid's cells in NOTE order, asked once: the grid is a rule, not a document. */
const cells = (): KitGrid["cells"] => {
  grid ??= ask<KitGrid>("grid", {}).cells;
  return grid;
};

/** Every cell in NOTE order, the order the symbols are WRITTEN in. */
export const cellsInNoteOrder = (): { row: number; col: number }[] =>
  cells().map(({ row, col }) => ({ row, col }));

/** The MIDI note a pad fires: the grid READS like a drum machine, bottom-left 36 to top-right 51, `row` counting from the TOP. */
export const padNote = (row: number, col: number): number =>
  cells().find((c) => c.row === row && c.col === col)?.note ?? Number.NaN;

/** What stops a CREATE, as message keys with their parameters. */
export const rackIssues = (
  name: string,
  pads: readonly RackPad[],
  taken: readonly string[] = [],
  hasRack = true,
): RackIssue[] =>
  ask("issues", { name, pads: [...pads], taken: [...taken], hasRack });
