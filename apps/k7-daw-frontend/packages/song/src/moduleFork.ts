// moduleFork.ts — taking ownership of a module the ENGINE merely reports: it has no folder at
// all, so wanting a face or presets for one materialises it into the workspace's `_modules` here.
// A `@workspace` or `@k7-core` module that already HAS a folder is simply edited in place — there is
// no project-local copy to fork into instead. Rationale: docs/moduleFork.md.

import { stub } from "./editStub";
import { engineModules, ioModules } from "./engineCatalogue";
import { moduleRefOf } from "./loader";
import type { LoadedModule } from "./rawDoc";
import { loadUiLibrary, writeUiFile } from "./uiLibrary";

export type ForkedModule = {
  /** The shared ref that was edited, e.g. "@engine/audio/delay/tape-delay". */
  ref: string;
  /** The local module the copy became — what the chains now name. */
  module: string;
  /** Project-relative path of the module file that holds the edit. */
  path: string;
  /** The chains repointed at the copy (empty when the project didn't play the module yet). */
  chains: string[];
};

/** The shared roots this project inherits, each under the namespace its LOCATION gives it — docs/moduleFork.md. */
export function sharedRoots(dir: string): { root: string; ns: string }[] {
  return stub("song/moduleFork.sharedRoots", dir);
}

/** The ref a shared module file answers to, keyed exactly as readModuleTree does — docs/moduleFork.md. */
export function sharedModuleRef(
  dir: string,
  file: string,
): { ref: string; folder: string; leaf: string; type: string } | null {
  return stub("song/moduleFork.sharedModuleRef", dir, file);
}

/** Every chain of a graph's k7x text that reaches `module`, `out` counting as well as `use` — see docs/moduleFork.md. */
export function chainsUsing(text: string, module: string): string[] {
  return stub("song/moduleFork.chainsUsing", text, module);
}

/** The catalogue as the crate reads it: only the fields a materialised module is made from. */
function catalogueWire(
  cat: Record<string, LoadedModule>,
): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(cat).map(([ref, m]) => [
      ref,
      {
        category: m.category,
        file: m.file,
        io: m.io,
        plugin: m.plugin && { type: m.plugin.type, name: m.plugin.name },
      },
    ]),
  );
}

/** Give a module the ENGINE (or an io role) merely reports a folder of its own — docs/moduleFork.md. */
export function materialiseEngineModule(
  dir: string,
  ref: string,
  catalogue: Record<string, LoadedModule> = {
    ...engineModules(),
    ...ioModules(),
  },
): ForkedModule {
  const forked = stub<ForkedModule>(
    "song/moduleFork.materialiseEngineModule",
    dir,
    ref,
    catalogueWire(catalogue),
  );
  // The panel follows in TypeScript: the crate has no ui library to read (the fork is already repointed).
  const panel = loadUiLibrary(dir).byRef[ref];
  const type = String(catalogue[ref]?.category ?? "");
  const localRef = `@workspace/${moduleRefOf(type, forked.module)}`;
  if (panel)
    writeUiFile(dir, localRef, panel.ui, `panel for ${localRef} (was ${ref})`);
  return forked;
}
