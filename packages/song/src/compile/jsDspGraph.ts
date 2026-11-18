// compile/jsDspGraph.ts — a DSP module's graph: a `dsp.k7x` decoded by the song crate, or a legacy `jsdsp.json` as it is.
// What a reader gets wrong: nothing builds a `jsdsp.ts`, and a bad graph plays a pass-through with the reason, never the engine's default.

import { basename, dirname, join } from "node:path";
import { wsExists, wsRead } from "../yaml/workspaceFile";
import { decodeDsp, dspCanDecode } from "./dspDecode";

/** What a graph that cannot play sends instead: a valid graph with no node, which both engine hosts pass through. */
export const PASS_THROUGH_GRAPH = '{"params":[],"nodes":[]}\n';

export interface JsDspGraph {
  /** The graph text the engine receives. */
  graph: string;
  /** Why the source gave no graph, when it did not; `graph` is then a pass-through. */
  error?: string;
}

const LEGACY_CACHE = "jsdsp.json";

// A legacy cache's text when it parses, else undefined.
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

// `.ts` and its kin drop to `.json`, as the Rust side's `cache_of` does.
const cacheOf = (source: string): string =>
  `${source.replace(/\.[tj]sx?$/, "")}.json`;

function literalGraph(source: string): JsDspGraph {
  if (!wsExists(source))
    return passThrough(`its ${basename(source)} is missing`);
  if (!dspCanDecode()) {
    const cached = readCache(join(dirname(source), LEGACY_CACHE));
    return cached !== undefined
      ? { graph: cached }
      : passThrough("this machine cannot decode dsp.k7x");
  }
  try {
    return { graph: decodeDsp(wsRead(source), source) };
  } catch (e) {
    return passThrough(e instanceof Error ? e.message : String(e));
  }
}

/** The graph of the DSP source at `source`: a `dsp.k7x` decoded, any other path a legacy `jsdsp.ts`'s committed cache. */
export function jsDspGraph(source: string): JsDspGraph {
  if (source.endsWith(".k7x")) return literalGraph(source);
  const cached = readCache(cacheOf(source));
  return cached !== undefined
    ? { graph: cached }
    : passThrough("its jsdsp.json is missing");
}
