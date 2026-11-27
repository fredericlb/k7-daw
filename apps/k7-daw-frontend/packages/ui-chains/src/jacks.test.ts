// The sockets a node shows.
//
// Two rules that pull against each other, which is why they are worth pinning. A port that MUST be fed is
// something to say up front, so every mandatory one is drawn before anything is wired. An optional one is an
// offer, and a module with five of them would otherwise present five empty holes to a reader using two — so
// past the mandatory ones exactly one free socket appears, beyond the last occupied port.
//
// Outputs have no such progression: an output is offered, never required, so all of them show at once.
//
// See docs/graphBuild.md.

import type { WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import {
  buildGraph,
  cablePath,
  type GraphNode,
  jackPoint,
  layout,
  PORT_PLATE,
} from "./graph";

const AUDIO = "audio" as const;

/** Two audio inputs, the second optional — the crossfade of `@studio/song`'s docs/ports.md. */
const CROSSFADE = [
  { name: "a", type: AUDIO },
  { name: "b", type: AUDIO, mandatory: false },
];

describe("the sockets a built graph puts on its nodes", () => {
  // deckA flows into the crossfade, deckB lands on its `b` — demos/crossfade-demo.k7.
  const wired: WebGraph = {
    chains: {
      deckA: {
        from: "deckA",
        use: [{ module: "crossfade", inputs: CROSSFADE, emits: AUDIO }],
        out: [{ to: "OUT", gain: 1 }],
      },
      deckB: { from: "deckB", use: [], out: [{ to: "crossfade#b", gain: 1 }] },
    },
    tracks: ["deckA", "deckB"],
    warnings: [],
  };

  it("counts a ported cable against the port it names", () => {
    const { nodes } = buildGraph(wired);
    const xf = nodes.find((n) => n.id === "mod:crossfade");
    expect(xf?.jacks?.in).toEqual([
      { name: "a", type: AUDIO, free: false }, // deckA's flow
      { name: "b", type: AUDIO, free: false }, // deckB, by name
    ]);
    expect(xf?.jacks?.out).toHaveLength(1);
  });

  it("gives a lane TWO outputs, audio then midi", () => {
    // A lane offers both, and the ORDER is load-bearing twice over — see docs/graphBuild.md.
    const { nodes } = buildGraph({
      chains: {},
      tracks: ["deckB"],
      warnings: [],
    });
    expect(nodes.find((n) => n.id === "trk:deckB")?.jacks?.out).toEqual([
      { name: "audio", type: AUDIO, free: true },
      { name: "midi", type: "midi", free: true },
    ]);
  });

  it("gives a lane an input socket, and a SPARE beside the output that is taken", () => {
    // Without both, nothing can be routed into a lane and no second send is possible.
    const { nodes } = buildGraph(wired);
    const deck = nodes.find((n) => n.id === "trk:deckA");
    expect(deck?.jacks?.in).toEqual([{ type: AUDIO, free: true }]);
    // deckA is wired BARE, so the spare beside it is UNNAMED too. See docs/graphBuild.md.
    expect(deck?.jacks?.out).toEqual([
      { name: "audio", type: AUDIO, free: false },
      { type: AUDIO, free: true },
      { name: "midi", type: "midi", free: true },
    ]);
  });

  it("still leaves a sink with no output socket", () => {
    // Unchanged, and the mirror of a source: somewhere to arrive, nowhere to leave.
    const { nodes } = buildGraph(wired);
    expect(nodes.find((n) => n.id === "snk:OUT")?.jacks?.out).toEqual([]);
    expect(nodes.find((n) => n.id === "snk:OUT")?.jacks?.in).toHaveLength(1);
  });

  it("leaves a live INPUT with no input socket — nothing plugs into a preamp", () => {
    // `monitor` is what says "this is an input", set by the file service for an io "in" step alone.
    const { nodes } = buildGraph({
      chains: {
        live: {
          from: "live",
          // The marker whatever its VALUE, and a three-way (auto/off/on) rather than a boolean.
          use: [{ module: "IN", emits: AUDIO, monitor: "off" as const }],
          out: [{ to: "OUT", gain: 1 }],
        },
      },
      tracks: [],
      warnings: [],
    });
    const inNode = nodes.find((n) => n.id === "mod:IN");
    expect(inNode?.jacks?.in).toEqual([]);
    // …and it still has somewhere to leave from, or it could feed nothing.
    expect(inNode?.jacks?.out).toHaveLength(1);
  });

  it("does not let an automation arrow occupy an input", () => {
    // A curve drives a knob and plugs into nothing, so it occupies no jack.
    const { nodes } = buildGraph({
      chains: {
        a: {
          from: "t",
          use: [
            {
              module: "amp",
              emits: AUDIO,
              automations: [{ param: "Gain", curve: "sweep" }],
            },
          ],
          out: [{ to: "OUT", gain: 1 }],
        },
      },
      tracks: ["t"],
      curves: { sweep: { type: "custom" } },
      warnings: [],
    });
    // Fed by the track and by nothing else: one socket, occupied.
    expect(nodes.find((n) => n.id === "mod:amp")?.jacks?.in).toEqual([
      { type: AUDIO, free: false },
    ]);
  });

  it("carries the port through layout", () => {
    // `targetPort` is set on the raw edge, and `layout` has to carry it over — see docs/graphBuild.md.
    const { nodes, edges } = buildGraph(wired);
    const laid = layout(nodes, edges);
    // deckB runs no modules, so its chain's pass-through node is what the ported cable leaves.
    const side = laid.edges.filter((e) => e.target === "mod:crossfade");
    expect(side.map((e) => e.targetPort).filter(Boolean)).toEqual(["b"]);
  });
});

// A cable ends in a PLUG, so both ends are moved off the layout engine's border points and onto the sockets.
describe("where a cable plugs in", () => {
  const node = (over: Partial<GraphNode>): GraphNode =>
    ({
      id: "n",
      label: "n",
      kind: "module",
      dim: false,
      missing: false,
      stub: false,
      notSupported: false,
      unknownPlugin: false,
      hasNativePluginUI: false,
      w: 160,
      h: 70,
      x: 100,
      y: 200,
      ...over,
    }) as GraphNode;

  const twoIn = node({
    jacks: {
      in: [
        { name: "a", type: AUDIO, free: false },
        { name: "b", type: AUDIO, free: false },
      ],
      out: [{ type: AUDIO, free: false }],
    },
  });

  it("puts a socket where the renderer draws it", () => {
    // Half a plate outside the edge — top for an input, bottom for an output — spread across the width.
    expect(jackPoint(twoIn, "in", 0)).toEqual({
      x: 100 + 160 / 3,
      y: 200 - PORT_PLATE / 2,
    });
    expect(jackPoint(twoIn, "in", 1)).toEqual({
      x: 100 + (160 * 2) / 3,
      y: 200 - PORT_PLATE / 2,
    });
    expect(jackPoint(twoIn, "out", 0)).toEqual({
      x: 100 + 80,
      y: 200 + 70 + PORT_PLATE / 2,
    });
  });

  it("lands a ported cable on the socket it names, not on the first", () => {
    const src = node({
      id: "s",
      x: -200,
      y: 200,
      jacks: { in: [], out: [{ type: AUDIO, free: false }] },
    });
    const path = cablePath(
      {
        id: "e",
        source: "s",
        target: "n",
        targetPort: "b",
        points: [
          { x: -40, y: 235 },
          { x: 100, y: 235 },
        ],
      },
      src,
      twoIn,
    );
    expect(path[path.length - 1]).toEqual(jackPoint(twoIn, "in", 1));
    expect(path[0]).toEqual(jackPoint(src, "out", 0));
  });

  it("hangs from socket to socket — plugged in at both ends, sagging between them", () => {
    const src = node({
      id: "s",
      x: -200,
      y: 200,
      jacks: { in: [], out: [{ type: AUDIO, free: false }] },
    });
    const path = cablePath(
      {
        id: "e",
        source: "s",
        target: "n",
        points: [
          { x: -40, y: 235 },
          { x: 100, y: 235 },
        ],
      },
      src,
      twoIn,
    );
    // Both ends are the SOCKETS, not the border points the layout engine routed to.
    expect(path[0]).toEqual(jackPoint(src, "out", 0));
    expect(path[path.length - 1]).toEqual(jackPoint(twoIn, "in", 0));
    // …and between them it HANGS, the shape itself being hang.ts's and tested there.
    expect(path.length).toBeGreaterThan(2);
    const sockets = Math.max(path[0].y, path[path.length - 1].y);
    expect(Math.max(...path.map((p) => p.y))).toBeGreaterThan(sockets);
  });

  it("drops the layout engine's waypoints — a patch bay's cables do not skirt anything", () => {
    // Kept, they bend a send around the module it skips. See docs/graphBuild.md.
    const bend = { x: 20, y: 400 };
    const path = cablePath(
      {
        id: "e",
        source: "s",
        target: "n",
        points: [{ x: -40, y: 235 }, bend, { x: 100, y: 235 }],
      },
      undefined,
      twoIn,
    );
    expect(path).not.toContainEqual(bend);
  });

  it("falls back to the layout engine's own end when a node is missing", () => {
    const pts = [
      { x: 0, y: 0 },
      { x: 10, y: 10 },
    ];
    const path = cablePath(
      { id: "e", source: "?", target: "?", points: pts },
      undefined,
      undefined,
    );
    expect(path[0]).toEqual(pts[0]);
    expect(path[path.length - 1]).toEqual(pts[1]);
  });
});

describe("several cables into one port-less node", () => {
  // Four lanes meeting an EQ, the shape a deck's stem cuts make — @studio/song's docs/setProject.md.
  const meeting: WebGraph = {
    chains: {
      deckA: { from: "deckA", use: [], out: [{ to: "eqA", gain: 1 }] },
      drums: { from: "drums", use: [], out: [{ to: "eqA", gain: 1 }] },
      bass: { from: "bass", use: [], out: [{ to: "eqA", gain: 1 }] },
      eqA: {
        from: "eqA",
        use: [{ module: "eqA", emits: AUDIO }],
        out: [{ to: "OUT", gain: 1 }],
      },
    },
    tracks: ["deckA", "drums", "bass"],
    warnings: [],
  };

  it("gives each arriving cable a socket of its own", () => {
    // Purely drawing: three cables knotted on one hole read as one, and the picture is how this graph is understood.
    const { nodes } = buildGraph(meeting);
    const eq = nodes.find((n) => n.id === "mod:eqA");
    // FOUR, not three: a bus chain starts at its own meeting point, so its own lane is a cable too.
    expect(eq?.jacks?.in).toHaveLength(4);
    expect(eq?.jacks?.in.every((j) => !j.free)).toBe(true);
  });

  it("ranks them in the graph's own order, so a cable keeps its socket", () => {
    // A shuffling picture would be worse than a knot: the eye would follow a cable that moved.
    const { edges } = buildGraph(meeting);
    const into = edges.filter((e) => e.target === "mod:eqA");
    expect(into.map((e) => e.targetSlot)).toEqual([0, 1, 2, 3]);
  });
});
