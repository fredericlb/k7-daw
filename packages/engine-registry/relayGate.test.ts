// relayGate.test.ts — a relay ticket is an account's, and it drives nothing until that account
// holds a claim: before it, the probe passes and everything else waits, then closes. Also the
// WebRTC offer, which reaches the engine's machine directly and so asks for the claim up front,
// and the engine token, which reaches through the relay only an account that may use the engine.

import type { AddressInfo } from "node:net";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { WebSocket, WebSocketServer } from "ws";
import { type CoreTarget, describeCore, startCore } from "./testCore.ts";

describeCore("the relay, end to end", () => {
  let core: CoreTarget;
  let engine: WebSocketServer;
  /** What the fake engine received after the relay's own pairing frame. */
  let got: string[];
  let toBrowser: WebSocket | null;

  beforeEach(async () => {
    got = [];
    toBrowser = null;
    engine = new WebSocketServer({ port: 0, host: "127.0.0.1" });
    engine.on("connection", (ws) => {
      let first = true;
      ws.on("message", (d) => {
        const m = JSON.parse(String(d));
        if (first) {
          first = false;
          ws.send(JSON.stringify({ type: "paired" }));
          toBrowser = ws;
          return;
        }
        got.push(m.type);
        if (m.type === "whoami")
          ws.send(JSON.stringify({ type: "whoami", id: "e1", token: "tok" }));
      });
    });
    await new Promise((r) => engine.once("listening", r));
    core = await startCore();
    await core.reg.upsert({
      id: "e1",
      token: "tok",
      hostname: "h",
      machineFingerprint: "",
      addresses: {
        loopback: "127.0.0.1",
        lanIps: [],
        port: (engine.address() as AddressInfo).port,
      },
      publicIpHint: "127.0.0.1",
    });
  });
  afterEach(async () => {
    for (const c of engine.clients) c.terminate();
    await new Promise((r) => engine.close(r));
    await core.stop();
  });

  const open = async (
    holder: string,
  ): Promise<{ ws: WebSocket; seen: string[] }> => {
    const ticket = encodeURIComponent(await core.ticket("e1", holder));
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(
        `ws://127.0.0.1:${core.port}/api/engine/e1/ws?ticket=${ticket}`,
      );
      const seen: string[] = [];
      ws.on("message", (d) => seen.push(JSON.parse(String(d)).type));
      ws.once("open", () => resolve({ ws, seen }));
      ws.once("error", reject);
    });
  };

  const until = async (ok: () => boolean) => {
    for (let i = 0; i < 100 && !ok(); i++)
      await new Promise((r) => setTimeout(r, 20));
    expect(ok()).toBe(true);
  };

  it("lets a non-holder probe, holds its driving, and lets it all through once it claims", async () => {
    const { ws, seen } = await open("bob");
    ws.send(JSON.stringify({ type: "whoami" }));
    ws.send(JSON.stringify({ type: "hello", role: "driver" }));
    await until(() => seen.includes("whoami"));
    expect(got).toEqual(["whoami"]);
    // The engine's pushes do not reach a socket with no claim behind it.
    toBrowser?.send(JSON.stringify({ type: "state", playing: true }));
    await new Promise((r) => setTimeout(r, 100));
    expect(seen).toEqual(["whoami"]);

    await core.reg.claim("e1", { project: null, holder: "bob" });
    await until(() => got.includes("hello"));
    toBrowser?.send(JSON.stringify({ type: "state", playing: true }));
    await until(() => seen.includes("state"));
    ws.close();
  });

  it("does not let another account's claim open the way", async () => {
    await core.reg.claim("e1", { project: null, holder: "alice" });
    const { ws } = await open("bob");
    ws.send(JSON.stringify({ type: "transport", action: "play" }));
    await new Promise((r) => setTimeout(r, 300));
    expect(got).not.toContain("transport");
    ws.close();
  });
});

describeCore("the WebRTC offer", () => {
  let core: CoreTarget;
  const signal = vi.fn(async () => "answer");

  beforeEach(async () => {
    signal.mockClear();
    core = await startCore({ user: "bob", signal });
    await core.reg.upsert({
      id: "e1",
      token: "tok",
      hostname: "h",
      machineFingerprint: "",
      addresses: { loopback: "127.0.0.1", lanIps: [], port: 8765 },
    });
  });
  afterEach(async () => {
    await core.stop();
  });

  const offer = () => core.post("/engine/e1/rtc", { sdp: "v=0" });

  it("is refused without a claim, and answered with one", async () => {
    expect((await offer()).status).toBe(403);
    expect(signal).not.toHaveBeenCalled();
    await core.reg.claim("e1", { project: null, holder: "alice" });
    expect((await offer()).status).toBe(403);
    await core.reg.claim("e1", { project: null, holder: "bob" });
    const r = await offer();
    expect(r.status).toBe(200);
    expect(r.body).toEqual({ sdp: "answer" });
  });

  it("mints each listed engine a ticket for the asking account, naming only its own claims", async () => {
    await core.reg.owners.assign("e1", "bob");
    await core.reg.owners.share("e1", ["alice"]);
    await core.reg.claim("e1", { project: "@local/ws/a.k7", holder: "alice" });
    const list = (await core.call("bob", "GET", "/engines")).body as {
      proxyTicket: string;
      heldBy?: string;
      assigned: boolean;
    }[];
    expect((await core.verifyTicket(list[0]?.proxyTicket ?? ""))?.holder).toBe(
      "bob",
    );
    expect(list[0]?.assigned).toBe(true);
    expect(list[0]?.heldBy).toBeUndefined();
  });
});
