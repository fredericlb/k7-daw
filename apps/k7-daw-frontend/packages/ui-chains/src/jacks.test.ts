// jacks.test.ts — where a cable plugs in: the sockets' places and the cables hung between them, which are the page's drawing geometry.
// The sockets a node shows (which are free, which are named) are the `chains` crate's: apps/k7-daw-wasm/chains/src/tests/jacks.rs.

import { describe, expect, it } from "vitest";
import { cablePath, type GraphNode, jackPoint, PORT_PLATE } from "./graph";

const AUDIO = "audio" as const;

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
