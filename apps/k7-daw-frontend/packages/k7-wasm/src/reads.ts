// reads.ts — what the page reads off the open project's document, answered by Rust (ops/reads.rs, song/src/reads).
// A reader would get wrong that an answer names rows and pieces by INDEX into what was passed: the page keeps its objects.
// An answer is memoised on the identity of the document passed, so a render that asks twice crosses the module once.

import { call } from "./abi.ts";
import type {
  Blanks,
  ClipLoop,
  ClipNameParts,
  CompBeats,
  Crossfade,
  DocTrack,
  FolderName,
  Hit,
  Instances,
  LaneInput,
  LaneMix,
  LaneNotes,
  LaneSpan,
  LayoutRow,
  PlayedTarget,
  RecInfo,
  SeqRef,
  ShellView,
  TakeRow,
  TakesEvent,
  TapRun,
} from "./types/index.ts";

export type {
  ClipLoop,
  ClipNameParts,
  CompBeats,
  Crossfade,
  FolderName,
  Hit,
  Instances,
  LaneInput,
  LaneNotes,
  LaneSpan,
  LayoutRow,
  PlayedTarget,
  TakeRow,
  TakesEvent,
  TapRun,
};

/** What a read takes of the page's own objects: any shape, read by Rust's slim model. */
type Doc = object;

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `reads.${op}`, args });

/** `fn` answered once per `key` object while it is alive. */
function memo<K extends object, R>(fn: (key: K) => R): (key: K) => R {
  const seen = new WeakMap<K, R>();
  return (key) => {
    if (seen.has(key)) return seen.get(key) as R;
    const out = fn(key);
    seen.set(key, out);
    return out;
  };
}

/** The loop a clip states on the axis its editor draws, or null. */
export const clipLoop = memo(
  (c: {
    type: string;
    length: number;
    loopStart?: number;
    loopEnd?: number;
  }): ClipLoop | null =>
    ask("clip_loop", {
      kind: c.type,
      length: c.length,
      loopStart: c.loopStart,
      loopEnd: c.loopEnd,
    }),
);

/** What a MIDI event plays, on the roll's axis: its window, else its placed extent; null when unplaced. */
export const playedZone = memo(
  (c: {
    type: string;
    length: number;
    loopStart?: number;
    loopEnd?: number;
  }): { from: number; to: number } | null =>
    ask("played_zone", {
      kind: c.type,
      length: c.length,
      loopStart: c.loopStart,
      loopEnd: c.loopEnd,
    }),
);

/** Every material placed on the track lanes: how often, and where first, in first-seen order. */
export const materialInstances = memo(
  (tracks: readonly Doc[]): Map<string, Instances> =>
    new Map(ask<[string, Instances][]>("material_instances", { tracks })),
);

/** The mixer modules to draw, in track order, a shared bus once. */
export const mixerModules = memo((tracks: readonly Doc[]): string[] =>
  ask("mixer_modules", { tracks }),
);

/** Every lane's notes as spans in seconds and the busiest instant each reaches; a lane with no notes is left out. */
export const noteSpans = (
  tracks: readonly Doc[],
  tempo: number | undefined,
): Record<string, LaneNotes> => ask("note_spans", { tracks, tempo });

/** Each picture lane's spans in seconds; a null end is no end. */
export const videoSpans = (
  tracks: readonly Doc[],
): Record<string, LaneSpan[]> => ask("video_spans", { tracks });

/** Raw activity as alphas between one half and one. */
export const toAlphas = (raw: readonly number[]): number[] =>
  ask("to_alphas", { raw });

/** An envelope's peaks max-pooled into at most `buckets` alphas. */
export const audioActivity = (peak: readonly number[], buckets: number) =>
  ask<number[]>("audio_activity", { peak, buckets });

/** A sequence's activity, a looping clip tiling its window's notes. */
export const midiActivity = (
  notes: readonly Doc[],
  spanBeats: number,
  buckets: number,
  loopStart = 0,
  loopEnd = 0,
): number[] =>
  ask("midi_activity", { notes, spanBeats, buckets, loopStart, loopEnd });

/** The alphas as a CSS mask gradient. */
export const activityMask = (alphas: readonly number[]): string =>
  ask("activity_mask", { alphas });

/** The meter every timecode of a read is parsed against. */
interface Meter {
  num: number;
  den: number;
}

/** The event `eventId` among a lane's clips with its takes and comp, or null; its `pieces` are indices. */
export const takesEvent = (
  track: string,
  eventId: string,
  clips: readonly Doc[],
  { num, den }: Meter,
): TakesEvent | null => ask("takes_event", { track, eventId, clips, num, den });

/** The spans of song time a take row is heard over. */
export const heardSpans = (
  comp: readonly { take: string; from: number; to: number }[],
  row: { ref: string; start: number; own: boolean },
  rowEnd: number,
): { from: number; to: number }[] =>
  ask("heard_spans", {
    comp,
    own: row.own,
    ref: row.ref,
    start: row.start,
    rowEnd,
  });

/** Where two pieces of the comp meet inside the event. */
export const compSeams = (
  start: number,
  to: number,
  comp: readonly { from: number; to: number }[],
): number[] => ask("comp_seams", { start, to, comp });

/** The line that plays `ref` over a span. */
export const compLine = (
  track: string,
  at: string,
  ref: string,
  from: number,
  to: number,
  meter: Meter,
): string => ask("comp_line", { track, at, ref, from, to, ...meter });

/** The line that arms the next take. */
export const armLine = (
  track: string,
  at: string,
  from: number,
  to: number | null,
  meter: Meter,
): string =>
  ask("arm_line", { track, at, from, to: to ?? undefined, ...meter });

/** The line that crossfades a seam over `beats`; 0 removes it. */
export const compFadeLine = (
  track: string,
  at: string,
  seam: number,
  beats: number,
  meter: Meter,
): string => ask("comp_fade_line", { track, at, seam, beats, ...meter });

/** A piece's edges dragged to `from..to`, as comp lines. */
export const pieceEdgeLines = (
  track: string,
  at: string,
  ref: string,
  pieces: readonly Doc[],
  pieceId: string,
  from: number,
  to: number,
  meter: Meter,
): string[] =>
  ask("piece_edge_lines", {
    track,
    at,
    ref,
    pieces,
    pieceId,
    from,
    to,
    ...meter,
  });

/** The lane layout: group headers, clustered members, automation lanes and picture rows, by index. */
export const projectRows = (a: {
  groups: readonly string[];
  tracks: readonly Doc[];
  video: readonly string[];
  automation: Record<string, number>;
  order?: readonly string[];
}): LayoutRow[] => ask("rows", a);

/** The id of the lane placing `clip`, or "". */
export const trackPlacing = (tracks: readonly Doc[], clip: string): string =>
  ask("track_placing", { tracks, clip });

/** The index of the placement on a lane playing material `ref`, or null. */
export const placementIndex = (
  clips: readonly Doc[],
  ref: string,
): number | null => ask("placement_of", { clips, ref });

/** The label of the midi-in node feeding `track` ("" any device), or null. */
export const midiInDeviceFor = (graph: Doc, track: string): string | null =>
  ask("midi_in_device_for", { graph, track });

/** Every lane a midi-in node is wired to. */
export const midiInLanes = memo((graph: Doc): string[] =>
  ask("midi_in_lanes", { graph }),
);

/** Every instrument a midi-in node feeds directly. */
export const midiInPlayed = memo((graph: Doc): PlayedTarget[] =>
  ask("midi_in_played", { graph }),
);

/** The first live input feeding `track`, directly or through other chains. */
export const laneInput = (graph: Doc, track: string): LaneInput | null =>
  ask("lane_input", { graph, track });

/** The sounds of the first rack downstream of `track`, as the graph holds them. */
export const rackFor = <T>(graph: Doc, track: string): T[] | null =>
  ask("rack_for", { graph, track });

/** Each rack key's name for the roll's gutter. */
export const rackKeyNames = (rack: readonly Doc[]): Map<number, string> =>
  new Map(ask<[number, string][]>("rack_key_names", { rack }));

/** The graph without the nodes in `notes`. */
export const withoutNotes = <G>(graph: G, notes: ReadonlySet<string>): G =>
  ask("without_notes", { graph, notes: [...notes] });

/** A chain file's text as a one-chain graph, or the first reason it does not read. */
export const templateGraph = <G>(
  name: string,
  text: string,
): { graph: G } | { error: string } => ask("template_graph", { name, text });

/** A line that hands a live input a channel. */
export const givesChannel = (line: string): boolean =>
  ask("gives_channel", { line });

/** Whether the graph holds an input monitored and reaching a channel. */
export const hasAudibleInput = (graph: Doc): boolean =>
  ask("has_audible_input", { graph });

/** Every note reaching each rack node, by module, sorted by start. */
export const rackHits = (
  tracks: readonly Doc[],
  graph: Doc,
): Record<string, Hit[]> => ask("rack_hits", { tracks, graph });

/** A clip ref split into the parts its shown name is made of. */
export const clipNameParts = (ref: string): ClipNameParts =>
  ask("clip_name_parts", { ref });

/** How far the output comes down for a pass that peaked at `peakDb`; never positive. */
export const trimFor = (peakDb: number): number => ask("trim_for", { peakDb });

/** The OUT node's new Volume once a pass that peaked at `peakDb` is trimmed to the target. */
export const fixedMaster = (masterDb: number, peakDb: number): number =>
  ask("fixed_master", { masterDb, peakDb });

/** The run after a tap, and its tempo in whole bpm once a second tap gives an interval. */
export const tapTempo = (taps: readonly number[], now: number): TapRun =>
  ask("tap_tempo", { taps, now });

/** The stem of a folder name, or the key of what is wrong with it (`required`, `noSlash`, `noDot`). */
export const folderName = (raw: string, suffixes: readonly string[]) =>
  ask<FolderName>("folder_name", { raw, suffixes });

/** The open timeline once the ids changed: kept, or followed through a rename. */
export const followRename = (
  open: string | null,
  before: readonly string[],
  after: readonly string[],
): string | null =>
  ask("follow_rename", { open: open ?? undefined, before, after });

/** The archetype a project's surfaces read as, or null when they are not stated. */
export const shapeOfSurfaces = (
  surfaces: readonly string[] | undefined,
): string | null => ask("shape_of_surfaces", { surfaces });

let blank: Blanks | undefined;

/** One valid document of each kind a caller with no song folder starts from, fresh every call so a caller may mutate what it gets. */
export const blanks = (): Blanks => {
  blank ??= call<Blanks>({ op: "reads.blanks" });
  return structuredClone(blank);
};

/** An authored `plugin: {type, name}` as the engine's wire form, `fallback` being the module name. */
export const enginePlugin = (
  plugin: { type?: unknown; name?: unknown } | undefined,
  fallback: string,
  emits?: string,
): { type: string; name: string } =>
  ask("engine_plugin", {
    type: plugin?.type == null ? undefined : String(plugin.type),
    name: plugin?.name == null ? undefined : String(plugin.name),
    fallback,
    emits,
  });

/** What the shell projects out of one document: the track order, the recorder's fields, the strips and the first sequence of each lane. */
export const shellView = (a: {
  tempo: number;
  num: number;
  den: number;
  countIn: number;
  tracks: readonly Doc[];
}): ShellView => ask("shell_view", a);

/** Each lane's mix and each picture lane's off flag: a group's mute silences its lanes, and one solo silences every lane not soloed. */
export const laneMix = (
  tracks: readonly Doc[],
  groups: readonly Doc[],
  video: readonly Doc[],
): { lanes: LaneMix[]; pictures: boolean[] } =>
  ask("lane_mix", { tracks, groups, video });

export type { DocTrack, LaneMix, RecInfo, SeqRef, ShellView };
