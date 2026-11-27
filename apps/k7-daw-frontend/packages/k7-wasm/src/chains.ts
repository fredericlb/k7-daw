// chains.ts — the Chains tab's rules from Rust: a widget's value maths, the cables' flow, a lane's reads of the graph, a surface's bindings.
// A reader would get wrong that each call is one JSON round trip: the graph travels whole per lane read or binding, never per frame.

import type { PageWidget, WebGraph } from "@studio/graph-model";
import { call } from "./abi.ts";
import type {
  BindingProblem,
  Kept,
  KeptChain,
  LaneSource,
  NodeFlow,
  ResolvedMapping,
  ResolvedRow,
  ResolvedTarget,
  SurfaceBinding,
} from "./types/index.ts";

export type {
  BindingProblem,
  LaneSource,
  NodeFlow,
  ResolvedTarget,
  SurfaceBinding,
};

/** The fields of a widget its value maths reads: the whole widget is never sent, a drag would pay for its options. */
const scale = (w: PageWidget) => ({
  min: w.min,
  max: w.max,
  step: w.step,
  curve: w.curve,
  pair: w.pair,
  bind: w.bind,
});

// JSON has no NaN: a range the maths cannot read answers null, which the page has always read as NaN.
const num = (n: number | null): number => n ?? Number.NaN;

/** A value to its 0..1 travel along the control: linear, log, or a crossfade's equal-power curve. */
export const widgetNorm = (w: PageWidget, value: number): number =>
  num(
    call<number | null>({
      op: "widget.norm",
      args: { widget: scale(w), value },
    }),
  );

/** A 0..1 travel back to a value, quantised to the widget's `step`. */
export const widgetDenorm = (w: PageWidget, value: number): number =>
  num(
    call<number | null>({
      op: "widget.denorm",
      args: { widget: scale(w), value },
    }),
  );

/** The dry level a crossfade knob sets beside `wet`. */
export const pairValue = (w: PageWidget, wet: number): number =>
  num(
    call<number | null>({
      op: "widget.pair",
      args: { widget: scale(w), value: wet },
    }),
  );

/** A control's reading: whole for a whole-number step, signed when its range goes below zero. */
export const widgetText = (w: PageWidget, value: number | undefined): string =>
  call<string>({ op: "widget.text", args: { widget: scale(w), value } });

/** A value typed into a control, clamped and quantised, null when the text is no number. */
export const widgetTyped = (w: PageWidget, text: string): number | null =>
  call<number | null>({ op: "widget.typed", args: { widget: scale(w), text } });

/** A knob read and typed as a mix in percent: a crossfade over two params, or the engine's own `@mix`. */
export const isMixKnob = (w: PageWidget): boolean =>
  call<boolean>({ op: "widget.mix", args: { widget: scale(w) } });

/** The seed each lane contributes, by lane id; a lane that puts out nothing is left out. */
export const seedFlows = (
  lanes: Readonly<Record<string, LaneSource>>,
): Map<string, NodeFlow> =>
  new Map(call<[string, NodeFlow][]>({ op: "flow.seeds", args: { lanes } }));

/** Do two seed maps say the same? The guard that keeps a meter frame from re-walking the graph. */
export function seedsEq(
  a: ReadonlyMap<string, NodeFlow>,
  b: ReadonlyMap<string, NodeFlow>,
): boolean {
  if (a === b) return true;
  if (a.size !== b.size) return false;
  for (const [id, x] of a) {
    const y = b.get(id);
    if (!y || x.audio !== y.audio || x.midi !== y.midi || x.video !== y.video)
      return false;
  }
  return true;
}

/** How hard each cable ripples now, by cable id; a cable carrying nothing is absent. */
export const computeFlow = (
  edges: readonly {
    id: string;
    source: string;
    target: string;
    signal?: "audio" | "midi" | "video" | "event";
  }[],
  seeds: ReadonlyMap<string, NodeFlow>,
  foldedAt: ReadonlyMap<string, string> = new Map(),
): Map<string, number> => {
  if (!seeds.size) return new Map();
  return new Map(
    call<[string, number][]>({
      op: "flow.compute",
      args: { edges, seeds: [...seeds], foldedAt: [...foldedAt] },
    }),
  );
};

/** A graph with only what a lane read looks at: the cables and the steps' names, never a panel. */
const lanesOf = (graph: WebGraph): WebGraph =>
  ({
    chains: Object.fromEntries(
      Object.entries(graph.chains).map(([name, c]) => [
        name,
        {
          from: c.from,
          out: c.out,
          use: c.use.map((s) => ({ module: s.module, category: s.category })),
        },
      ]),
    ),
    videoTracks: graph.videoTracks,
  }) as unknown as WebGraph;

/** The chains a track's signal flows through, in the order reached, and whether each loses its `from`. */
export const subgraphForTrack = (graph: WebGraph, track: string): WebGraph => {
  const kept = call<KeptChain[]>({
    op: "lane.chains",
    args: { graph: lanesOf(graph), track },
  });
  const chains: WebGraph["chains"] = {};
  for (const { name, dropFrom } of kept)
    chains[name] = dropFrom
      ? { ...graph.chains[name], from: undefined }
      : graph.chains[name];
  // The colours and curve shapes ride along, so the panel draws the same curve nodes the full graph does.
  return {
    chains,
    tracks: [track],
    trackColors: graph.trackColors,
    curves: graph.curves,
    warnings: [],
  };
};

/** The modules one track's signal reaches, in the order it reaches them. */
export const trackModules = (graph: WebGraph, track: string): string[] =>
  call<string[]>({
    op: "lane.modules",
    args: { graph: lanesOf(graph), track },
  });

/** The modules feeding one track, its input and anything between that and the lane, farthest first. */
export const trackInputs = (graph: WebGraph, track: string): string[] =>
  call<string[]>({
    op: "lane.inputs",
    args: { graph: lanesOf(graph), track },
  });

/** The graph a join reads: each step's module, plugin and track, and only the widgets some line binds to. */
export const bindableGraph = (
  graph: WebGraph | null,
  mapping: Readonly<Record<string, SurfaceBinding | null>> | undefined,
): WebGraph | null => {
  if (!graph) return null;
  const wanted = new Set<string>();
  for (const b of Object.values(mapping ?? {})) {
    if (b) wanted.add(b.param);
    if (b?.also) wanted.add(b.also.param);
  }
  const keep = (ws: Record<string, PageWidget> | undefined) =>
    Object.fromEntries(
      Object.entries(ws ?? {}).filter(([, w]) => w.bind && wanted.has(w.bind)),
    );
  return {
    chains: Object.fromEntries(
      Object.entries(graph.chains).map(([name, c]) => [
        name,
        {
          from: c.from,
          use: c.use.map((s) => ({
            module: s.module,
            pluginId: s.pluginId,
            ui: s.ui && {
              widgets: keep(s.ui.widgets),
              tabs: s.ui.tabs?.map((t) => ({ widgets: keep(t.widgets) })),
            },
          })),
        },
      ]),
    ),
  } as unknown as WebGraph;
};

/** What a surface's mapping resolved to against the graph: the lines that did, and the sentences for those that did not. */
export interface ResolvedBindings<C> {
  rows: (Omit<ResolvedRow, "control"> & { control: C })[];
  problems: BindingProblem[];
}

export type PluglessPolicy = "track" | "refuse";

/** Resolve a whole mapping against the graph: three failures, all reported rather than dropped. */
export function resolveMapping<C extends { id: string }>(
  mapping: Readonly<Record<string, SurfaceBinding | null>> | undefined,
  graph: WebGraph | null,
  opts: {
    controls: readonly C[];
    noun: string;
    plugless: PluglessPolicy;
    /** Resolve each line's second target too, as the desk does. */
    withAlso?: boolean;
  },
): ResolvedBindings<C> {
  const byId = new Map(opts.controls.map((c) => [c.id, c]));
  const { rows, problems }: ResolvedMapping = call<ResolvedMapping>({
    op: "bindings.resolve",
    args: {
      mapping,
      graph: bindableGraph(graph, mapping),
      controls: [...byId.keys()],
      noun: opts.noun,
      plugless: opts.plugless,
      withAlso: opts.withAlso ?? false,
    },
  });
  return {
    rows: rows.map((r) => ({ ...r, control: byId.get(r.control) as C })),
    problems,
  };
}

/** The controls a mapping does not name: the gate, reported once over the whole surface. */
export function unmappedControls<C extends { id: string }>(
  controls: readonly C[],
  mapping: Readonly<Record<string, SurfaceBinding | null>> | undefined,
): readonly C[] {
  const ids = new Set(
    call<string[]>({
      op: "bindings.unmapped",
      args: { mapping, controls: controls.map((c) => c.id) },
    }),
  );
  return controls.filter((c) => ids.has(c.id));
}

/** The graph with no curve card and no cable touching one, the same graph when it holds none. */
export function withoutCurves<
  G extends {
    nodes: readonly { id: string; kind: string }[];
    edges: readonly { source: string; target: string }[];
  },
>(graph: G): G {
  const kept = call<Kept | null>({
    op: "place.withoutCurves",
    args: {
      nodes: graph.nodes.map((n) => [n.id, n.kind]),
      edges: graph.edges.map((e) => [e.source, e.target]),
    },
  });
  if (!kept) return graph;
  return {
    ...graph,
    nodes: kept.nodes.map((i) => graph.nodes[i]),
    edges: kept.edges.map((i) => graph.edges[i]),
  };
}
