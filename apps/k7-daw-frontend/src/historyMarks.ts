// historyMarks.ts — what a History panel row LIGHTS on the graph and the timeline: the lanes, clips, chains and nodes its files name.
// The marks are read off the paths in Rust (the chains crate's history_marks); what stays here is the sets the surfaces test against.
// The one thing a reader would get wrong: the sets come back as lists in Rust, and a node is lit by its lane, its own name or its chain.

import type { WebGraph } from "@studio/graph-model";
import { historyMarksOf } from "@studio/k7-wasm/surfaces";

/** What one row touched, by the names the two surfaces draw with. */
export interface HistoryMark {
  lanes: ReadonlySet<string>;
  clips: ReadonlySet<string>;
  chains: ReadonlySet<string>;
  /** Single nodes, when the label names one of a touched chain's — then that chain is NOT lit whole. */
  modules: ReadonlySet<string>;
}

/** The marks for one step or commit: `paths` project-relative, `label` a journal step's command line. */
export function historyMarks(
  paths: readonly string[],
  label = "",
  graph?: WebGraph | null,
): HistoryMark {
  // Only each chain's node names cross: a command line names a node, nothing else of the graph.
  const chains = Object.entries(graph?.chains ?? {}).map(([name, c]) => [
    name,
    { use: c.use.map((s) => ({ module: s.module })) },
  ]);
  const m = historyMarksOf(
    paths,
    label,
    graph ? { chains: Object.fromEntries(chains) } : undefined,
  );
  return {
    lanes: new Set(m.lanes),
    clips: new Set(m.clips),
    chains: new Set(m.chains),
    modules: new Set(m.modules),
  };
}

/** Whether a graph card is one `mark` lights: its lane, its own name, or the chain it is in. */
export function nodeMarked(
  mark: HistoryMark,
  n: { kind: string; module?: string; chain?: string; chains?: string[] },
): boolean {
  if (n.module && mark.modules.has(n.module)) return true;
  if (n.kind === "track" && n.module && mark.lanes.has(n.module)) return true;
  if (n.chain && mark.chains.has(n.chain)) return true;
  return !!n.chains?.some((c) => mark.chains.has(c));
}

/** Nothing lit — what the surfaces read while no row is hovered. */
export const isEmptyMark = (m: HistoryMark): boolean =>
  !m.lanes.size && !m.clips.size && !m.chains.size && !m.modules.size;

/** The graph's `marked` prop for `mark`, or none — what each of the three graph hosts hands DomChains. */
export const nodePredicate = (
  mark: HistoryMark | null | undefined,
):
  | ((n: {
      kind: string;
      module?: string;
      chain?: string;
      chains?: string[];
    }) => boolean)
  | undefined => (mark ? (n) => nodeMarked(mark, n) : undefined);
