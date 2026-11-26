// types.ts — the chains routing graph as the browser receives it: the types are GENERATED from the song crate's Rust (./generated).
// What stays here is what a generated file cannot hold: the values both sides agree on, and the small helpers over the types.
// Lives apart from @studio/web-protocol so that drawing a graph drags in neither auth, nor workspaces, nor the engine registry.

import type {
  CcBinding,
  CurveFile,
  Cut,
  Polarity,
  PortType,
  Shape,
  WebGraph,
  WebModuleCategory,
  WebModuleUI,
  WebTab,
  WebWidget,
  WidgetOption,
} from "./generated";

export type * from "./generated";

/** What a wire carries; `event` stays OUT of the signal path and is typed only to colour a socket. */
export type WebSignal = PortType | "event";

/** One MIDI control change: the channel a musician counts (1-16), and the control number (0-127). */
export type WebCc = CcBinding;
/** A curve with every field resolved, and when it runs. */
export type CurveSpec = CurveFile;
export type LfoShape = Shape;
export type CurvePolarity = Polarity;
export type CurveCut = Cut;

/** What the PAGE adds to a panel the wire carries: a dropdown's `arg`, a tab's folded `cols`, the narrow card's `tabsAs`. */
export type PageWidgetOption = WidgetOption & { arg?: string };
export type PageWidget = Omit<WebWidget, "options"> & {
  options?: PageWidgetOption[];
};
export type PageTab = Omit<WebTab, "widgets"> & {
  widgets: Record<string, PageWidget>;
  cols?: number;
};
export type PageModuleUI = Omit<
  WebModuleUI,
  "widgets" | "tabs" | "vertical"
> & {
  widgets: Record<string, PageWidget>;
  tabs?: PageTab[];
  vertical?: Omit<PageModuleUI, "script" | "vertical" | "tabsAs">;
  tabsAs?: "column" | "dropdown";
};

/** The module type folders as VALUES; the generated `WebModuleCategory` keeps the list honest — docs/graph.md. */
export const MODULE_CATEGORIES = [
  "io",
  "instruments",
  "audio",
  "transformers",
  "midi", // the legacy alias of transformers
  "video_effects",
  "video_transitions",
  "shaders",
  "shapes",
] as const satisfies readonly WebModuleCategory[];

/** How a routing graph names a PORT: `crossfade#b`, a third separator for a third axis — docs/graph.md. */
export const PORT_SEP = "#";

/** A ref split into node and port; only the LAST `#` is the port, so the others are left alone. */
export function splitPort(ref: string): { node: string; port?: string } {
  const cut = ref.lastIndexOf(PORT_SEP);
  if (cut <= 0) return { node: ref }; // no separator, or a ref that IS one — not a port
  const port = ref.slice(cut + 1);
  return port ? { node: ref.slice(0, cut), port } : { node: ref };
}

/** …and back; a port-less ref is the node's own name unchanged, which is the overwhelming case. */
export const portRef = (node: string, port?: string): string =>
  port ? `${node}${PORT_SEP}${port}` : node;

/** The two outputs every LANE offers, audio FIRST because the order is load-bearing — see docs/graph.md. */
export const TRACK_OUT_AUDIO = "audio";
export const TRACK_OUT_MIDI = "midi";
export const TRACK_OUTPUTS = [
  { name: TRACK_OUT_AUDIO, type: "audio" },
  { name: TRACK_OUT_MIDI, type: "midi" },
] as const satisfies readonly { name: string; type: WebSignal }[];

/** One automation LANE as the timeline draws it, derived from the graph by `automationLanes` — docs/graph.md. */
export interface WebAutomationLane {
  track: string; // the lane it draws under — the chain's own name
  module: string; // which node in that chain owns the knob
  param: string;
  curve: string; // the curve id, which is what a later edit will address
  /** Song timecodes and 0..1 values verbatim; empty for an LFO or ADSR, whose row says only that it is bound. */
  points: { time: string; value: number }[];
}

/** Every automation lane by track — the join done ONCE, since two surfaces would disagree; docs/graph.md. */
export function automationLanes(
  graph: Pick<WebGraph, "chains" | "curves">,
): Record<string, WebAutomationLane[]> {
  const out: Record<string, WebAutomationLane[]> = {};
  for (const c of Object.values(graph.chains ?? {})) {
    const track = c.from;
    if (!track) continue;
    for (const s of c.use)
      for (const a of s.automations ?? []) {
        const curve = graph.curves?.[a.curve];
        (out[track] ??= []).push({
          track,
          module: s.module,
          param: a.param,
          curve: a.curve,
          points: curve?.points ?? [],
        });
      }
  }
  return out;
}
