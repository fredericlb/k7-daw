// groupEdge.ts — which widgets share a group's LEFT edge with its name, and how far each is drawn in: the rule is Rust's (the chains crate).
// The inset is drawing only: the file, the grid and the builder's checks keep every widget on its cells.
// The one thing a reader would get wrong: a group holds nothing — membership is geometry. Rationale: ui-chains-react's docs/widgets.md.

import type { PageWidget } from "@studio/graph-model";
import { groupEdgeInsets as insets } from "@studio/k7-wasm/nodes";

/** Each widget drawn inset, by id: framed by a group that draws a name or a switch, on its left edge. */
export function groupEdgeInsets(
  widgets: Record<string, PageWidget>,
): Map<string, number> {
  return new Map(Object.entries(insets(widgets)));
}
