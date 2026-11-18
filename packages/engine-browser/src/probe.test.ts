// The probe's ordering of the answers one engine gave. A relay that measured faster than the
// engine's own loopback must not win: the gate takes unasked only an engine reached directly.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mayAutoPick } from "./choose.ts";
import {
  candidatesFor,
  orderHits,
  type ProbeTarget,
  probeEngines,
  reachedDirectly,
} from "./probe.ts";

const target: ProbeTarget = {
  id: "desk",
  addresses: { loopback: "127.0.0.1", lanIps: [], port: 8765 },
  proxyTicket: "ticket",
};

const hit = (
  url: string,
  direct: boolean,
  rttMs: number,
  reply: { token?: string; needsPairing?: boolean } = {},
) => ({
  c: { url, direct },
  reply: {
    token: reply.token ?? "t",
    needsPairing: reply.needsPairing ?? false,
    rttMs,
  },
});

const RELAY = "wss://k7.example/api/engines/proxy/desk/ws?ticket=ticket";
const LOOPBACK = "ws://127.0.0.1:8765";

describe("orderHits", () => {
  it("puts the direct socket first even when the relay measured faster", () => {
    const [best] = orderHits([hit(RELAY, false, 1), hit(LOOPBACK, true, 5)]);
    expect(best.c.url).toBe(LOOPBACK);
    // …which is what lets the gate take this machine's engine with nobody clicking.
    const ranked = { engine: target, wsUrl: best.c.url, token: "t" };
    expect(reachedDirectly(ranked)).toBe(true);
    expect(
      mayAutoPick({ ...ranked, latency: null, fileBase: null }, undefined),
    ).toBe(true);
  });

  it("still ranks by speed among sockets of the same kind", () => {
    const [best] = orderHits([
      hit("ws://192.168.1.20:8765", true, 9),
      hit(LOOPBACK, true, 2),
    ]);
    expect(best.c.url).toBe(LOOPBACK);
  });

  it("prefers a usable relay to a direct socket that withheld its token or needs pairing", () => {
    expect(
      orderHits([
        hit(LOOPBACK, true, 1, { token: "" }),
        hit(RELAY, false, 9),
      ])[0].c.url,
    ).toBe(RELAY);
    expect(
      orderHits([
        hit(LOOPBACK, true, 1, { needsPairing: true }),
        hit(RELAY, false, 9),
      ])[0].c.url,
    ).toBe(RELAY);
  });
});

// Node has no location: `candidatesFor` reads the page's protocol and href, and nothing else.
describe("candidatesFor and the WebRTC offer", () => {
  beforeEach(() => {
    vi.stubGlobal("location", {
      protocol: "https:",
      href: "https://app.k7.test/media",
    });
  });

  afterEach(() => vi.unstubAllGlobals());

  const dataChannels = (candidates: { url: string }[]) =>
    candidates.filter((c) => c.url.startsWith("rtc:"));

  it("offers no data channel before a claim, and one after it", () => {
    const tunnelled = { ...target, rtc: true };
    const listed = candidatesFor(tunnelled, "");
    const driving = candidatesFor(tunnelled, "", { claim: true });
    expect(dataChannels(listed)).toEqual([]);
    expect(dataChannels(driving)).toHaveLength(1);
  });

  it("offers none for an engine whose supervisor holds no tunnel", () => {
    const driving = candidatesFor(target, "", { claim: true });
    expect(dataChannels(driving)).toEqual([]);
  });
});

// k7's own engine is a container: what it lists are the container's addresses, which no browser reaches.
describe("candidatesFor for the server's own engine", () => {
  beforeEach(() => {
    vi.stubGlobal("location", {
      protocol: "https:",
      href: "https://app.k7.test/media",
    });
  });

  afterEach(() => vi.unstubAllGlobals());

  it("offers no direct address at all, only the relay", () => {
    const server = { ...target, server: true, rtc: true };
    const urls = candidatesFor(server, "", { claim: true }).map((c) => c.url);
    expect(urls.some((u) => u.startsWith("ws://"))).toBe(false);
    expect(urls.some((u) => u.startsWith("wss://"))).toBe(true);
  });
});

// The page's own machine is the one thing it can learn without the server: nothing answered there last time.
describe("a loopback that just refused", () => {
  /** Enough of a socket for `probeOne`: it refuses on the first microtask, and never opens. */
  class Refused {
    onopen: (() => void) | null = null;
    onmessage: ((ev: { data: string }) => void) | null = null;
    onerror: (() => void) | null = null;
    onclose: (() => void) | null = null;
    constructor(_url: string) {
      void Promise.resolve().then(() => this.onerror?.());
    }
    close(): void {}
    send(): void {}
  }

  beforeEach(() => {
    vi.stubGlobal("location", {
      protocol: "https:",
      href: "https://app.k7.test/media",
    });
    vi.stubGlobal("WebSocket", Refused);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("is left alone for two minutes, then offered again", async () => {
    const at = 1_000_000;
    const now = vi.spyOn(Date, "now").mockReturnValue(at);
    const urls = () => candidatesFor(target, "").map((c) => c.url);

    expect(urls()).toContain(LOOPBACK);
    await probeEngines([target], "");
    expect(urls()).not.toContain(LOOPBACK);
    now.mockReturnValue(at + 2 * 60_000 + 1);
    expect(urls()).toContain(LOOPBACK);
  });
});
