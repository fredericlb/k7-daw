// compile/jsDspGraph.ts — a DSP module's graph: the `jsdsp.json` beside its `dsp.k7x`, since node has no decoder of the source.
// What a reader gets wrong: a bad graph plays a pass-through with the reason, never the engine's default.

import { basename, dirname, join } from "node:path";
import { wsExists, wsRead } from "../yaml/workspaceFile";

/** What a graph that cannot play sends instead: a valid graph with no node, which both engine hosts pass through. */
export const PASS_THROUGH_GRAPH = '{"params":[],"nodes":[]}\n';

export interface JsDspGraph {
  /** The graph text the engine receives. */
  graph: string;
  /** Why the source gave no graph, when it did not; `graph` is then a pass-through. */
  error?: string;
}

const CACHE = "jsdsp.json";

// The cache's text when it parses, else undefined.
function readCache(path: string): string | undefined {
  try {
    const text = wsRead(path);
    JSON.parse(text);
    return text;
  } catch {
    return undefined;
  }
}

const passThrough = (error: string): JsDspGraph => ({
  graph: PASS_THROUGH_GRAPH,
  error,
});

function literalGraph(source: string): JsDspGraph {
  if (!wsExists(source))
    return passThrough(`its ${basename(source)} is missing`);
  const cached = readCache(join(dirname(source), CACHE));
  return cached !== undefined
    ? { graph: cached }
    : passThrough("this machine cannot decode dsp.k7x");
}

/** The graph of the DSP source at `source`: a `dsp.k7x`'s cache, any other path none. */
export function jsDspGraph(source: string): JsDspGraph {
  if (source.endsWith(".k7x")) return literalGraph(source);
  return passThrough(`${basename(source)} is not a dsp.k7x`);
}
