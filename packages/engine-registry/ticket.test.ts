// ticket.test.ts — the relay ticket over its lifetime, as an upgrade sees it: good for one engine
// and one account for a minute, reusable within it, and nothing once the process that minted it
// is gone. The ticket is a bearer credential: the account comes from it alone. docs/proxy.md.
// Runs against the TypeScript relay, or the Rust one with ENGINE_CORE_BIN (testCore.ts).

import type { AddressInfo } from "node:net";
import { afterEach, beforeEach, expect, it } from "vitest";
import { WebSocket, WebSocketServer } from "ws";
import { type CoreTarget, describeCore, startCore } from "./testCore.ts";

const TTL_MS = 60_000;

describeCore("a ticket's lifetime, verified alone", () => {
  let core: CoreTarget;
  beforeEach(async () => {
    core = await startCore();
  });
  afterEach(async () => {
    core.clock.real();
    await core.stop();
  });

  it("holds for its minute, to the millisecond, and not after", async () => {
    core.clock.fake();
    await core.clock.set(1_000_000);
    const t = await core.ticket("e1", "ann@x");
    await core.clock.set(1_000_000 + TTL_MS);
    expect(await core.verifyTicket(t)).toEqual({ id: "e1", holder: "ann@x" });
    await core.clock.set(1_000_000 + TTL_MS + 1);
    expect(await core.verifyTicket(t)).toBeNull();
  });

  it("verifies any number of times within its minute: nothing spends it", async () => {
    const t = await core.ticket("e1", "ann@x");
    for (let i = 0; i < 3; i++)
      expect(await core.verifyTicket(t)).toEqual({ id: "e1", holder: "ann@x" });
  });

  it("is nothing when malformed", async () => {
    const [payload, sig] = (await core.ticket("e1", "ann@x")).split(".");
    for (const bad of [
      "",
      ".",
      "abc",
      `${payload}.`,
      `.${sig}`,
      `${sig}.${payload}`,
    ])
      expect(await core.verifyTicket(bad)).toBeNull();
  });

  it("is nothing to another process, whose secret is its own", async () => {
    const t = await core.foreignTicket("e1", "ann@x");
    expect(await core.verifyTicket(t)).toBeNull();
    expect(await core.verifyTicket(await core.ticket("e1", "ann@x"))).toEqual({
      id: "e1",
      holder: "ann@x",
    });
  });
});

describeCore("a ticket at the relay's upgrade", () => {
  let core: CoreTarget;
  let engine: WebSocketServer;
  /** What the fake engine received after the relay's pairing frame. */
  let got: string[];
  const sockets: WebSocket[] = [];

  beforeEach(async () => {
    got = [];
    engine = new WebSocketServer({ port: 0, host: "127.0.0.1" });
    engine.on("connection", (ws) => {
      let first = true;
      ws.on("message", (d) => {
        if (first) {
          first = false;
          ws.send(JSON.stringify({ type: "paired" }));
          return;
        }
        got.push(JSON.parse(String(d)).type);
      });
    });
    await new Promise((r) => engine.once("listening", r));
    const enginePort = (engine.address() as AddressInfo).port;
    core = await startCore();
    for (const id of ["e1", "e2"])
      await core.reg.upsert({
        id,
        token: "tok",
        hostname: "h",
        machineFingerprint: "",
        addresses: { loopback: "127.0.0.1", lanIps: [], port: enginePort },
        publicIpHint: "127.0.0.1",
      });
  });
  afterEach(async () => {
    core.clock.real();
    for (const s of sockets.splice(0)) s.terminate();
    for (const c of engine.clients) c.terminate();
    await new Promise((r) => engine.close(r));
    await core.stop();
  });

  /** Upgrade `/api/engine/<id>/ws` with `ticket`: the open socket, or the refusal's status. */
  const dial = (
    id: string,
    ticket: string,
    headers: Record<string, string> = {},
  ): Promise<WebSocket | number> =>
    new Promise((resolve) => {
      const ws = new WebSocket(
        `ws://127.0.0.1:${core.port}/api/engine/${id}/ws?ticket=${encodeURIComponent(ticket)}`,
        { headers },
      );
      sockets.push(ws);
      ws.once("open", () => resolve(ws));
      ws.once("unexpected-response", (_q, r) => resolve(r.statusCode ?? 0));
      ws.once("error", () => resolve(0));
    });

  const until = async (ok: () => boolean) => {
    for (let i = 0; i < 100 && !ok(); i++)
      await new Promise((r) => setTimeout(r, 20));
    expect(ok()).toBe(true);
  };

  it("opens only the engine it names: one for e1 is a 401 on e2", async () => {
    expect(await dial("e2", await core.ticket("e1", "ann@x"))).toBe(401);
    expect(await dial("e2", await core.ticket("e2", "ann@x"))).toBeInstanceOf(
      WebSocket,
    );
  });

  it("opens any number of sockets within its minute, and none after it", async () => {
    core.clock.fake();
    const t = await core.ticket("e1", "ann@x");
    expect(await dial("e1", t)).toBeInstanceOf(WebSocket);
    expect(await dial("e1", t)).toBeInstanceOf(WebSocket);
    await core.clock.advance(TTL_MS + 1);
    expect(await dial("e1", t)).toBe(401);
  });

  it("is a 401 when it comes from another process", async () => {
    expect(await dial("e1", await core.foreignTicket("e1", "ann@x"))).toBe(401);
  });

  it("drives as the account it names, whoever presents it and whatever else the upgrade carries", async () => {
    await core.reg.claim("e1", { project: null, holder: "ann@x" });
    const ws = await dial("e1", await core.ticket("e1", "ann@x"), {
      cookie: "sid=bob",
      "x-studio-user": "bob@x",
    });
    if (!(ws instanceof WebSocket)) throw new Error(`refused: ${ws}`);
    ws.send(JSON.stringify({ type: "transport", action: "play" }));
    await until(() => got.includes("transport"));
  });
});
