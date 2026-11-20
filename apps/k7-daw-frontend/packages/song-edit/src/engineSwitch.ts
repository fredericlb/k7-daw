// engineSwitch.ts — running a module as another of its ENGINES, by writing `engine:` on the
// attribute set that is on. The names are asked of the RESOLVED module: a node lists no engines
// of its own, it inherits them from the module whose folder holds them. Always a reload, since
// another engine is another plugin. See apps/k7-daw-frontend/packages/song/docs/engines.md.

import { stub } from "@studio/song/editStub";

/** What a module can run as, and which one it does — empty when it has no engines. */
export function moduleEngines(
  dir: string,
  module: string,
): { names: string[]; current?: string } {
  return stub("song-edit/engineSwitch.moduleEngines", dir, module);
}

/** Switch a module to one of its engines; refused for a name it has not got. */
export function selectModuleEngine(
  dir: string,
  module: string,
  name: string,
): { path: string } {
  return stub("song-edit/engineSwitch.selectModuleEngine", dir, module, name);
}
