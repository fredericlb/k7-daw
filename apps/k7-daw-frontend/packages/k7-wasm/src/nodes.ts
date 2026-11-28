// nodes.ts — a selection of cards and what it links, a node's shown name, a group's edge, a row's pinned mixer: the `chains` crate's rules.
// A reader would get wrong that a card travels whole: the selection rules read only its kind, name, ref, engine and box.

import type { PageWidget } from "@studio/graph-model";
import { call } from "./abi.ts";
import type {
  DragBox,
  Laid,
  SelectNode,
  ShownStep,
  SplitMixer,
} from "./types/index.ts";

export type { DragBox, SelectNode, ShownStep, SplitMixer };

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `nodes.${op}`, args });

/** Shift+click on `name`: the selection with it toggled, the single selection it grew from counted in. */
export const toggleSelection = (
  current: readonly string[],
  primary: string | null,
  name: string,
): string[] => ask("toggleSelection", { current, primary, name });

/** The names of the OTHER selected cards a control on `of` drives too: none unless all share one ref and one engine. */
export const linkedNodeNames = (
  nodes: readonly SelectNode[],
  selection: readonly string[],
  of: SelectNode,
): string[] => ask("linked", { nodes, selection, of });

/** The module names whose cards a rectangle TOUCHES, in world coordinates. */
export const nodesInBox = (
  nodes: readonly SelectNode[],
  box: DragBox,
): string[] => ask("inBox", { nodes, box });

/** Each placed node's shown name, by node name; a node no step places is absent. */
export const shownNameMap = (
  steps: readonly ShownStep[],
): Record<string, string> => ask("shownNames", { steps });

/** Each widget drawn inset, by id: framed by a group that draws a name or a switch, on its left edge. */
export const groupEdgeInsets = (
  widgets: Record<string, PageWidget>,
): Record<string, number> => ask("groupEdge", { widgets });

/** A row without its last card, and that card alone; null when the row does not END on a lane's mixer. */
export const splitTrailingMixer = (row: Laid): SplitMixer | null =>
  ask("splitMixer", { row });
