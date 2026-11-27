// buildGraph.ts — a WebGraph turned into unpositioned nodes and edges, built by the `chains` crate; this file asks, then orders the folder lists.
// A reader would get wrong that a dropdown's folder names are sorted HERE: people's order is the page's collator, which the module has not got.

import type { PageModuleUI, PageWidget, WebGraph } from "@studio/graph-model";
import { type BuildOptions, buildGraphOp } from "@studio/k7-wasm/graph";
import { byPeople } from "@studio/web-protocol/tree";
import type { RawEdge, RawNode } from "./graphNode";

export type { BuildOptions };

/** The binds whose dropdown lists a folder's names: `none`, then each name in the folder's own order. */
const FOLDER_BINDS = new Set(["@base", "@shader"]);

/** A dropdown the build filled from a folder: `none` first, the names after, in code-point order. */
const isFolderList = (w: PageWidget): boolean =>
  !!w.bind &&
  FOLDER_BINDS.has(w.bind) &&
  w.options?.[0]?.label === "none" &&
  w.options[0].value === 0 &&
  w.options.every((o) => o.arg === undefined);

/** Sorts each folder-backed dropdown's names for people, keeping `none` first and every value where the engine reads it. */
export function sortFolderLists(ui: PageModuleUI | undefined): void {
  if (!ui) return;
  const sort = (ws: Record<string, PageWidget>) => {
    for (const w of Object.values(ws))
      if (isFolderList(w) && w.options)
        w.options = [
          w.options[0],
          ...w.options.slice(1).sort((a, b) => byPeople(a.label, b.label)),
        ];
  };
  sort(ui.widgets);
  for (const t of ui.tabs ?? []) sort(t.widgets);
}

/** The nodes and unpositioned edges of the graph, the assemblies folded when `open` names which are open. */
export function buildGraph(
  graph: WebGraph,
  opts: BuildOptions = {},
  open?: readonly string[],
): { nodes: RawNode[]; edges: RawEdge[] } {
  const built = buildGraphOp(graph, opts, open);
  for (const n of built.nodes) sortFolderLists(n.ui);
  return built;
}

/** The nodes sized to the tab each has OPEN, top edge kept: the layout made room for the longest, a card draws only its page. */
export function withOpenTabs<
  N extends { id: string; h: number; tabH?: number[] },
>(nodes: readonly N[], tabOf: Readonly<Record<string, number>>): readonly N[] {
  let changed = false;
  const out = nodes.map((n) => {
    const h = n.tabH?.[tabOf[n.id] ?? 0];
    if (h === undefined || h === n.h) return n;
    changed = true;
    return { ...n, h };
  });
  return changed ? out : nodes;
}
