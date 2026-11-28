// multiSelect.ts — several nodes selected at once (Shift+click), and which of them a control LINKS: the rules are Rust's (the chains crate).
// A control moved on one node of a selection of one ref on one engine moves on all of them by the same TRAVEL, each from where it stood.
// The one thing a reader would get wrong: a mixed selection links NOTHING — no partial fan-out.

import type { PageWidget } from "@studio/graph-model";
import {
  nodesInBox as inBox,
  linkedNodeNames,
  type SelectNode,
  toggleSelection as toggle,
} from "@studio/k7-wasm/nodes";
import { type GraphNode, widgetDenorm } from "./graph";

/** A card as the rules read it: its kind, name, ref, engine, baked state and box — nothing of its panel crosses. */
const slim = (n: GraphNode): SelectNode => ({
  kind: n.kind,
  module: n.module,
  uses: n.uses as string | undefined,
  engine: n.engine as string | undefined,
  frozen: n.frozen,
  x: n.x,
  y: n.y,
  w: n.w,
  h: n.h,
});

/** Shift+click on `name`: the selection with it toggled, the single selection it grew from counted in. */
export function toggleSelection(
  current: ReadonlySet<string>,
  primary: string | null,
  name: string,
): ReadonlySet<string> {
  return new Set(toggle([...current], primary, name));
}

/** The OTHER selected nodes a control on `n` drives too — none unless all share one ref and one engine. */
export function linkedNodes(
  nodes: readonly GraphNode[],
  selection: ReadonlySet<string>,
  n: GraphNode,
): GraphNode[] {
  if (selection.size < 2) return [];
  const names = new Set(
    linkedNodeNames(nodes.map(slim), [...selection], slim(n)),
  );
  return nodes.filter((m) => m.kind === "module" && names.has(m.module ?? ""));
}

/** A linked control's value: its own start moved by the TRAVEL of the one in hand (`t0` → `t`, 0..1 of the throw), clamped. */
export function linkedValue(
  w: PageWidget,
  ownStartT: number,
  t0: number,
  t: number,
): number {
  return widgetDenorm(w, ownStartT + (t - t0));
}

/** The module names whose cards a rectangle TOUCHES, in world coordinates — a Shift+drag's catch. */
export function nodesInBox(
  nodes: readonly GraphNode[],
  box: { x0: number; y0: number; x1: number; y1: number },
): string[] {
  return inBox(nodes.map(slim), box);
}
