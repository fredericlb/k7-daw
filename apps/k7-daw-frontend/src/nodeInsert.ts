// nodeInsert.ts — what the graph's `+` types into the command bar: the page's graph reduced to the names Rust reads, and the rules over them.
// A reader would get wrong where the rules are: the line's spelling, the lane of an arrow and the new name are Rust's (`@studio/k7-wasm/edit`).

import type { WebGraph } from "@studio/graph-model";
import {
  chainRefFromCreate,
  createForGraphCommand,
  type GraphNames,
  laneOfArrow as laneOf,
  nodeNameFor as nameFor,
  nodeNewRefArg,
  type PendingInsert,
  wireCommand,
} from "@studio/k7-wasm/edit";

export {
  chainRefFromCreate,
  createForGraphCommand,
  nodeNewRefArg,
  type PendingInsert,
  wireCommand,
};

/** The graph as the rules read it: each chain by its lane with the nodes it uses and what it sends to, plus every name held elsewhere. */
function namesOf(graph: Partial<WebGraph>): GraphNames {
  return {
    tracks: graph.tracks ?? [],
    chains: Object.entries(graph.chains ?? {}).map(([lane, c]) => ({
      lane,
      uses: c.use.map((s) => s.module),
      outs: c.out.map((o) => o.to),
    })),
    videoTracks: graph.videoTracks ?? [],
    modules: Object.keys(graph.modules ?? {}),
    curves: Object.keys(graph.curves ?? {}),
  };
}

/** The lane a `+` on this arrow drops onto: a chain is called after its lane, so the key is the answer, port or no port. */
export const laneOfArrow = (
  graph: Pick<WebGraph, "chains">,
  before: string | null | undefined,
): string | null => laneOf(namesOf(graph), before);

/** What a node of `module` dropped on this arrow should be called. */
export const nodeNameFor = (
  graph: WebGraph,
  module: string,
  before: string | null,
): string => nameFor(namesOf(graph), module, before);
