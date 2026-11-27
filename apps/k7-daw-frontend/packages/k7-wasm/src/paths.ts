// paths.ts — which KIND of project file a path is, and a path made relative, resolved or split: song-edit's project_paths, one sync call each.
// A reader would get wrong that a typed name is resolved here: the scoring is Rust's (`fuzzyScores`, one call per pool), the tie-break the browser's collator.

import { call } from "./abi.ts";
import { materialRefOf } from "./schema.ts";
import type { FileTreeNode, PathRule, TreeEntry } from "./types/index.ts";
import { fuzzyScores } from "./ui.ts";

export type { FileTreeNode as FileNode, TreeEntry } from "./types/index.ts";

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `paths.${op}`, args });

const is = (rule: PathRule, path: string): boolean => ask("is", { rule, path });

/** A path with forward slashes, whatever the platform handed us. */
export const norm = (path: string): string => path.replace(/\\/g, "/");

/** A MIDI sequence file: the Editor panel previews its inline notes. */
export const isSeqFile = (path: string): boolean => is("seq", path);

/** A clip file: audio (a waveform) or video. */
export const isClipFile = (path: string): boolean => is("clip", path);

/** A LANE's file, a node in `nodes/` or a chain project's `tracks/`; deliberately loose. */
export const isTrackFile = (path: string): boolean => is("track", path);

/** A PICTURE lane's file: a lane file named after one of `pictureLanes`. */
export const isVideoTrackFile = (
  path: string,
  pictureLanes: readonly string[],
): boolean =>
  // A project with no picture lane has no picture lane's file, and the tree asks once per row.
  pictureLanes.length > 0 &&
  ask("is_video_track", { path, pictureLanes: [...pictureLanes] });

/** A module file whose rack's `source:` names a file BESIDE it, rather than the project-relative path a clip's source is. */
export const isRackModuleFile = (path: string): boolean =>
  is("rackModule", path);

/** A module file the BUILDER opens: both trees, since the builder edits both. */
export const isModuleFile = (path: string): boolean => is("module", path);

/** The chain-`use` key a file answers to in `hints.racks`: a module's folder name or a NODE's stem. */
export const rackKeyOf = (path: string): string | null =>
  ask("rack_key", { path });

/** The file in `files` that IS `<folder>/<id>`, or null: a lookup and not a search. */
export const entityFile = (
  files: string[],
  folder: string,
  id: string,
): string | null => ask("entity_file", { files, folder, id });

/** A file's id: its name without the folder or the extension. */
export const fileStem = (path: string): string => ask("stem", { path });

/** A project folder, slashed and without a trailing `/`; "" when there is none or it is the workspace itself. */
export const projectDir = (activeProject: string | null): string =>
  ask("project_dir", { project: activeProject });

/** A workspace-relative path made relative to the project folder, or merely slashed when it lies outside it. */
export const toProjectRelative = (
  path: string,
  activeProject: string | null,
): string => ask("relative", { path, project: activeProject });

/** Is this path inside the project the ENGINE holds? */
export const inActiveProject = (
  path: string,
  activeProject: string | null,
): boolean => ask("in_project", { path, project: activeProject });

/** The generic module file beside `rel`: a node's sibling. */
export const moduleFileOf = (rel: string): string =>
  ask("module_file_of", { path: rel });

/** The folder a path sits in: "" at the root, which every file route reads as the workspace. */
export const parentDir = (path: string): string => ask("parent", { path });

/** The last segment of a path: the name a rename starts from. */
export const baseName = (path: string): string => ask("base", { path });

/** A line split into its leading `n` tokens and the rest, so a name with spaces survives. */
export const headTokens = (rest: string, n: number): string[] =>
  ask("head_tokens", { rest, n });

/** A gutter source where the COMPILER puts it: a rack's sits beside its file, a clip's at the project root. */
export const resolveGutterSource = (
  source: string,
  file: string,
  project: string | null,
): string => ask("gutter", { source, file, project });

/** Enough of the hints to resolve: the "kind:name" path map and the root it is relative to. */
export interface PathHints {
  paths: Record<string, string>;
  projectRoot: string;
}

/** The module FILE behind an instance's `extends:`, or null: `base:` first, then `module:` for a local ref. */
export const moduleBehind = (
  content: string,
  hints: PathHints,
  ownPath?: string | null,
): string | null =>
  ask("module_behind", {
    content,
    paths: hints.paths,
    projectRoot: hints.projectRoot,
    own: ownPath ?? undefined,
  });

/** A clip file's kind: audio unless its source ends in a picture extension; a half-typed file answers. */
export const isAudioClipFile = (text: string): boolean =>
  ask("audio_clip", { text });

/** The rack module a sequence names via `use_rack`, so a slice rack can dock its source waveform. */
export const findUseRack = (text: string): string | null =>
  ask("use_rack", { text });

/** The tree flattened to openable text paths: binaries can't be shown. */
export const textFiles = (nodes: FileTreeNode[]): string[] =>
  ask("text_files", { nodes });

/** EVERY path the tree holds, folders included and binaries kept: what a `file …` command may name. */
export const allEntries = (nodes: FileTreeNode[]): TreeEntry[] =>
  ask("entries", { nodes });

/** The pool's best fuzzy match, or null: the highest score, ties by the collator. */
function topScore(q: string, pool: readonly string[]): string | null {
  const scores = fuzzyScores(q, pool);
  return (
    pool
      .map((p, i) => ({ p, score: scores[i] }))
      .filter((x) => x.score >= 0)
      .sort((a, b) => b.score - a.score || a.p.localeCompare(b.p))[0]?.p ?? null
  );
}

/** The best fuzzy match in a pool, or null: an exact entry first. */
function bestMatch(arg: string, pool: readonly string[]): string | null {
  if (!arg) return null;
  return pool.includes(arg) ? arg : topScore(arg, pool);
}

/** The tree entry a name means, folders included. */
export const resolveEntry = (
  arg: string,
  entries: readonly string[],
): string | null => bestMatch(arg, entries);

/** The module file a name means: MODULES FIRST, then the whole project, so `ui builder ampli` cannot land on `clips/ampli`. */
export const resolveModuleFile = (
  arg: string,
  files: readonly string[],
): string | null =>
  bestMatch(
    arg,
    files.filter((f) => is("ownModule", f)),
  ) ?? bestMatch(arg, files);

/** The sequence file a typed name means: SEQUENCES FIRST, then the whole project. */
export function resolveSequence(arg: string, files: string[]): string | null {
  if (!arg) return null;
  if (files.includes(arg)) return arg;
  return (
    topScore(
      arg,
      files.filter((f) => isSeqFile(f)),
    ) ?? topScore(arg, files)
  );
}

/** The file `ui file` was asking for: an exact path, then the best fuzzy match, then the MATERIAL behind a placement. */
export function resolveEditorFile(arg: string, files: string[]): string | null {
  if (!arg) return null;
  const material = materialRefOf(arg);
  return (
    (files.includes(arg) ? arg : null) ??
    topScore(arg, files) ??
    (material === arg ? null : topScore(material, files))
  );
}
