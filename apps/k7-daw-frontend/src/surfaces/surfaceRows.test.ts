// The wiring of the Surfaces rows: Rust's groups gain their addresses and the page's locale order. The filters and their order are
// tested in the chains crate (apps/k7-daw-wasm/chains/src/surface_rows/tests.rs).

import { describe, expect, it } from "vitest";
import { LAN_API, type LanHello, type LanPeer } from "./lanApi";
import { type PeerState, surfaceGroups } from "./surfaceRows";

const peer = (over: Partial<LanPeer> = {}): LanPeer => ({
  engineId: "e1",
  name: "Studio",
  host: "10.0.0.2",
  door: 18790,
  engine: 8765,
  api: LAN_API,
  proto: 1,
  reached: true,
  trusted: true,
  ...over,
});

const hello = (over: Partial<LanHello> = {}): LanHello => ({
  device: {
    name: "Studio",
    engineId: "e1",
    build: null,
    api: LAN_API,
    proto: 1,
  },
  project: {
    workspace: "band",
    path: "mix.k7",
    label: "band / mix.k7",
    app: "k7",
  },
  surfaces: [
    { id: "desk", type: "turntables" },
    { id: "arr", type: "timeline" },
  ],
  screens: [],
  ...over,
});

const rows = (peers: PeerState[], drawable = (_type: string) => true) =>
  surfaceGroups({
    peers,
    selfId: "me",
    selfName: "This Mac",
    drawable,
    origin: "http://127.0.0.1:18789",
  });

describe("surfaceGroups", () => {
  it("spells each open row's address and leaves a blocked one without", () => {
    const [g] = rows(
      [{ peer: peer(), hello: hello() }],
      (t) => t !== "timeline",
    );
    expect(g.rows[0].href).toBe(
      "http://127.0.0.1:18789/@lan/e1/band/mix.k7/@external/desk",
    );
    expect(g.rows[1].blocked).toEqual({ why: "windowOnly" });
    expect(g.rows[1].href).toBeUndefined();
  });

  it("sorts the devices by name and keeps a device not yet answered out", () => {
    const named = (engineId: string, name: string): PeerState => ({
      peer: peer({ engineId }),
      hello: hello({ device: { ...hello().device, name } }),
    });
    const groups = rows([
      named("b", "Zed"),
      named("a", "Amp"),
      { peer: peer({ engineId: "c" }), hello: undefined },
      { peer: peer({ engineId: "d" }), hello: null },
    ]);
    expect(groups.map((g) => g.device)).toEqual(["Amp", "Studio", "Zed"]);
    expect(groups[1].blocked).toEqual({ why: "notAnswering" });
  });
});
