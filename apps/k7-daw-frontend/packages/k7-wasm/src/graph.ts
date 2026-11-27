// graph.ts — the Chains tab's graph build and its arrangements from Rust: the cards and cables, a module row, its seams, the stacks.
// A reader would get wrong that each call sends only what it reads: a layout pass sends a few fields of each card, never its panel.

import type { PageModuleUI, WebGraph } from "@studio/graph-model";
import { call } from "./abi.ts";
import type {
  Built,
  EdgeInsert,
  BuildOptions as GeneratedBuildOptions,
  RowOptions as GeneratedRowOptions,
  Laid,
  RawEdge,
  RawNode,
  SeamsAnswer,
  Size,
  Spaced,
  StackAnswer,
} from "./types/index.ts";

export type { Built, EdgeInsert, Laid, SeamsAnswer, Spaced, StackAnswer };

/** The generated options with every list readonly: a host holds its lists as state and must not be asked to copy them. */
type Loose<T> = {
  [K in keyof T]: T[K] extends (infer U)[] | undefined
    ? readonly U[] | undefined
    : T[K];
};
export type BuildOptions = Loose<GeneratedBuildOptions>;
export type RowOptions = Loose<GeneratedRowOptions>;

/** The graph a build reads: what the page's other halves hold (warnings, issues, pins) is left behind. */
const buildable = (g: WebGraph): WebGraph => {
  const {
    warnings: _w,
    issues: _i,
    positions: _p,
    autoFreezeWaiting: _a,
    ...rest
  } = g;
  return rest as WebGraph;
};

/** A graph with the cables and the steps' names only: what a seam or a stack reads of the chains. */
const chainsOnly = (g: WebGraph): WebGraph =>
  ({
    chains: Object.fromEntries(
      Object.entries(g.chains).map(([name, c]) => [
        name,
        {
          from: c.from,
          out: c.out,
          use: c.use.map((s) => ({ module: s.module, category: s.category })),
        },
      ]),
    ),
    videoTracks: g.videoTracks,
  }) as unknown as WebGraph;

/** The cards and cables of a graph, unpositioned; with `open`, its assemblies folded, those named drawn open. */
export const buildGraphOp = (
  graph: WebGraph,
  opts: BuildOptions = {},
  open?: readonly string[],
): Built =>
  call<Built>({
    op: "graph.build",
    args: { graph: buildable(graph), opts, open },
  });

/** The named modules laid side by side, the graph built inside the call. */
export const moduleRowOp = (
  graph: WebGraph,
  modules: readonly string[],
  opts: RowOptions = {},
): Laid =>
  call<Laid>({
    op: "graph.row",
    args: { graph: buildable(graph), modules, opts },
  });

/** The fields of a card a rank or a spacing pass reads. */
const ranked = (n: RawNode) => ({
  id: n.id,
  kind: n.kind,
  label: n.label,
  module: n.module,
  rank: n.rank,
  x: n.x,
  w: n.w,
});

/** The ranks that change when the named modules share one: `[id, rank]` pairs. */
export const alignRanks = (
  nodes: readonly RawNode[],
  modules: readonly string[],
): [string, number][] =>
  call<[string, number][]>({
    op: "graph.align",
    args: { nodes: nodes.map(ranked), modules },
  });

/** Where the named modules move along x for an even gap, and the width that needs; null when fewer than two are there. */
export const spaceModules = (
  nodes: readonly RawNode[],
  modules: readonly string[],
): Spaced | null =>
  call<Spaced | null>({
    op: "graph.space",
    args: { nodes: nodes.map(ranked), modules },
  });

/** The fields of a card the seams and the inserts read: where it is, what it is, and its sockets. */
const placed = (n: RawNode) => ({
  id: n.id,
  kind: n.kind,
  label: n.label,
  module: n.module,
  x: n.x,
  y: n.y,
  w: n.w,
  h: n.h,
  monitor: n.monitor,
  device: n.device,
  jacks: n.jacks,
  assemblyOf: n.assemblyOf && {
    uses: n.assemblyOf.uses,
    open: n.assemblyOf.open,
    members: [],
    plugs: n.assemblyOf.plugs,
  },
});

const slimLaid = (l: {
  nodes: readonly RawNode[];
  edges: readonly RawEdge[];
  width: number;
  height?: number;
}) => ({
  nodes: l.nodes.map(placed),
  edges: l.edges.map((e) => ({
    id: e.id,
    source: e.source,
    target: e.target,
    automation: e.automation && { param: e.automation.param },
  })),
  width: l.width,
  height: l.height ?? 0,
  left: 0,
  top: 0,
});

/** A glued row's seams, its tail arrow, which cables lie glued, and the width the row then needs. */
export const seamsOp = (
  full: Parameters<typeof slimLaid>[0],
  row: Parameters<typeof slimLaid>[0],
  graph: WebGraph,
  branch: boolean,
): SeamsAnswer =>
  call<SeamsAnswer>({
    op: "graph.seams",
    args: {
      full: slimLaid(full),
      row: slimLaid(row),
      graph: chainsOnly(graph),
      branch,
    },
  });

/** What an arrow's insert is, for each `[source, target]` pair in the order given; null where nothing may splice. */
export const insertsFor = (
  nodes: readonly RawNode[],
  pairs: readonly (readonly [string, string])[],
): (EdgeInsert | null)[] =>
  call<(EdgeInsert | null)[]>({
    op: "graph.inserts",
    args: { nodes: nodes.map(placed), pairs },
  });

/** The name `graph.k7x` has for a card's jack: a box answers with the member behind its plug. */
export const jackRefOp = (n: RawNode, port?: string): string =>
  call<string>({ op: "graph.jackRef", args: { node: placed(n), port } });

/** Where the stack arrangement puts each card, and the lane each skipping cable runs down. */
export const stackOp = (
  nodes: readonly RawNode[],
  edges: readonly RawEdge[],
  graph: WebGraph,
  trackOrder: readonly string[],
): StackAnswer =>
  call<StackAnswer>({
    op: "graph.stack",
    args: {
      nodes: nodes.map((n) => ({
        ...placed(n),
        chain: n.chain,
        camera: n.camera,
      })),
      edges: edges.map((e) => ({
        id: e.id,
        source: e.source,
        target: e.target,
        automation: e.automation && { param: e.automation.param },
      })),
      graph: chainsOnly(graph),
      trackOrder,
    },
  });

/** How big a card must be to hold a panel, and with `footer` the bottom bar under it. */
export const nodeSizeOp = (ui?: PageModuleUI, footer = false): Size =>
  call<Size>({ op: "graph.nodeSize", args: { ui, footer } });
