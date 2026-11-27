// edit.ts — the rules of the lines the page writes and reads back, from Rust (the `verbs` crate's `ui` module): export, pending edits, reloads, `+`.
// A reader would get wrong that state is held here: a pending edit is a value the page keeps and hands back on each call.

import { call } from "./abi.ts";
import type {
  ChangedFile,
  ClipFields,
  ClipPatches,
  ExportAsk,
  GraphNames,
  MixFormat,
  PendingEdit,
  PendingInsert,
  Scope,
  Spawn,
  TimelineFormat,
  VerbClass,
  VideoFormat,
} from "./types/index.ts";

export {
  DEFAULT_VIDEO_AUDIO,
  EXPORT_USAGE,
  MIX_FORMATS,
  TIMELINE_FORMATS,
  VIDEO_FORMATS,
} from "./consts.generated.ts";

export type {
  ChangedFile,
  ClipFields,
  ExportAsk,
  GraphNames,
  MixFormat,
  PendingEdit,
  PendingInsert,
  Scope,
  Spawn,
  TimelineFormat,
  VerbClass,
  VideoFormat,
};

// ---- project export ---------------------------------------------------------

/** Read the rest of a `project export` line, or null when it says nothing this understands. */
export const parseExportAsk = (rest: string): ExportAsk | null =>
  call<ExportAsk | null>({ op: "edit.parse_export_ask", args: { rest } });

/** The command line the share panel runs for the tracks zip. */
export const tracksLine = (timeline?: string): string =>
  call<string>({
    op: "edit.tracks_line",
    args: timeline ? { timeline } : {},
  });

// ---- what a sent line is expected to do --------------------------------------

/** The edits `line` makes: none for any other verb, or for a line that does not parse. */
export const pendingOf = (
  line: string,
  num: number,
  den: number,
): PendingEdit[] =>
  call<PendingEdit[]>({ op: "edit.pending_of", args: { line, num, den } });

type PatchedClip = {
  start: number;
  disabled?: boolean;
  fadeIn?: number;
  fadeOut?: number;
  warpOff?: boolean;
};

/** A lane's clips as the pending edits leave them: hidden ones gone, a mute flipped, a range moved. */
export function pendingClips<C extends PatchedClip>(
  track: string,
  clips: readonly C[],
  edits: readonly PendingEdit[],
): C[] {
  if (!edits.length) return clips as C[];
  const fields: ClipFields[] = clips.map((c) => ({
    start: c.start,
    ...(c.disabled !== undefined ? { disabled: c.disabled } : {}),
    ...(c.fadeIn !== undefined ? { fadeIn: c.fadeIn } : {}),
    ...(c.fadeOut !== undefined ? { fadeOut: c.fadeOut } : {}),
    ...(c.warpOff !== undefined ? { warpOff: c.warpOff } : {}),
  }));
  const { patches } = call<ClipPatches>({
    op: "edit.pending_clips",
    args: { track, clips: fields, edits: [...edits] },
  });
  const out: C[] = [];
  clips.forEach((c, i) => {
    const patch = patches[i];
    if (!patch) return;
    const next = { ...c } as Record<string, unknown>;
    // A null takes the field away, as the page's `undefined` did.
    for (const [k, v] of Object.entries(patch)) next[k] = v ?? undefined;
    out.push(next as C);
  });
  return out;
}

/** Where a pending split cuts the event starting at `start` on `track`, if one does. */
export const pendingSplitAt = (
  track: string,
  start: number,
  edits: readonly PendingEdit[],
): number | undefined =>
  call<number | null>({
    op: "edit.pending_split_at",
    args: { track, start, edits: [...edits] },
  }) ?? undefined;

// ---- which routes a reload concerns -----------------------------------------

/** Whether a reload that wrote `files` (null: unknown) concerns `scope`. */
export const reloadMoved = (
  scope: Scope,
  files: readonly ChangedFile[] | null,
): boolean =>
  call<boolean>({
    op: "edit.reload_moved",
    args: { scope, files: files ? [...files] : null },
  });

// ---- the graph's `+` --------------------------------------------------------

/** The lane a `+` on this arrow drops onto, or null on the bare canvas. */
export const laneOfArrow = (
  graph: GraphNames,
  before: string | null | undefined,
): string | null =>
  call<string | null>({
    op: "edit.lane_of_arrow",
    args: { graph, before: before ?? null },
  });

/** What a node of `module` dropped on this arrow should be called. */
export const nodeNameFor = (
  graph: GraphNames,
  module: string,
  before: string | null,
): string =>
  call<string>({
    op: "edit.node_name_for",
    args: { graph, module, before },
  });

/** What a chain should name, given the creating command that just ran: a node is named by its file alone. */
export const chainRefFromCreate = (line: string): string | null =>
  call<string | null>({ op: "edit.chain_ref_from_create", args: { line } });

/** The line the `+` pre-writes: `node new`, ending where the node's name goes. */
export const createForGraphCommand = (kind: string, name: string): string =>
  call<string>({
    op: "edit.create_for_graph_command",
    args: { kind, name },
  });

/** The catalogue ref that rides on the line, which the node extends directly. */
export const nodeNewRefArg = (catalogueRef?: string): string =>
  call<string>({
    op: "edit.node_new_ref_arg",
    args: catalogueRef ? { catalogueRef } : {},
  });

/** Where the node that line makes should go: a splice on an arrow, and on the bare canvas only the pin, and only with a point. */
export const wireCommand = (
  node: string,
  at: PendingInsert | null,
  spawn?: Spawn | null,
): string | null =>
  call<string | null>({
    op: "edit.wire_command",
    args: { node, at, spawn: spawn ?? null },
  });

// ---- lines two surfaces spell alike -----------------------------------------

/** The chains filtered on the whole ref: an empty search is every chain rather than none. */
export const filterOps = (ops: string[], search: string): string[] =>
  call<string[]>({ op: "edit.filter_ops", args: { ops, search } });

/** The command line that runs a library chain over one clip. */
export const processLine = (clip: string, chain: string): string =>
  call<string>({ op: "edit.process_line", args: { clip, chain } });

/** The records a set list's line may name: every audio file but a `stems/` folder's, sorted. */
export const playlistRefs = (files: readonly string[]): string[] =>
  call<string[]>({ op: "edit.playlist_refs", args: { files: [...files] } });

/** The fields a set list's file may carry at `path`. */
export const playlistFieldsAt = (path: readonly string[]): string[] =>
  call<string[]>({ op: "edit.playlist_fields_at", args: { path: [...path] } });

/** The values a set list's field may take, over the records. */
export const playlistValuesAt = (
  refs: readonly string[],
  path: readonly string[],
  field: string,
): string[] =>
  call<string[]>({
    op: "edit.playlist_values_at",
    args: { refs: [...refs], path: [...path], field },
  });

// ---- the merge's gate -------------------------------------------------------

/** What a command line does to the workspace: it writes, only reads, or writes this device's settings. */
export const classOfLine = (line: string): VerbClass =>
  call<VerbClass>({ op: "edit.class_of_line", args: { line } });

/** Every verb of the catalogue, which the gate classes. */
export const classedVerbs = (): string[] =>
  call<string[]>({ op: "edit.classed_verbs" });
