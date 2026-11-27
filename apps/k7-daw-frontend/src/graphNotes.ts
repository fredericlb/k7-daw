// graphNotes.ts — the graph without its NOTES (`@meta/doc` nodes, listed under `modules`), for a view that does not draw them.
// A reader would get wrong the order: it is filtered before `buildGraph`, or the hole would stay in the layout.

import { withoutNotes as without } from "@studio/k7-wasm/reads";
import type { WebGraph } from "./types";

export const withoutNotes = (
  graph: WebGraph,
  notes: ReadonlySet<string>,
): WebGraph => (notes.size === 0 ? graph : without(graph, notes));
