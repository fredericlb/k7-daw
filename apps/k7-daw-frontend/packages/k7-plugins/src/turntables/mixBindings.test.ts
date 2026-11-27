// The lookups a binding makes against the engine's live state: the join itself is turntables' Rust (tests/bindings.rs).

import type { WebGraph, WebWidget } from "@studio/graph-model";
import { resolveBindings } from "@studio/k7-wasm/desk";
import type { MixView } from "@studio/k7-wasm/text";
import { describe, expect, it } from "vitest";
import {
  controlDriving,
  engineAgrees,
  leavesByItsOwnPair,
  shownValue,
} from "./mixBindings";

/** A graph with one chain holding the steps given, each addressable and drawn. */
const graphWith = (
  steps: {
    module: string;
    pluginId?: string;
    // The widget's own fields, minus the placement: `curve` and `step` are what the desk must honour.
    widget?: Partial<WebWidget> & { bind: string };
  }[],
): WebGraph =>
  ({
    tracks: ["deckA"],
    trackSignals: { deckA: "audio" },
    chains: {
      deckA: {
        from: "deckA",
        use: steps.map((s) => ({
          module: s.module,
          category: "audio",
          emits: "audio",
          enabled: true,
          pluginId: s.pluginId,
          ui: s.widget
            ? {
                cols: 1,
                rows: 1,
                widgets: { w: { type: "knob", col: 0, row: 0, ...s.widget } },
              }
            : undefined,
        })),
        out: [{ to: "OUT", gain: 1 }],
      },
    },
    warnings: [],
  }) as unknown as WebGraph;

const VIEW: MixView = { crossfade: { module: "crossfade", param: "Mix" } };

describe("reading a binding backwards, for the graph", () => {
  const tempoView: MixView = {
    "deckA.tempo": { module: "tempoA", param: "Target BPM" },
    "deckB.tempo": { module: "tempoB", param: "Target BPM" },
  };
  const graph = () => {
    const g = graphWith([{ module: "tempoA" }]);
    g.chains.deckB = {
      from: "deckB",
      use: [
        { module: "tempoB", category: "audio", emits: "audio", enabled: true },
      ],
      out: [{ to: "OUT", gain: 1 }],
    } as unknown as WebGraph["chains"][string];
    return g;
  };

  it("finds the control a module's param belongs to", () => {
    // A node knows nothing about decks, so the file's line is read the other way — docs/deckSet.md.
    const { bindings } = resolveBindings(tempoView, graph());
    expect(controlDriving(bindings, "tempoB", "Target BPM")?.control.id).toBe(
      "deckB.tempo",
    );
  });

  it("FOLLOWS the file, so re-pointing a line moves the graph's knob too", () => {
    // Deck A's fader aimed at deck B's module: the knob on `tempoB` is now deck A's control.
    const crossed: MixView = {
      "deckA.tempo": { module: "tempoB", param: "Target BPM" },
    };
    const { bindings } = resolveBindings(crossed, graph());
    expect(controlDriving(bindings, "tempoB", "Target BPM")?.control.id).toBe(
      "deckA.tempo",
    );
  });

  it("refuses a binding that HAS a plugin", () => {
    // That one has a socket of its own, and answering would give one param two routes to the engine.
    const { bindings } = resolveBindings(
      VIEW,
      graphWith([{ module: "crossfade", pluginId: "deckA:crossfade" }]),
    );
    expect(controlDriving(bindings, "crossfade", "Mix")).toBeUndefined();
  });

  it("knows nothing about a param nobody bound", () => {
    const { bindings } = resolveBindings(tempoView, graph());
    expect(controlDriving(bindings, "tempoA", "Source BPM")).toBeUndefined();
    expect(controlDriving(bindings, "nope", "Target BPM")).toBeUndefined();
  });
});

describe("what a control's position SHOWS", () => {
  const bound = () =>
    resolveBindings(
      VIEW,
      graphWith([{ module: "crossfade", pluginId: "deckA:crossfade" }]),
    ).bindings[0];
  const LIVE = { "deckA:crossfade": { Mix: 0.8 } };

  it("draws the ENGINE's value when no hand is on it", () => {
    // A per-surface copy left the graph drawing the file's number after a knob was turned in Mix.
    expect(shownValue(bound(), undefined, LIVE)).toBe(0.8);
  });

  it("lets the hand win while a gesture is in flight", () => {
    // Optimism, and only for the round trip: waiting for the engine would make every drag lag.
    expect(shownValue(bound(), 0.2, LIVE)).toBe(0.2);
  });

  it("knows nothing rather than guessing zero", () => {
    // Before the first frame: zero is a real value, and on a ±20 dB EQ it is the CENTRE.
    expect(shownValue(bound(), undefined, {})).toBeUndefined();
    expect(shownValue(undefined, undefined, LIVE)).toBeUndefined();
  });

  it("yields the override once the engine has caught up", () => {
    // An override that never yields is a control that has stopped listening.
    expect(engineAgrees(bound(), 0.8, LIVE)).toBe(true);
    expect(engineAgrees(bound(), 0.81, LIVE)).toBe(true); // within a hair
    expect(engineAgrees(bound(), 0.2, LIVE)).toBe(false);
    // Nothing heard yet is not agreement — the knob would snap back to whatever the file said.
    expect(engineAgrees(bound(), 0.8, {})).toBe(false);
  });
});

describe("leavesByItsOwnPair", () => {
  const b = {
    pluginId: "cueA:cueA",
  } as Parameters<typeof leavesByItsOwnPair>[0];
  const doc = (over: { outputChannels?: number[] }) => ({
    tracks: [{ id: "cueA", ...over }],
  });

  it("says yes to a cue bus that leaves by a pair this rig has", () => {
    expect(leavesByItsOwnPair(b, doc({ outputChannels: [3, 4] }))).toBe(true);
  });

  it("says NO to one this machine named no pair for", () => {
    // A machine with no `system cue`: loaded, bound, heard nowhere — a button that would lie.
    expect(leavesByItsOwnPair(b, doc({ outputChannels: [] }))).toBe(false);
    expect(leavesByItsOwnPair(b, doc({}))).toBe(false);
  });

  it("reads the CHANNELS, never the mute — a `bus_only` strip is muted and perfectly audible", () => {
    // The trap this replaced: keying on `mute` would have called eqA silent and killed the EQ knobs.
    const strip = { pluginId: "eqA:eqA" } as typeof b;
    const withStrip = {
      tracks: [{ id: "eqA", outputChannels: [], mute: true }],
    };
    expect(leavesByItsOwnPair(strip, withStrip)).toBe(false);
    // …which is why only the CUE control asks this question, and nothing else may.
  });

  it("says no when the document has no such track at all", () => {
    expect(leavesByItsOwnPair(b, { tracks: [] })).toBe(false);
    expect(leavesByItsOwnPair(b, null)).toBe(false);
  });
});
