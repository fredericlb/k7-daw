// moduleRow.ts — named modules laid out SIDE BY SIDE in the order given, dagre unused, and the two passes that line modules up on one rank.
// The arrangements are the `chains` crate's; this file is their page-side shape: the old option names, and the folder lists ordered for people.

import type { WebGraph } from "@studio/graph-model";
import {
  alignRanks,
  moduleRowOp,
  type RowOptions,
  spaceModules,
} from "@studio/k7-wasm/graph";
import { sortFolderLists } from "./buildGraph";
import type { LaidOutGraph, RawNode } from "./graphNode";

/** What a host asks of a row; the machine's lists ride under `devices`, as `buildGraph` is told them. */
export type ModuleRowOptions = Omit<
  RowOptions,
  "inputDevices" | "cameras" | "midiDevices"
> & {
  devices?: Pick<RowOptions, "inputDevices" | "cameras" | "midiDevices">;
};

/** Pull the named modules onto ONE rank — the DEEPEST of theirs. See docs/graphBuild.md. */
export function alignModuleRank(
  nodes: RawNode[],
  modules: readonly string[],
): RawNode[] {
  const changed = new Map(alignRanks(nodes, modules));
  if (!changed.size) return nodes;
  return nodes.map((n) =>
    changed.has(n.id) ? { ...n, rank: changed.get(n.id) } : n,
  );
}

/** Re-space the named modules along x AFTER layout — equal GAP, the caller's order. See docs/graphBuild.md. */
export function spaceModulesEvenly(
  laid: LaidOutGraph,
  modules: readonly string[],
): LaidOutGraph {
  const spaced = spaceModules(laid.nodes, modules);
  if (!spaced) return laid;
  const moved = new Map(spaced.moved);
  return {
    ...laid,
    nodes: laid.nodes.map((n) => {
      const x = moved.get(n.id);
      return x === undefined ? n : { ...n, x };
    }),
    width: Math.max(laid.width, spaced.right),
  };
}

/** The named modules laid out SIDE BY SIDE in the order given, dagre unused. See docs/graphBuild.md. */
export function moduleRow(
  graph: WebGraph,
  modules: string[],
  { devices, ...opts }: ModuleRowOptions = {},
): LaidOutGraph {
  const laid = moduleRowOp(graph, modules, { ...opts, ...devices });
  for (const n of laid.nodes) sortFolderLists(n.ui);
  return laid as unknown as LaidOutGraph;
}
