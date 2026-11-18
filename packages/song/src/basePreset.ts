// A module's named BASES — the whole-state files in its own `base/` folder (a `.vstpreset`, or a Surge
// XT `.fxp`), and which one a module or node has chosen. Why a base is not a preset: docs/userPresets.md.
// A base may sit in a subfolder, and is then named by its path: `Pads/Bell Pad`.

import { join } from "node:path";
import { byCodePoint } from "@studio/song-schema/textOrder";
import { stub } from "./editStub";
import { listFilesUnder } from "./listFiles";

/** The folder a module keeps its bases in, and the extensions a base file carries. */
export const MODULE_BASES_DIR = "base";
const BASE_EXT = ".vstpreset";
const BASE_EXTS = [BASE_EXT, ".fxp"];
// Deep enough for a category folder and one below it; a library is not a tree to walk forever.
const MAX_DEPTH = 3;

const baseName = (file: string): string | undefined => {
  const ext = BASE_EXTS.find((e) => file.toLowerCase().endsWith(e));
  return ext ? file.slice(0, -ext.length) : undefined;
};

/** The bases in ONE folder, by name and without the extension — subfolders as `<dir>/<name>` — in a stable order. */
export function basesIn(folder: string | null | undefined): string[] {
  if (!folder) return [];
  // No base/ folder, or one that cannot be read — both are "this module has none".
  return listFilesUnder(join(folder, MODULE_BASES_DIR), {
    maxDepth: MAX_DEPTH,
    regularOnly: true,
  })
    .map(baseName)
    .filter((n): n is string => !!n)
    .sort(byCodePoint);
}

/** The file a `base:` names, or undefined so the caller falls back to the module's default — never one outside `base/`. */
export function basePresetIn(
  folder: string,
  base: unknown,
): string | undefined {
  return stub("song/basePreset.basePresetIn", folder, base);
}

/** Whether a resolved base is a Surge XT patch, which the engine loads as one rather than as a `.vstpreset`. */
export const isSurgePatch = (file: string | undefined): boolean =>
  stub("song/basePreset.isSurgePatch", file);

/** The state file a module loads: the chosen `base:`, then the node's `.vstpreset`, then the module's own. */
export function presetFor(
  folder: string,
  leaf: string,
  node: string | undefined,
  base: unknown,
): string | undefined {
  return stub("song/basePreset.presetFor", folder, leaf, node, base);
}

// A cycle in `extends` is already refused upstream; this is a backstop so a bad file cannot hang a UI request.
const MAX_ANCESTRY = 16;

/** Just enough of a module to find its folder and its parent — the resolved map and the ref map both fit. */
interface BaseOwner {
  extends?: unknown;
  /** Where its files are, stamped by the loader on every module it reads. */
  folderAbs?: unknown;
}

/** Every base this module may be set to, its ancestors' included — see docs/userPresets.md. */
export function moduleBaseChoices(
  module: BaseOwner | undefined,
  refs: Readonly<Record<string, BaseOwner>>,
): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  const add = (folder: unknown) => {
    if (typeof folder !== "string" || !folder) return;
    for (const b of basesIn(folder))
      if (!seen.has(b)) {
        seen.add(b);
        out.push(b);
      }
  };
  add(module?.folderAbs);
  let ref = module?.extends;
  for (let i = 0; i < MAX_ANCESTRY && typeof ref === "string" && ref; i++) {
    const parent = refs[ref];
    add(parent?.folderAbs);
    ref = parent?.extends;
  }
  return out;
}
