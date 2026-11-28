// desk.ts — the DJ desk's rules from Rust (ops/desk.rs): the deck document, the click, the beat grid, loops, the bindings, the set list.
// A reader would get wrong that an absent tempo or load is a missing key or null, never a made-up one: Rust reads both as "nothing there".

import type { WebGraph } from "@studio/graph-model";
import { call } from "./abi.ts";
import { bindableGraph } from "./chains.ts";
import type {
  DeckLoad,
  DeckModuleDoc,
  DeckTempo,
  DeskBinding,
  DeskBindings,
  LoopEdges,
  ProjectDoc,
  SetList,
  SurfaceBinding,
} from "./types/index.ts";

export {
  BEATS_PER_BAR,
  BPM_RANGE,
  CENSOR_RATE,
  DECKS,
  DEFAULT_LOOP_BEATS,
  ECHO_RANGE,
  EQ_BANDS,
  EQ_DB,
  FILTER_RANGE,
  LEVEL_DB,
  LOOP_BEATS,
  MIX_CENTRE,
  PITCH_RANGE,
  TEMPO_SOURCE_PARAM,
  TRIM_DB,
} from "./consts.generated.ts";
export { scratchDelay } from "./gestureDesk.ts";
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

export * from "./ops/desk.ts";

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

// loops

/** What to send, in order: the order is the contract. */
export const loopParams = (
  edges: LoopEdges | null,
  roll: boolean,
  resize = false,
): LoopParam[] => ask("loop_params", { edges, roll, resize });

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

// bindings

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

// the set list

/** A playlist resolved against the workspace's audio files; a null `files` is a tree not read yet. */
export const resolveSet = (
  text: string | null,
  files: ReadonlySet<string> | null,
): SetList => ask("resolve_set", { text, files: files ? [...files] : null });
