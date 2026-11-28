// paths.ts — which KIND of project file a path is, and a path made relative, resolved or split: song-edit's project_paths, one sync call each.
// A reader would get wrong that a typed name is resolved here: the scoring is Rust's (`fuzzyScores`, one call per pool), the tie-break the browser's collator.

import { call } from "./abi.ts";
import { materialRefOf } from "./schema.ts";
import type { PathRule } from "./types/index.ts";
import { fuzzyScores } from "./ui.ts";

export * from "./ops/paths.ts";

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

/** A module file the BUILDER opens: both trees, since the builder edits both. */
export const isModuleFile = (path: string): boolean => is("module", path);

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
