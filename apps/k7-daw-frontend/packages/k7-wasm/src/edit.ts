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
export * from "./ops/edit.ts";

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

/** The command line the share panel runs for the tracks zip. */
export const tracksLine = (timeline?: string): string =>
  call<string>({
    op: "edit.tracks_line",
    args: timeline ? { timeline } : {},
  });

// ---- what a sent line is expected to do --------------------------------------

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

// ---- the graph's `+` --------------------------------------------------------

/** The catalogue ref that rides on the line, which the node extends directly. */
export const nodeNewRefArg = (catalogueRef?: string): string =>
  call<string>({
    op: "edit.node_new_ref_arg",
    args: catalogueRef ? { catalogueRef } : {},
  });

// ---- lines two surfaces spell alike -----------------------------------------

// ---- the merge's gate -------------------------------------------------------
