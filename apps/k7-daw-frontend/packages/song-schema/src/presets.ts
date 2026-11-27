// presets.ts — a module's named settings, one FILE each, in a `presets/` tree beside the modules
// rather than in the module's folder: `<presets>/<ns>/<module path>/<name>.k7x`. The tree's root is
// where it is KEPT (the workspace or a library); the path is which module it is FOR. No node:fs here,
// so the browser shares the vocabulary. See apps/k7-daw-frontend/packages/song/docs/userPresets.md.

/** The workspace's preset tree, beside `_modules/` — the `@workspace` level's spelling. */
export const SHARED_PRESETS_DIR = "_presets";

/** …and a library's, a family of the repository like `modules/` and `chains/`. */
export const LIBRARY_PRESETS_DIR = "presets";

/** Where a preset is kept: `workspace`, or the name of one of the account's libraries. */
export type PresetScope = string;

export const WORKSPACE_SCOPE: PresetScope = "workspace";

/** One preset as a listing names it: where it is kept, and its name. */
export interface PresetEntry {
  scope: PresetScope;
  name: string;
}

/** What a node offers: the module ref its presets are keyed by, those presets, and where a new one may go. */
export interface NodePresets {
  ref: string | null;
  presets: PresetEntry[];
  scopes: PresetScope[];
}

/** The level a project's own samples answer to — a sample a preset may not keep, being one project's. */
export const PROJECT_SAMPLE_PREFIX = "@project/";

/** A preset's name is its file's name, so it is held to what every filesystem agrees on. */
export const PRESET_NAME = /^[A-Za-z0-9][A-Za-z0-9 _-]{0,63}$/;

const SEGMENT = /^[A-Za-z0-9_][A-Za-z0-9_.+-]*$/;

/** `@k7-core/instruments/synth` → `k7-core/instruments/synth`, the folder its presets sit in; null for a ref that has no such path. */
export function presetModulePath(ref: string): string | null {
  const m = /^@(.+)$/.exec(ref);
  if (!m) return null;
  const segs = m[1].split("/");
  return segs.length >= 2 && segs.every((s) => SEGMENT.test(s))
    ? segs.join("/")
    : null;
}
