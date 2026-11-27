// text.ts — a file's k7x text edited and read by the Rust writers of song-edit: text in, the edited text and its splices out.
// A reader would get wrong what a refused edit is: a `false` in `results` with the text unchanged; a thrown Error is a text that does not read.

import { call } from "./abi.ts";
import type {
  AssemblyOp,
  AssemblyRead,
  ChainRead,
  K7Change,
  LibraryPins,
  MixCues,
  MixOp,
  MixViewRead,
  PlaylistOp,
  SequenceOp,
  TextAnswer,
  VideoOp,
  VideoPlacement,
} from "./types/index.ts";

export type {
  AssemblyOp,
  AssemblyRead,
  ChainItem,
  ChainRead,
  LibraryPins,
  MixCues,
  MixOp,
  MixViewRead,
  PlaylistOp,
  SequenceOp,
  VideoOp,
  VideoPlacement,
} from "./types/index.ts";

/** What `views/mix.k7x` binds as the page holds it: control id to what it drives, `null` for one not wired yet. */
export type MixView = NonNullable<MixViewRead["value"]>;

/** What a writer left: the new text, the splices that made it, and each op's own answer (false for a refused edit). */
export interface TextEdit {
  text: string;
  changes: K7Change[];
  results: boolean[];
}

type Seg = string | number;

/** The answer as a value: a failed op is the Error its sentence says. */
function edited(a: TextAnswer): TextEdit {
  if (a.error) throw new Error(a.error.message);
  return { text: a.text, changes: a.changes, results: a.results };
}

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `text.${op}`, args });

/** The placements a picture lane declares, in file order, the ref-less ones included; an unreadable text has none. */
export const listVideoPlacements = (
  text: string,
  at: Seg[] = [],
): VideoPlacement[] => ask("video_list", { text, at });

/** Runs lane writers in order on the lane at `at`; a refused one answers false, and an unreadable text is left as it is. */
export const editVideo = (
  text: string,
  ops: VideoOp[],
  at: Seg[] = [],
): TextEdit => edited(ask("video", { text, at, ops }));

/** A sequence's data with its short form expanded; throws when the text does not read or the short form does not. */
export const sequenceData = (text: string): Record<string, unknown> =>
  ask("sequence_data", { text });

/** Runs note writers on a sequence's EXPANDED notes, the short form kept while it holds; throws what a writer refuses with. */
export const editSequence = (text: string, ops: SequenceOp[]): TextEdit =>
  edited(ask("sequence", { text, ops }));

/** The cue points `mix.k7x` holds by song ref; none, with the issues, when the sheet does not read. */
export const mixCues = (text: string): MixCues => ask("mix_cues", { text });

/** Sets cue points in the sheet, its comments and other cues kept. */
export const editMix = (text: string, ops: MixOp[]): TextEdit =>
  edited(ask("mix", { text, ops }));

/** The desk's bindings from `views/mix.k7x`; none, with the issues, when the file does not read. */
export const readMixView = (text: string): MixViewRead =>
  ask("mix_view", { text });

/** What a playlist file that does not exist yet says: a stub that parses. */
export const playlistStub = (name: string): string =>
  ask("playlist_stub", { name });

/** Adds, drops or moves playlist lines; a `false` result is an edit the playlist refused. */
export const editPlaylist = (text: string, ops: PlaylistOp[]): TextEdit =>
  edited(ask("playlist", { text, ops }));

/** A workspace's pins from `_library-pins.k7x`; a text that does not read is no pins. */
export const parseLibraryPins = (text: string): LibraryPins =>
  ask("pins_parse", { text });

/** `_library-pins.k7x`'s text, libraries in name order so a diff stays one line. */
export const stringifyLibraryPins = (pins: LibraryPins): string =>
  ask("pins_write", { pins });

/** What a library `assembly.k7x` promotes; none, with the issues, when the file does not read. */
export const readAssemblyTemplate = (text: string): AssemblyRead =>
  ask("assembly_read", { text });

/** Promotes or takes back params of an assembly template, its comments and key order kept. */
export const editAssembly = (text: string, ops: AssemblyOp[]): TextEdit =>
  edited(ask("assembly", { text, ops }));

/** The items of a chain file's steps, or why it does not read. */
export const readChain = (text: string): ChainRead =>
  ask("chain_read", { text });

/** Data read at `path` with its short forms expanded: a sequence's `notes("c4 d4")`, a graph's `runs`. */
export const expandSugar = <T = unknown>(path: string, data: T): T =>
  ask("sugar_expand", { path, data });

/** The text of `path` made to hold `after` (its expanded data), each short form kept while its expansion is unchanged. */
export const applySugared = (
  path: string,
  text: string,
  after: unknown,
): TextEdit => edited(ask("sugar_apply", { path, text, after }));
