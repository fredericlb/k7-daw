// nodeDeleteGuard.ts — whether `node delete` would leave a module it feeds short of a mandatory input.
// What a reader gets wrong: a node something feeds is never refused, since the delete joins its feeders to what it fed.

import { normalizeInputs } from "@studio/song/graph";
import { loadModuleRefs, loadModules } from "@studio/song/loader";
import { resolveModuleMap } from "@studio/song/moduleResolve";
import { readGraphValue } from "@studio/song-edit/projectEdit";
import { splitPort } from "@studio/song-schema/moduleTypes";

/** The nodes `node` is the last feed of while their module requires one, in graph order; empty to go ahead. */
export function starvedByDelete(dir: string, node: string): string[] {
  let graph: Record<string, string[]>;
  try {
    graph = readGraphValue(dir).modules ?? {};
  } catch {
    // An unreadable graph is the delete's own error to report.
    return [];
  }
  const feeders = (target: string): string[] =>
    Object.entries(graph)
      .filter(([, dests]) => dests.some((d) => splitPort(d).node === target))
      .map(([from]) => from);
  if (feeders(node).length) return [];
  const fed = [...new Set((graph[node] ?? []).map((d) => splitPort(d).node))];
  if (!fed.length) return [];
  // Resolved, as the compile reads them: a node's ports are usually its module's.
  const modules = resolveModuleMap(loadModules(dir), loadModuleRefs(dir));
  return fed.filter((d) => {
    const mandatory = normalizeInputs(modules[d]?.inputs).filter(
      (i) => i.mandatory,
    ).length;
    return feeders(d).filter((f) => f !== node).length < mandatory;
  });
}
