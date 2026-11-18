// relaySocket.test.ts — the WebSocket half of the relay over its lifetime, on real sockets: a
// browser, the relay and a fake engine on 127.0.0.1. Either end closing closes the other; a socket
// stays on the engine it was dialled to while a new one follows the registry; an engine gone from
// the registry is refused; frames keep their order through the dial and the claim. docs/proxy.md.

import type { AddressInfo } from "node:net";
import { MAX_FRAME_BYTES } from "@studio/engine-tunnel/frames";
import { afterEach, beforeEach, expect, it } from "vitest";
import { WebSocket, WebSocketServer } from "ws";
import {
  type CoreTarget,
  describeCore,
  REGISTRY_TTL_MS,
  startCore,
} from "./testCore.ts";

/** A fake engine: answers the relay's pairing frame, records the rest, answers `whoami`. */
interface FakeEngine {
  wss: WebSocketServer;
  port: number;
  /** Every frame after the pairing one, text as its `type`, binary as `bin:<bytes>`. */
  got: string[];
  /** The relay's sockets, in the order they connected. */
  peers: WebSocket[];
}

async function fakeEngine(name: string): Promise<FakeEngine> {
  const wss = new WebSocketServer({ port: 0, host: "127.0.0.1" });
  const e: FakeEngine = { wss, port: 0, got: [], peers: [] };
  wss.on("connection", (ws) => {
    let first = true;
    e.peers.push(ws);
    ws.on("message", (d, binary) => {
      if (first) {
        first = false;
        ws.send(JSON.stringify({ type: "paired" }));
        return;
      }
      if (binary) return void e.got.push(`bin:${(d as Buffer).length}`);
      const m = JSON.parse(String(d));
      e.got.push(m.n === undefined ? m.type : `${m.type}${m.n}`);
      if (m.type === "whoami")
        ws.send(JSON.stringify({ type: "whoami", id: "e1", name }));
    });
  });
  await new Promise((r) => wss.once("listening", r));
  e.port = (wss.address() as AddressInfo).port;
  return e;
}

const until = async (ok: () => boolean) => {
  for (let i = 0; i < 100 && !ok(); i++)
    await new Promise((r) => setTimeout(r, 20));
  expect(ok()).toBe(true);
};

describeCore("the relay's socket over its lifetime", () => {
  let core: CoreTarget;
  let engines: FakeEngine[];
  const clients: WebSocket[] = [];

  const register = (e: FakeEngine) =>
    core.reg.upsert({
      id: "e1",
      token: "tok",
      hostname: "h",
      machineFingerprint: "",
      addresses: { loopback: "127.0.0.1", lanIps: [], port: e.port },
      publicIpHint: "127.0.0.1",
    });

  beforeEach(async () => {
    engines = [await fakeEngine("first")];
    core = await startCore();
    await register(engines[0]);
  });
  afterEach(async () => {
    core.clock.real();
    for (const c of clients.splice(0)) c.terminate();
    for (const e of engines) {
      for (const c of e.wss.clients) c.terminate();
      await new Promise((r) => e.wss.close(r));
    }
    await core.stop();
  });

  /** A browser's socket as account `holder`, with what it read: text as its `type` or `whoami:<name>`. */
  const browser = async (
    holder = "ann@x",
  ): Promise<{ ws: WebSocket; seen: string[]; opened: Promise<void> }> => {
    const ticket = encodeURIComponent(await core.ticket("e1", holder));
    const ws = new WebSocket(
      `ws://127.0.0.1:${core.port}/api/engine/e1/ws?ticket=${ticket}`,
    );
    clients.push(ws);
    const seen: string[] = [];
    ws.on("message", (d, binary) => {
      if (binary) return void seen.push(`bin:${(d as Buffer).length}`);
      const m = JSON.parse(String(d));
      seen.push(m.type === "whoami" ? `whoami:${m.name}` : m.type);
    });
    ws.on("error", () => {});
    const opened = new Promise<void>((r, j) => {
      ws.once("open", () => r());
      ws.once("unexpected-response", (_q, res) =>
        j(new Error(`refused ${res.statusCode}`)),
      );
    });
    return { ws, seen, opened };
  };

  /** A browser whose relay has dialled its engine: the probe answered. */
  const connected = async (holder = "ann@x") => {
    const b = await browser(holder);
    await b.opened;
    b.ws.send(JSON.stringify({ type: "whoami" }));
    await until(() => b.seen.some((s) => s.startsWith("whoami")));
    return b;
  };

  const closeOf = (ws: WebSocket): Promise<number> =>
    new Promise((r) => ws.once("close", (code) => r(code)));

  it("closes the engine's socket when the browser closes its own", async () => {
    const { ws } = await connected();
    const up = engines[0].peers[0];
    const gone = closeOf(up);
    ws.close();
    await gone;
  });

  it("closes the browser's socket when the engine closes its own", async () => {
    const { ws } = await connected();
    const gone = closeOf(ws);
    engines[0].peers[0].close();
    await gone;
    expect(ws.readyState).toBe(WebSocket.CLOSED);
  });

  it("closes the browser's socket when the engine's drops without a close frame", async () => {
    const { ws } = await connected();
    const gone = closeOf(ws);
    engines[0].peers[0].terminate();
    await gone;
  });

  it("keeps an open socket on the engine it dialled when the engine re-registers elsewhere; a new one reaches the new address", async () => {
    await core.reg.claim("e1", { project: null, holder: "ann@x" });
    const old = await connected();
    expect(old.seen).toEqual(["whoami:first"]);
    engines.push(await fakeEngine("second"));
    await register(engines[1]);
    const fresh = await connected();
    expect(fresh.seen).toEqual(["whoami:second"]);
    old.ws.send(JSON.stringify({ type: "transport" }));
    await until(() => engines[0].got.includes("transport"));
    expect(engines[1].got).toEqual(["whoami"]);
  });

  it("refuses a new socket with 404 once the engine's heartbeat lapsed, and stops an open one driving it", async () => {
    await core.reg.claim("e1", { project: null, holder: "ann@x" });
    const open = await connected();
    core.clock.fake();
    await core.clock.advance(REGISTRY_TTL_MS);
    await expect((await browser()).opened).rejects.toThrow("refused 404");
    open.ws.send(JSON.stringify({ type: "transport" }));
    await new Promise((r) => setTimeout(r, 300));
    expect(engines[0].got).toEqual(["whoami"]);
  });

  it("closes a browser that sends a frame past MAX_FRAME_BYTES, and the engine's socket with it", async () => {
    const { ws } = await connected();
    const code = closeOf(ws);
    const up = closeOf(engines[0].peers[0]);
    ws.send(Buffer.alloc(MAX_FRAME_BYTES + 1));
    // 1009 when the close frame outruns the teardown, 1006 when acceptSocket's terminate wins: both are an end.
    expect([1009, 1006]).toContain(await code);
    await up;
    expect(engines[0].got).toEqual(["whoami"]);
  });

  it("refuses an engine frame past MAX_FRAME_BYTES rather than relaying it", async () => {
    await core.reg.claim("e1", { project: null, holder: "ann@x" });
    const b = await connected();
    const code = closeOf(b.ws);
    engines[0].peers[0].send(Buffer.alloc(MAX_FRAME_BYTES + 1));
    await Promise.race([code, new Promise((r) => setTimeout(r, 1000))]);
    expect(b.seen).not.toContain(`bin:${MAX_FRAME_BYTES + 1}`);
    expect(b.ws.readyState).not.toBe(WebSocket.OPEN);
  });

  it("delivers in the order sent what a browser sends before the dial and before its claim", async () => {
    const b = await browser();
    await b.opened;
    const sent = [
      "whoami",
      "hello",
      ...Array.from({ length: 30 }, (_, n) => `transport${n}`),
      "whoami",
    ];
    for (const s of sent) {
      const m = /^(\D+)(\d+)$/.exec(s);
      b.ws.send(
        JSON.stringify(m ? { type: m[1], n: Number(m[2]) } : { type: s }),
      );
    }
    await until(() => engines[0].got.length === 1);
    await new Promise((r) => setTimeout(r, 100));
    expect(engines[0].got).toEqual(["whoami"]);
    await core.reg.claim("e1", { project: null, holder: "ann@x" });
    await until(() => engines[0].got.length === sent.length);
    expect(engines[0].got).toEqual(sent);
  });
});
