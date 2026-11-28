// text.ts — a file's k7x text edited and read by the Rust writers of song-edit: text in, the edited text and its splices out.
// A reader would get wrong what a refused edit is: a `false` in `results` with the text unchanged; a thrown Error is a text that does not read.

import { call } from "./abi.ts";
import type {
  AssemblyOp,
  ClipGrid,
  K7Change,
  MixOp,
  MixViewRead,
  PlaylistOp,
  TextAnswer,
  VideoOp,
} from "./types/index.ts";

export * from "./ops/text.ts";

export type {
  AssemblyOp,
  AssemblyRead,
  GridNote,
  LibraryPins,
  MixCues,
  MixOp,
  MixViewRead,
  PlaylistOp,
  SequenceRead,
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

/** Runs lane writers in order on the lane at `at`; a refused one answers false, and an unreadable text is left as it is. */
export const editVideo = (
  text: string,
  ops: VideoOp[],
  at: Seg[] = [],
): TextEdit => edited(ask("video", { text, at, ops }));

/** Sets cue points in the sheet, its comments and other cues kept. */
export const editMix = (text: string, ops: MixOp[]): TextEdit =>
  edited(ask("mix", { text, ops }));

/** Adds, drops or moves playlist lines; a `false` result is an edit the playlist refused. */
export const editPlaylist = (text: string, ops: PlaylistOp[]): TextEdit =>
  edited(ask("playlist", { text, ops }));

/** Promotes or takes back params of an assembly template, its comments and key order kept. */
export const editAssembly = (text: string, ops: AssemblyOp[]): TextEdit =>
  edited(ask("assembly", { text, ops }));

/** Data read at `path` with its short forms expanded: a sequence's `notes("c4 d4")`, a graph's `runs`. */
export const expandSugar = <T = unknown>(path: string, data: T): T =>
  ask("sugar_expand", { path, data });

/** Sets a clip file's `from` and `to` in the buffer, each edge spelled as it was; `from` 0 and a `to` of null leave their key out. */
export const editClipWindow = (
  text: string,
  from: number,
  to: number,
  grid: ClipGrid,
): TextEdit =>
  edited(
    ask("clip_window", {
      text,
      from: Number.isFinite(from) ? from : undefined,
      to: Number.isFinite(to) ? to : undefined,
      grid,
    }),
  );

/** Sets one slice symbol's `from` and `to` in the buffer; `results` is false when no such symbol is there. */
export const editSliceWindow = (
  text: string,
  symbol: string,
  from: number,
  to: number,
): TextEdit => edited(ask("slice_window", { text, symbol, from, to }));
