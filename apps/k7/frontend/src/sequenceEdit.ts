// `sequence edit roll <sequence>` — turning the name in it into a file of this project. The
// grid it opens is the same one the popin editor tab mounts (@studio/ui-editor/note-grid/plugin).

import { fuzzyScore } from "./fuzzy";
import { isSeqFile } from "./projectPaths";

/** The sequence file a typed name means — SEQUENCES FIRST, then the whole project; null is reported rather than opening an empty grid. */
export function resolveSequence(arg: string, files: string[]): string | null {
  if (!arg) return null;
  if (files.includes(arg)) return arg;
  const best = (pool: string[]): string | null =>
    pool
      .map((p) => ({ p, score: fuzzyScore(arg, p) }))
      .filter((x) => x.score >= 0)
      .sort((a, b) => b.score - a.score || a.p.localeCompare(b.p))[0]?.p ??
    null;
  return best(files.filter(isSeqFile)) ?? best(files);
}
