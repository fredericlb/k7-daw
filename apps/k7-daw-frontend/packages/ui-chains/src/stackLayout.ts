// stackLayout.ts — the graph as STACKS: one column per lane, what no lane reaches a column per chain, the MIX last. No dagre.
// The arrangement is the `chains` crate's and answers only where each card goes; this file puts those places on the built nodes.

import type { WebGraph } from "@studio/graph-model";
import { stackOp } from "@studio/k7-wasm/graph";
import type {
  GraphEdge,
  GraphNode,
  LaidOutGraph,
  RawEdge,
  RawNode,
} from "./graphNode";

/** The stack arrangement of an already-built graph: lanes in `trackOrder`, whatever no lane reaches, the MIX last. */
export function stackLayout(
  nodes: RawNode[],
  edges: RawEdge[],
  graph: WebGraph,
  trackOrder: readonly string[],
): LaidOutGraph {
  const answer = stackOp(nodes, edges, graph, trackOrder);
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const sides = new Map(answer.sides);
  const placed: GraphNode[] = [];
  for (const p of answer.nodes) {
    const n = byId.get(p.id);
    if (!n) continue;
    const { rank: _rank, ...rest } = n;
    // Sockets are never PLATED here, and a card forking to more than one place keeps its outputs.
    placed.push({
      ...rest,
      x: p.x,
      y: p.y,
      w: p.w,
      jacks: p.jacks,
      ...(p.bar ? { bar: true } : {}),
    });
  }
  return {
    nodes: placed,
    edges: edges.map(
      (e): GraphEdge => ({
        id: e.id,
        assembly: e.assembly,
        source: e.source,
        target: e.target,
        label: e.label,
        automation: e.automation,
        signal: e.signal,
        targetPort: e.targetPort,
        sourcePort: e.sourcePort,
        targetSlot: e.targetSlot,
        side: sides.get(e.id),
        // Two points, which `cablePath` replaces with the sockets it computes from the nodes.
        points: [
          { x: 0, y: 0 },
          { x: 0, y: 0 },
        ],
      }),
    ),
    width: answer.width,
    height: answer.height,
    left: 0,
    top: 0,
  };
}
