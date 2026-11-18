// moduleValues.ts — one module's params written into the project the server already holds, so a
// change that is only VALUES reaches the engine as a diff rather than as a recompile. Null means
// "not this way": the caller falls back to the full reload. See docs/verbs.md.

import type { PluginDoc, ProjectDoc } from "@studio/song/project";
import { pluginModule } from "@studio/song-schema/moduleTypes";

/** The held project with `module`'s plugins given `params` — merged over theirs, or replacing them when `whole`. */
export function withModuleParams(
  p: ProjectDoc,
  module: string,
  params: Record<string, number>,
  whole: boolean,
): ProjectDoc | null {
  let found = false;
  let dropped = false;
  const patch = (pl: PluginDoc): PluginDoc => {
    if (pluginModule(pl.id) !== module) return pl;
    found = true;
    if (!whole) return { ...pl, params: { ...pl.params, ...params } };
    // A param nothing states any more would be left where it was by a diff, where a rebuild resets it.
    if (Object.keys(pl.params).some((k) => !(k in params))) dropped = true;
    return { ...pl, params: { ...params } };
  };
  const np: ProjectDoc = {
    ...p,
    masterPlugins: p.masterPlugins.map(patch),
    tracks: p.tracks.map((t) => ({ ...t, plugins: t.plugins.map(patch) })),
  };
  return found && !dropped ? np : null;
}
