// layout.ts — the page's door to the Rust layout: it sends what the layout reads and puts the answer back on the page's own cards.
// A reader would get wrong that the layout is Rust's end to end: rust-sugiyama, the rank rows and the pins (docs/graphBuild.md).

import { layoutOp } from "@studio/k7-wasm/graph";
import type {
  GraphEdge,
  GraphNode,
  LaidOutGraph,
  Pt,
  RawEdge,
  RawNode,
} from "./graphNode";

/** Lay a built graph out; `positions` are the file's pins by name, `prev` where each card was drawn last, by id. */
export function layout(
  nodes: RawNode[],
  raw: RawEdge[],
  positions?: Record<string, Pt>,
  prev?: ReadonlyMap<string, Pt>,
): LaidOutGraph {
  const at = layoutOp(nodes, raw, positions, prev);
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const placed: GraphNode[] = [];
  for (const p of at.nodes) {
    const n = byId.get(p.id);
    if (!n) continue;
    placed.push({
      ...n,
      x: p.x,
      y: p.y,
      w: p.w,
      h: p.h,
      ...(p.assembly ? { assembly: p.assembly } : {}),
      ...(p.pushed ? { pushed: p.pushed } : {}),
    });
  }
  const points = new Map(at.edges);
  const edges = raw.map(
    (e): GraphEdge => ({
      id: e.id,
      source: e.source,
      target: e.target,
      label: e.label,
      automation: e.automation,
      signal: e.signal,
      targetPort: e.targetPort,
      sourcePort: e.sourcePort,
      targetSlot: e.targetSlot,
      assembly: e.assembly,
      points: points.get(e.id) ?? [],
    }),
  );
  return {
    nodes: placed,
    edges,
    width: at.width,
    height: at.height,
    left: at.left,
    top: at.top,
  };
}
