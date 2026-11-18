// songRoots.ts — the places a native compile reads from and the rig it is told, as the file service's facts (docs/rustCompilerHost.md).
// The one thing a reader would get wrong: the crate finds no root itself, so every folder `projectRoots` admits must be a field here.

import { lstatSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import {
  ACCOUNT_LIBRARY,
  AUTO_RENDERS_HOME_DIR,
} from "@studio/song-schema/moduleTypes";
import { libraryReadRoots, workspaceDirOf } from "./confine";
import { sampleBanks } from "./enginePattern";
import { modelsRoot } from "./modelCache";
import { loadPreferences, type Preferences } from "./preferences";
import {
  accountLibraries,
  accountLibraryHome,
  isLibraryRootEditable,
  META_MODULES_ROOT,
  moduleNamespaceOf,
  SHARED_MODULES_PATH,
} from "./sharedFolders";
import { studioHome } from "./studioHome";

/** The folders of a library repository the compile reads, each possibly a link out of the library (decision 737). */
const FAMILIES = [
  "modules",
  "chains",
  "assemblies",
  "console",
  "samples",
  "irs",
  "wavetables",
  "models",
] as const;

/** The crate's `Roots`, by its own snake_case keys. */
export interface SongRoots {
  project: string;
  workspace: string;
  workspace_namespace?: string;
  confine: string;
  libraries: { name: string; root: string; editable: boolean }[];
  meta: string;
  scan: string;
  renders: string;
  models: string;
  sample_banks: string[];
  also_readable: string[];
  paths: "posix" | "windows";
}

/** The crate's spelling: `/` everywhere, a drive letter kept; a `\` is a name's character anywhere but Windows. */
function spell(path: string): string {
  return process.platform === "win32" ? path.replace(/\\/g, "/") : path;
}

/** The real path of `dir` when it is a link, else null; a link that cannot be read costs that folder only. */
function linkTarget(dir: string): string | null {
  try {
    return lstatSync(dir).isSymbolicLink() ? realpathSync(dir) : null;
  } catch {
    return null;
  }
}

/** Every place a compile of the project at `dir` may read, in the shapes `Roots` takes. */
export function songRoots(dir: string): SongRoots {
  const project = resolve(dir);
  const libraries = [
    { name: ACCOUNT_LIBRARY, root: accountLibraryHome() },
    ...accountLibraries(project),
  ];
  const also = new Set(libraryReadRoots());
  // A family folder that is a link leads to a real path the compile may read (737).
  for (const lib of libraries)
    for (const family of FAMILIES) {
      const target = linkTarget(join(lib.root, family));
      if (target) also.add(target);
    }
  const namespace = moduleNamespaceOf(resolve(project, SHARED_MODULES_PATH));
  return {
    project: spell(project),
    workspace: spell(dirname(project)),
    ...(namespace === "workspace" ? {} : { workspace_namespace: namespace }),
    confine: spell(workspaceDirOf(project)),
    libraries: libraries.map((l) => ({
      name: l.name,
      root: spell(l.root),
      editable: isLibraryRootEditable(l),
    })),
    meta: spell(META_MODULES_ROOT),
    // Matches engineCatalogue's `extsPath`, which that file keeps private.
    scan: spell(join(studioHome(), "exts.json")),
    renders: spell(join(studioHome(), AUTO_RENDERS_HOME_DIR)),
    models: spell(modelsRoot()),
    // The project's own `samples/` is first and the crate adds it itself, so only the machine's banks are named.
    sample_banks: sampleBanks(project).slice(1).map(spell),
    also_readable: [...also].map(spell),
    paths: process.platform === "win32" ? "windows" : "posix",
  };
}

/** The rig the compile is told: the channel labels and the cue pair, as the preferences file states them. */
export function songRig(): Pick<Preferences, "channels" | "cueOutput"> {
  const { channels, cueOutput } = loadPreferences();
  return { channels, ...(cueOutput === undefined ? {} : { cueOutput }) };
}
