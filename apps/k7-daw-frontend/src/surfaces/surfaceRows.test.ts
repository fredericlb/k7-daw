import { describe, expect, it } from "vitest";
import { LAN_API, type LanHello, type LanPeer } from "./lanApi";
import { olderDevice, type PeerState, surfaceGroups } from "./surfaceRows";

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

const rows = (peers: PeerState[], extra: object = {}) =>
  surfaceGroups({
    peers,
    selfId: "me",
    selfName: "This Mac",
    drawable: () => true,
    origin: "http://127.0.0.1:18789",
    ...extra,
  });

describe("surfaceGroups", () => {
  it("makes one group per device with a row per surface, in the project's order", () => {
    const [g] = rows([{ peer: peer(), hello: hello() }]);
    expect(g.device).toBe("Studio");
    expect(g.project).toBe("band / mix.k7");
    expect(g.rows.map((r) => r.id)).toEqual(["desk", "arr"]);
    expect(g.rows[0].href).toBe(
      "http://127.0.0.1:18789/@lan/e1/band/mix.k7/@external/desk",
    );
  });

  it("sorts the devices by name", () => {
    const groups = rows([
      {
        peer: peer({ engineId: "b" }),
        hello: hello({ device: { ...hello().device, name: "Zed" } }),
      },
      {
        peer: peer({ engineId: "a" }),
        hello: hello({ device: { ...hello().device, name: "Amp" } }),
      },
    ]);
    expect(groups.map((g) => g.device)).toEqual(["Amp", "Zed"]);
  });

  it("drops this device's own engine", () => {
    expect(rows([{ peer: peer({ engineId: "me" }), hello: hello() }])).toEqual(
      [],
    );
  });

  it("draws a device only once its hello answered, and says so when it did not", () => {
    expect(rows([{ peer: peer(), hello: undefined }])).toEqual([]);
    expect(rows([{ peer: peer(), hello: null }])[0].blocked).toEqual({
      why: "notAnswering",
    });
    expect(
      rows([{ peer: peer({ reached: false }), hello: undefined }])[0].blocked,
    ).toEqual({
      why: "notAnswering",
    });
  });

  it("fails the trust before it reads the device's rows", () => {
    const [g] = rows([{ peer: peer({ trusted: false }), hello: hello() }]);
    expect(g.blocked).toEqual({ why: "notTrusted" });
    expect(g.rows).toEqual([]);
  });

  it("disables every row of an incompatible device, naming the older one", () => {
    const older = hello({ device: { ...hello().device, api: LAN_API - 1 } });
    const [g] = rows([{ peer: peer(), hello: older }]);
    expect(g.rows.every((r) => r.blocked?.why === "update" && !r.href)).toBe(
      true,
    );
    expect(g.rows[0].blocked).toEqual({ why: "update", device: "Studio" });
    const newer = hello({ device: { ...hello().device, api: LAN_API + 1 } });
    expect(rows([{ peer: peer(), hello: newer }])[0].rows[0].blocked).toEqual({
      why: "update",
      device: "This Mac",
    });
  });

  it("disables a type with no drawer here, after the compatibility", () => {
    const [g] = rows([{ peer: peer(), hello: hello() }], {
      drawable: (t: string) => t !== "timeline",
    });
    expect(g.rows[0].blocked).toBeUndefined();
    expect(g.rows[1].blocked).toEqual({ why: "windowOnly" });
    expect(g.rows[1].href).toBeUndefined();
  });

  it("lists the other devices already showing a surface", () => {
    const h = hello({
      screens: [
        { surface: "desk", device: "iPad", since: 1 },
        { surface: "desk", device: "Pixel", since: 2 },
      ],
    });
    const [g] = rows([{ peer: peer(), hello: h }]);
    expect(g.rows[0].alsoOn).toEqual(["iPad", "Pixel"]);
    expect(g.rows[1].alsoOn).toEqual([]);
  });

  it("keeps a device with no project, and one whose project declares nothing, as two empties", () => {
    const [none] = rows([
      { peer: peer(), hello: hello({ project: null, surfaces: [] }) },
    ]);
    expect(none.project).toBeNull();
    const [bare] = rows([{ peer: peer(), hello: hello({ surfaces: [] }) }]);
    expect(bare.project).toBe("band / mix.k7");
    expect(bare.rows).toEqual([]);
  });
});

describe("olderDevice", () => {
  it("says nothing when both speak the same", () => {
    expect(olderDevice(hello().device, "Me")).toBeNull();
  });
});
