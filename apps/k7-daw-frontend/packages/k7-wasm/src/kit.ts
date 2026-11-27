// kit.ts — the rack builder's rules from Rust: which pad fires which note, where the samples go, what stops a CREATE, and the KIT node it commits.
// A reader would get wrong that a refusal is a sentence: Rust answers a message key and its parameters, the words are the page's (paraglide).

import { call } from "./abi.ts";
import { KIT_GRID } from "./consts.generated.ts";
import type {
  KitDestination,
  KitGrid,
  RackIssue,
  RackPad,
} from "./types/index.ts";

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

/** A pad's default symbol: the file's stem, upper-cased and squeezed to the characters a symbol is addressed by. */
export const defaultSymbol = (fileName: string): string =>
  ask("default_symbol", { fileName });

/** Which library's sampler a kit extends: `@k7-core`'s first, else any library's; null when none holds one. */
export const rackModuleRef = (baseModules: readonly string[]): string | null =>
  ask("module_ref", { baseModules: [...baseModules] });

/** The folder the samples are uploaded into, workspace-relative. */
export const kitUploadDir = (
  dest: KitDestination,
  kit: string,
  project: string,
): string => ask("upload_dir", { dest, kit, project });

/** The name a tap writes for one of them: `@project/<kit>/<file>` or `@<library>/<kit>/<file>`. */
export const kitSampleRef = (
  dest: KitDestination,
  kit: string,
  file: string,
): string => ask("sample_ref", { dest, kit, file });

/** What stops a CREATE, as message keys with their parameters. */
export const rackIssues = (
  name: string,
  pads: readonly RackPad[],
  taken: readonly string[] = [],
  hasRack = true,
): RackIssue[] =>
  ask("issues", { name, pads: [...pads], taken: [...taken], hasRack });

/** The KIT node the builder writes, as k7x: a node of the sampler, one tap per pad, one per line. */
export const buildKitNode = (
  name: string,
  pads: readonly RackPad[],
  dest: KitDestination,
  rackRef: string,
): string => ask("node", { name, pads: [...pads], dest, rackRef });
