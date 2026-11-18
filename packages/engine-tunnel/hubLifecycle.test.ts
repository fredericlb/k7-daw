// hubLifecycle.test.ts — a tunnel over its lifetime, on real sockets: the reference agent dialling
// a hub, a fake engine behind it, and local clients on bound paths. A drop ends every channel the
// tunnel carried and the agent's redial serves new ones; the newest verified tunnel wins; a
// refused one changes nothing; a bound path answers 503 with no tunnel and 403 off loopback.

import { mkdtempSync } from "node:fs";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, expect, it } from "vitest";
import { WebSocket, WebSocketServer } from "ws";
import { TunnelAgent } from "./agent.ts";
import { CLOSED } from "./channel.ts";
import {
  TUNNEL_PATH,
  TUNNEL_SECRET_HEADER,
  TUNNEL_TOKEN_HEADER,
} from "./frames.ts";
import {
  type Chan,
  type Dial,
  describeCore,
  type HubTarget,
  NO_LAN_PEER,
  startHub,
  until,
} from "./testCore.ts";

const TOKEN = "tok";
const SECRET = "s".repeat(64);

const settle = (ms = 150) => new Promise((r) => setTimeout(r, ms));

/** A fake engine recording each frame as text, with the agent's sockets in connection order. */
async function fakeEngine() {
  const wss = new WebSocketServer({ port: 0, host: "127.0.0.1" });
  const got: string[] = [];
  const peers: WebSocket[] = [];
  wss.on("connection", (s) => {
    peers.push(s);
    s.on("message", (d) => got.push(d.toString()));
  });
  await new Promise((r) => wss.once("listening", r));
  return { wss, got, peers, port: (wss.address() as AddressInfo).port };
}
type FakeEngine = Awaited<ReturnType<typeof fakeEngine>>;

describeCore("a tunnel over its lifetime", () => {
  let hub: HubTarget;
  let port: number;
  let engines: FakeEngine[];
  const agents: TunnelAgent[] = [];
  const clients: WebSocket[] = [];

  beforeEach(async () => {
    engines = [];
    hub = await startHub({
      verify: { id: "e1", token: TOKEN, secret: SECRET },
    });
    port = hub.port;
  });
  afterEach(async () => {
    for (const a of agents.splice(0)) a.stop();
    for (const c of clients.splice(0)) c.terminate();
    for (const e of engines) {
      for (const c of e.wss.clients) c.terminate();
      await new Promise((r) => e.wss.close(r));
    }
    await hub.stop();
  });

  /** The reference agent for e1, in front of a fresh fake engine. */
  const startAgent = async (): Promise<{
    agent: TunnelAgent;
    engine: FakeEngine;
  }> => {
    const engine = await fakeEngine();
    engines.push(engine);
    const agent = new TunnelAgent({
      registryUrl: hub.agentUrl,
      id: "e1",
      token: TOKEN,
      tunnelSecret: SECRET,
      enginePort: engine.port,
      cacheDir: mkdtempSync(join(tmpdir(), "hub-lifecycle-")),
      log: () => {},
    });
    agents.push(agent);
    agent.start();
    return { agent, engine };
  };

  const channel = async (): Promise<Chan> => {
    const ch = await hub.openReady("e1");
    if (!ch) throw new Error("no channel");
    return ch;
  };

  /** A local client on `path`: the open socket, or the refusal's status. */
  const local = (
    path: string,
    as: Dial = { host: "127.0.0.1" },
  ): Promise<WebSocket | number> =>
    new Promise((resolve) => {
      const ws = new WebSocket(`ws://${as.host}:${port}${path}`, {
        headers: as.headers,
        localAddress: as.localAddress,
      });
      clients.push(ws);
      ws.once("open", () => resolve(ws));
      ws.once("unexpected-response", (_q, r) => resolve(r.statusCode ?? 0));
      ws.once("error", () => resolve(0));
    });

  /** A bare supervisor's upgrade with `secret`: the open socket, or the refusal's status. */
  const bareTunnel = (secret: string): Promise<WebSocket | number> =>
    new Promise((resolve) => {
      const ws = new WebSocket(`ws://127.0.0.1:${port}${TUNNEL_PATH}?id=e1`, {
        headers: {
          [TUNNEL_TOKEN_HEADER]: TOKEN,
          [TUNNEL_SECRET_HEADER]: secret,
        },
      });
      clients.push(ws);
      ws.once("open", () => resolve(ws));
      ws.once("unexpected-response", (_q, r) => resolve(r.statusCode ?? 0));
      ws.once("error", () => resolve(0));
    });

  it("ends every channel of a tunnel that dropped, and serves new ones once the agent redialled", async () => {
    const { engine } = await startAgent();
    await until(() => hub.has("e1"));
    const before = await channel();
    before.send("one");
    await until(() => engine.got.length === 1);
    const ended = new Promise((r) => before.once("close", r));
    const engineSide = new Promise((r) => engine.peers[0].once("close", r));
    hub.cut();
    await ended;
    await engineSide;
    expect(await hub.has("e1")).toBe(false);
    expect(await hub.openReady("e1")).toBeNull();
    await until(() => hub.has("e1"));
    expect(before.readyState).toBe(CLOSED);
    before.send("lost");
    const after = await channel();
    after.send("two");
    await until(() => engine.got.length === 2);
    await settle();
    expect(engine.got).toEqual(["one", "two"]);
  });

  it("closes a local client when its tunnel drops, and serves the same bound path again after the redial", async () => {
    const { engine } = await startAgent();
    await until(() => hub.has("e1"));
    const path = await hub.bindLocal("e1", []);
    const first = await local(path);
    if (!(first instanceof WebSocket)) throw new Error(`refused: ${first}`);
    const gone = new Promise((r) => first.once("close", r));
    hub.cut();
    await gone;
    expect(await local(path)).toBe(503);
    await until(() => hub.has("e1"));
    const again = await local(path);
    if (!(again instanceof WebSocket)) throw new Error(`refused: ${again}`);
    again.send("back");
    await until(() => engine.got.includes("back"));
  });

  it("lets the newest verified tunnel win: the incumbent's channels end, new ones reach the newcomer's engine", async () => {
    const a = await startAgent();
    await until(() => hub.has("e1"));
    const old = await channel();
    const ended = new Promise((r) => old.once("close", r));
    const b = await startAgent();
    await ended;
    // The displaced agent would redial and win back in turn; a real machine runs one supervisor.
    a.agent.stop();
    await until(async () => (await hub.has("e1")) && b.agent.connected);
    const fresh = await channel();
    fresh.send("to-b");
    await until(() => b.engine.got.length === 1);
    expect(a.engine.got).toEqual([]);
  });

  it("refuses a tunnel with another secret and leaves the incumbent's channels working", async () => {
    const { engine } = await startAgent();
    await until(() => hub.has("e1"));
    const ch = await channel();
    expect(await bareTunnel("o".repeat(64))).toBe(403);
    ch.send("still");
    await until(() => engine.got.includes("still"));
    expect(ch.readyState).not.toBe(CLOSED);
  });

  it("answers 503 on a bound path while its engine has no tunnel, and 403 on a key never bound", async () => {
    const path = await hub.bindLocal("e1", []);
    expect(await local(path)).toBe(503);
    expect(await local(`${TUNNEL_PATH}/local/never-bound`)).toBe(403);
    await startAgent();
    await until(() => hub.has("e1"));
    expect(await local(path)).toBeInstanceOf(WebSocket);
  });

  it.skipIf(NO_LAN_PEER)(
    "answers 403 on a bound path to a peer that is not loopback, even with the tunnel up",
    async () => {
      await startAgent();
      await until(() => hub.has("e1"));
      const path = await hub.bindLocal("e1", []);
      expect(await local(path, hub.as("203.0.113.9"))).toBe(403);
      expect(await local(path, hub.as("::ffff:127.0.0.1"))).toBeInstanceOf(
        WebSocket,
      );
    },
  );

  it("answers 403 on a path whose binding was undone", async () => {
    await startAgent();
    await until(() => hub.has("e1"));
    const path = await hub.bindLocal("e1", []);
    await hub.unbindLocal(path);
    expect(await local(path)).toBe(403);
  });
});
