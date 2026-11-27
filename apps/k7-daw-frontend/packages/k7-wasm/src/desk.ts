// desk.ts — the DJ desk's rules from Rust (ops/desk.rs): the deck document, the click, the beat grid, loops, the bindings, the set list.
// A reader would get wrong that an absent tempo or load is a missing key or null, never a made-up one: Rust reads both as "nothing there".

import type { WebGraph } from "@studio/graph-model";
import { call, trapped, typed } from "./abi.ts";
import { bindableGraph } from "./chains.ts";
import type {
  BeatGrid,
  DeckLoad,
  DeckLoop,
  DeckModuleDoc,
  DeckTempo,
  DeskBinding,
  DeskBindings,
  GridOf,
  LevelDb,
  LoopEdges,
  LoopSpan,
  MixControl,
  Needles,
  ProjectDoc,
  Published,
  SetList,
  SurfaceBinding,
} from "./types/index.ts";

export {
  BEATS_PER_BAR,
  BPM_RANGE,
  CENSOR_RATE,
  DECKS,
  DEFAULT_LOOP_BEATS,
  DOC_TEMPO,
  ECHO_RANGE,
  EQ_BANDS,
  EQ_DB,
  FILTER_RANGE,
  LEVEL_DB,
  LIVE_WARP_BPM,
  LOOP_BEATS,
  MIX_CENTRE,
  PITCH_RANGE,
  TEMPO_MODULE,
  TEMPO_PARAM,
  TEMPO_SOURCE_PARAM,
  TRIM_DB,
} from "./consts.generated.ts";
export type {
  BeatGrid,
  DeckLoad,
  DeckLoop,
  DeckModuleDoc,
  DeckTempo,
  DeskBinding,
  DeskBindings,
  GridOf,
  LevelDb,
  LoopEdges,
  LoopSpan,
  MixControl,
  Needles,
  Published,
  SetEntry,
  SetIssue,
  SetList,
} from "./types/index.ts";

import type { DECKS } from "./consts.generated.ts";

/** The two decks, named as the project names them. */
export type DeckId = (typeof DECKS)[number];
/** What each deck holds; one with nothing on it is null. */
export type DeckLoads = Record<DeckId, DeckLoad | null>;
/** What each deck plays at; one with no tempo is absent. */
export type DeckTempos = Partial<Record<DeckId, DeckTempo>>;
/** Which decks are turning: a platter carries its own transport. */
export type DeckPlaying = Partial<Record<DeckId, boolean>>;
/** One param write, a name and a value, for whichever platters a deck has. */
export type LoopParam = readonly [name: string, value: number];

const ask = <R>(op: string, args: object = {}): R =>
  call<R>({ op: `desk.${op}`, args });

// beats

/** Where the beat grid starts: a circular mean of every onset, not the first transient (docs/turntables/bpm.md). */
export const beatPhase = (
  transients: readonly number[],
  bpm: number,
): BeatGrid => ask("beat_phase", { transients, bpm });

/** Where to seek the slave so its beats land on the master's, or null when a grid is unknown. */
export const alignSeek = (
  master: GridOf,
  slave: GridOf,
  at: Needles,
): number | null => ask("align_seek", { master, slave, at });

/** The analysis with a corrected tempo laid over it: the correction belongs to the record. */
export const corrected = (
  tempo: DeckTempo,
  bpm: number | undefined,
): DeckTempo => ask("corrected", { tempo, bpm });

// loops

/** One beat in source seconds, at the record's own tempo. */
export const beatSeconds = (tempo: DeckTempo | undefined): number =>
  ask("beat_seconds", { tempo });

/** Can this deck loop at all? It takes a measured tempo and a length. */
export const canLoop = (tempo: DeckTempo | undefined): boolean =>
  ask("can_loop", { tempo });

/** The nearest beat to `at`; `at` itself when the deck has no grid. */
export const snapToBeat = (at: number, tempo: DeckTempo | undefined): number =>
  ask("snap_to_beat", { at, tempo });

/** A length halved or doubled, held between the two ends. */
export const resized = (beats: number, factor: number): number =>
  ask("resized", { beats, factor });

/** How many beats long a manual IN to OUT is. */
export const beatsBetween = (
  from: number,
  to: number,
  tempo: DeckTempo | undefined,
): number => ask("beats_between", { from, to, tempo });

/** The edges to send, or null when they would not be a loop. */
export const loopEdges = (
  loop: DeckLoop,
  tempo: DeckTempo | undefined,
): LoopEdges | null => ask("loop_edges", { loop, tempo });

/** Where a jump of `beats` lands, held inside the record. */
export const beatJump = (
  needle: number,
  beats: number,
  tempo: DeckTempo | undefined,
  quantise: boolean,
): number => ask("beat_jump", { needle, beats, tempo, quantise });

/** What to send, in order: the order is the contract. */
export const loopParams = (
  edges: LoopEdges | null,
  roll: boolean,
  resize = false,
): LoopParam[] => ask("loop_params", { edges, roll, resize });

/** The loop in source seconds, for drawing it. */
export const loopSpan = (
  loop: DeckLoop | null,
  tempo: DeckTempo | undefined,
): LoopSpan | null => ask("loop_span", { loop, tempo });

// the deck document

/** Which deck's beat the click follows, or null. */
export const metronomeDeck = (
  loads: DeckLoads,
  tempos: DeckTempos,
  master: DeckId | null,
): DeckId | null => ask("metronome_deck", { loads, tempos, master });

/** What the engine plays: the compiled project with the decks filled in, or the stand-in when there is none. */
export const deckDocument = (set: {
  compiled?: ProjectDoc | null;
  loads: DeckLoads;
  mix?: number;
  modules?: Partial<Record<DeckId, DeckModuleDoc[]>>;
  tempos?: DeckTempos;
  /** Which deck's beat the click follows, or null for no click. */
  click?: DeckId | null;
  playing?: DeckPlaying;
}): ProjectDoc => ask("document", set);

/** Every platter a deck drives: four when the record is separated, else its own. */
export const plattersOf = (deck: DeckId, load?: DeckLoad | null): string[] =>
  ask("platters_of", { deck, load });

/** How fast a platter runs, as a multiple of the record's own clock; 0 when stopped. */
export const platterRate = (
  tempo: DeckTempo | undefined,
  playing: boolean,
): number => ask("platter_rate", { tempo, playing });

/** What a platter's `Key` should be: 1 holds the pitch; a hand on the record wins. */
export const platterKey = (
  tempo: DeckTempo | undefined,
  holding: boolean,
): number => ask("platter_key", { tempo, holding });

/** The faders after one has been moved: the target, and nothing else. */
export const nudgedTempos = (
  faders: DeckTempos,
  deck: DeckId,
  bpm: number,
): DeckTempos => ask("nudged_tempos", { faders, deck, bpm });

/** The keylock switch belongs to the deck, so it is merged in rather than stored per song. */
export const withKeylock = (tempos: DeckTempos, on: DeckPlaying): DeckTempos =>
  ask("with_keylock", { tempos, on });

/** What the decks play at: the slave takes the master's tempo. */
export const syncedTempos = (
  tempos: DeckTempos,
  master: DeckId | null,
): DeckTempos => ask("synced_tempos", { tempos, master });

/** Every deck's level at crossfade `mix`, as the engine's `set-plugin-param` takes it. */
export const crossfadeLevels = (mix: number): LevelDb[] =>
  ask("crossfade_levels", { mix });

/** Has any deck's tempo moved far enough since the last send to be worth another document? */
export const worthARebuild = (sent: DeckTempos, now: DeckTempos): boolean =>
  ask("worth_a_rebuild", { sent, now });

/** How long to hold this scratch, in milliseconds: 0 goes now; a release (null rate) never waits. */
export function scratchDelay(
  sinceLast: number,
  rate: number | null,
  every?: number,
): number {
  const x = typed();
  try {
    return x.k7_g_scratch_delay(
      sinceLast,
      rate ?? Number.NaN,
      every ?? Number.NaN,
    );
  } catch (e) {
    return trapped(e);
  }
}

// bindings

/** The required controls a `mapping` does not name; the optional ones are left alone. */
export const unmappedControls = (
  mapping: Readonly<Record<string, SurfaceBinding | null>> | undefined,
): MixControl[] => ask("unmapped_controls", { mapping });

/** Resolve every binding of `views/mix.k7x` against the graph: three failures, all reported rather than dropped. */
export const resolveBindings = (
  mapping: Readonly<Record<string, SurfaceBinding | null>> | undefined,
  graph: WebGraph | null,
): DeskBindings =>
  ask("resolve_bindings", { mapping, graph: bindableGraph(graph, mapping) });

/** The stretches to apply, keyed by the TRACK each lands on. */
export const stretchTargets = (
  bindings: readonly DeskBinding[],
  byDeck: DeckTempos,
  plugins: Iterable<string>,
): DeckTempos =>
  ask("stretch_targets", { bindings, byDeck, plugins: [...plugins] });

/** What every tempo module with no plugin should read, for a host to lend to the graph. */
export const offlinePublish = (
  bindings: readonly DeskBinding[],
  byDeck: DeckTempos,
  sourceParam: string,
): Published[] => ask("offline_publish", { bindings, byDeck, sourceParam });

/** What to send for a control at `value`: normalised by the control's range, expanded by the widget's. */
export const valueFor = (binding: DeskBinding, value: number): number =>
  ask("value_for", { binding, value });

// the set list

/** A playlist resolved against the workspace's audio files; a null `files` is a tree not read yet. */
export const resolveSet = (
  text: string | null,
  files: ReadonlySet<string> | null,
): SetList => ask("resolve_set", { text, files: files ? [...files] : null });
