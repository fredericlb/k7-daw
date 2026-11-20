// project.ts — the ProjectDoc's shapes and the few helpers the verbs and the page still call on one: an audio clip's
// definition as an unplaced ClipDoc, a track's mixer, the group fold. The compile is the song crate's, not here.
// The doc SHAPES live in @studio/engine-protocol/project and are re-exported so this stays the one import.

import type {
  ClipDoc,
  NoteDoc,
  PluginDoc,
  ProjectDoc,
  TrackDoc,
} from "@studio/engine-protocol/project";
import { parsePosition } from "@studio/song-schema/timecode";
import { readClipWarp } from "@studio/song-schema/warpPoints";
import { confineToProject } from "./confine";
import { isFolderProject } from "./folder";
import { exportSub, readsAsPicture } from "./loader";
import type { AssembledClip } from "./rawDoc";

export type * from "@studio/engine-protocol/project";

/** Bars of count-in a project gets when settings.k7x does not say. */
export const DEFAULT_COUNT_IN = 1;
// These live in @studio/engine-protocol/build, a leaf a BROWSER can hold; re-exported where callers look.
export {
  DSP_TYPE,
  EXTERNAL_PLUGIN_FORMATS,
  enginePlugin,
  JSDSP_TYPE,
} from "@studio/engine-protocol/build";
// …and the one plugin type that is NOT on the wire, so it stays in song's own vocabulary.
export { OFFLINE_TYPE } from "@studio/song-schema/moduleTypes";

// Fold each group's mute/solo onto its members — OR, not override, see docs/mixFolding.md.
export function withGroupMix(doc: ProjectDoc): ProjectDoc {
  if (!doc.groups.length) return doc;
  const byId = new Map(doc.groups.map((g) => [g.id, g]));
  return {
    ...doc,
    tracks: doc.tracks.map((t) => {
      const g = byId.get(t.group);
      if (!g || (!g.mute && !g.solo)) return t;
      return { ...t, mute: t.mute || g.mute, solo: t.solo || g.solo };
    }),
  };
}

/** The project grid a timecode window needs to become seconds. */
export interface ClipGrid {
  tempo: number;
  num: number;
  den: number;
}

/** A clip's window edge in SECONDS; a timecode needs the grid, and without one is dropped — docs/timecode.md. */
function windowEdge(v: unknown, grid?: ClipGrid): number | undefined {
  if (v == null || v === "") return undefined;
  const n = Number(v);
  if (Number.isFinite(n)) return n > 0 ? n : undefined;
  if (!grid) return undefined;
  try {
    const beats = parsePosition(String(v), grid.num, grid.den);
    return beats > 0 ? (beats * 60) / grid.tempo : undefined;
  } catch {
    return undefined; // a half-typed or legacy edge: no window rather than a wrong one
  }
}

// Map a clip struct to a ClipDoc: with no explicit `type`, `notes` means MIDI and `source` means audio.
function mapClip(
  c: AssembledClip,
  baseDir: string,
  where: string,
  grid?: ClipGrid,
): ClipDoc {
  const type: "audio" | "midi" =
    c.type === "midi" || (!c.type && c.notes) ? "midi" : "audio";
  if (type === "audio" && !c?.source)
    throw new Error(`audio clip ${where} is missing 'source'`);
  return {
    id: String(c.id ?? ""),
    type,
    name: String(c.name ?? ""),
    // A source outside the project's roots is no file at all (docs/confine.md).
    source: c.source ? (confineToProject(baseDir, String(c.source)) ?? "") : "",
    start: Number(c.start ?? 0),
    length: Number(c.length ?? 0),
    offset: Number(c.offset ?? 0),
    ...(windowEdge(c.from, grid) != null
      ? { sourceFrom: windowEdge(c.from, grid) }
      : {}),
    ...(windowEdge(c.to, grid) != null
      ? { sourceTo: windowEdge(c.to, grid) }
      : {}),
    fadeIn: Number(c.fadeIn ?? 0),
    fadeOut: Number(c.fadeOut ?? 0),
    // The clip's own stretch map, through the same reader the node compiler uses — see docs/warp.md.
    ...(() => {
      const { map, declined } = readClipWarp(
        (c as { warp?: Parameters<typeof readClipWarp>[0] }).warp,
      );
      return map && !declined ? { warp: map } : {};
    })(),
    ...(c.disabled ? { disabled: true } : {}),
    loopStart: Number(c.loopStart ?? 0),
    loopEnd: Number(c.loopEnd ?? 0),
    notes: (c.notes ?? []).map(
      (n): NoteDoc => ({
        time: Number(n?.time ?? 0),
        duration: Number(n?.duration ?? 0),
        key: Number(n?.key ?? 0),
        vel: Number(n?.vel ?? 1),
        channel: Number(n?.channel ?? 0),
        rel: Number(n?.rel ?? 1),
      }),
    ),
  };
}

// The audio clip `id` as a standalone, UNPLACED ClipDoc — null for a video source or a single-file song.
export function loadAudioClipDoc(
  dir: string,
  id: string,
  // The project's grid, so a TIMECODE window resolves; omitted, such an edge is dropped.
  grid?: ClipGrid,
): ClipDoc | null {
  if (!isFolderProject(dir)) return null;
  const def = exportSub(dir, "clips")?.[id];
  if (!def?.source || readsAsPicture(String(def.source))) return null;
  return mapClip({ id, ...def }, dir, `'${id}'`, grid);
}

// Imported as well as re-exported: `export … from` publishes a name without BINDING it, and trackMixer reads it.
import { MIXER_PLUGIN } from "@studio/engine-protocol/build";

export {
  MIXER_PARAMS,
  MIXER_PLUGIN,
  RECORDER_PLUGIN,
  VOLUME_DB,
} from "@studio/engine-protocol/build";

/** The LAST mixer in a track's chain, or undefined — a track with none has nowhere to put a level. */
export function trackMixer(t: TrackDoc): PluginDoc | undefined {
  for (let i = t.plugins.length - 1; i >= 0; i--)
    if (t.plugins[i].type === MIXER_PLUGIN) return t.plugins[i];
  return undefined;
}
