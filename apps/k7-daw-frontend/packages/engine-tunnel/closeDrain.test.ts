// closeDrain.test.ts — a channel's close delivers what was queued ahead of it, on both agents: the hub's
// last frames reach the engine, in order, and the engine's last frames reach the hub. A stuck engine
// still lets the channel go within the bound (CLOSE_DRAIN_MS), and a loopback client gone before its
// channel opened has its frames delivered too — docs/tunnel.md.

import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { type AddressInfo, createServer, type Server } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import { WebSocket, WebSocketServer } from "ws";
import { CLOSE_DRAIN_MS } from "./drain.ts";
import { type AgentHandle, agentKinds } from "./testAgents.ts";
import { describeCore, type HubTarget, startHub, until } from "./testCore.ts";

const TOKEN = "tok";
const WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/** An engine that reads every frame and never answers a close: the agent has to let go by itself. */
async function stuckEngine(): Promise<{
  server: Server;
  port: number;
  texts: string[];
  closeAt: () => number | null;
  endAt: () => number | null;
}> {
  const texts: string[] = [];
  let closeAt: number | null = null;
  let endAt: number | null = null;
  const server = createServer((sock) => {
    let buf = Buffer.alloc(0);
    let upgraded = false;
    sock.on("data", (d) => {
      buf = Buffer.concat([buf, d]);
      if (!upgraded) {
        const end = buf.indexOf("\r\n\r\n");
        if (end < 0) return;
        const key = /sec-websocket-key:\s*(\S+)/i.exec(
          buf.subarray(0, end).toString(),
        )?.[1];
        const accept = createHash("sha1")
          .update(`${key}${WS_GUID}`)
          .digest("base64");
        sock.write(
          `HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`,
        );
        upgraded = true;
        buf = buf.subarray(end + 4);
      }
      for (;;) {
        if (buf.length < 2) return;
        const op = buf[0] & 0x0f;
        let len = buf[1] & 0x7f;
        let at = 2;
        if (len === 126) {
          if (buf.length < 4) return;
          len = buf.readUInt16BE(2);
          at = 4;
        } else if (len === 127) {
          if (buf.length < 10) return;
          len = Number(buf.readBigUInt64BE(2));
          at = 10;
        }
        const masked = (buf[1] & 0x80) !== 0;
        const mask = masked ? buf.subarray(at, at + 4) : null;
        at += masked ? 4 : 0;
        if (buf.length < at + len) return;
        const payload = Buffer.from(buf.subarray(at, at + len));
        if (mask) for (let i = 0; i < len; i++) payload[i] ^= mask[i % 4];
        buf = buf.subarray(at + len);
        if (op === 0x1) texts.push(payload.toString("utf8"));
        if (op === 0x8 && closeAt === null) closeAt = Date.now();
      }
    });
    sock.on("error", () => {});
    sock.on("close", () => {
      endAt = Date.now();
    });
  });
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  return {
    server,
    port: (server.address() as AddressInfo).port,
    texts,
    closeAt: () => closeAt,
    endAt: () => endAt,
  };
}

describeCore.each(agentKinds)(
  "a channel's close drains — $name agent",
  ({ make }) => {
    let hub: HubTarget | null = null;
    let agent: AgentHandle | null = null;
    const closers: (() => void)[] = [];

    afterEach(async () => {
      agent?.stop();
      for (const c of closers.splice(0)) c();
      await hub?.stop();
      hub = null;
      agent = null;
    });

    const startAgent = async (enginePort: number, cache: string) => {
      hub = await startHub({ verify: { id: "e1", token: TOKEN } });
      agent = make({
        registryUrl: hub.agentUrl,
        id: "e1",
        token: TOKEN,
        enginePort,
        cacheDir: cache,
        log: () => {},
      });
      agent.start();
      await until(() => hub?.has("e1") ?? false);
      return hub;
    };

    it("delivers the hub's last frames to the engine, in order, before closing it", async () => {
      const remote = mkdtempSync(join(tmpdir(), "drain-remote-"));
      const cache = mkdtempSync(join(tmpdir(), "drain-cache-"));
      mkdirSync(join(remote, "song"), { recursive: true });
      writeFileSync(join(remote, "song", "a.wav"), "RIFF-a".repeat(50_000));
      const got: string[] = [];
      let closedWith: number | null = null;
      const engine = new WebSocketServer({ port: 0, host: "127.0.0.1" });
      closers.push(() => engine.close());
      engine.on("connection", (s: WebSocket) => {
        s.on("message", (d) => got.push(d.toString()));
        s.on("close", () => {
          closedWith = got.length;
        });
      });
      await new Promise((r) => engine.once("listening", r));
      const h = await startAgent((engine.address() as AddressInfo).port, cache);
      const ch = await h.openReady("e1", [
        { remote, name: "ws", writable: false },
      ]);
      if (!ch) throw new Error("no channel");
      // The first frame names a file, so it waits on a download while the rest queue behind it.
      ch.send(
        JSON.stringify({ type: "project", file: `${remote}/song/a.wav` }),
      );
      ch.send(JSON.stringify({ type: "transport", action: "stop" }));
      ch.send(JSON.stringify({ type: "drop-session" }));
      ch.close();
      await until(() => closedWith !== null, CLOSE_DRAIN_MS + 2000);
      expect(got.map((g) => JSON.parse(g).type)).toEqual([
        "project",
        "transport",
        "drop-session",
      ]);
      expect(closedWith).toBe(3);
    });

    it("delivers the engine's last frames to the hub before the channel's close", async () => {
      const remote = mkdtempSync(join(tmpdir(), "drain-remote-"));
      const cache = mkdtempSync(join(tmpdir(), "drain-cache-"));
      let engineSock = null as WebSocket | null;
      const engine = new WebSocketServer({ port: 0, host: "127.0.0.1" });
      closers.push(() => engine.close());
      engine.on("connection", (s: WebSocket) => {
        engineSock = s;
      });
      await new Promise((r) => engine.once("listening", r));
      const h = await startAgent((engine.address() as AddressInfo).port, cache);
      const ch = await h.openReady("e1", [
        { remote, name: "ws", writable: true },
      ]);
      if (!ch) throw new Error("no channel");
      await until(() => engineSock !== null);
      const got: string[] = [];
      let closedWith: number | null = null;
      ch.on("message", (d: Buffer) => got.push(d.toString()));
      ch.on("close", () => {
        closedWith = got.length;
      });
      const local = join(cache, "ws", "take.wav");
      mkdirSync(join(cache, "ws"), { recursive: true });
      writeFileSync(local, "RIFF-take".repeat(50_000));
      // The first frame names a file the engine wrote, so it waits on an upload while the engine closes.
      engineSock?.send(JSON.stringify({ type: "recorded", file: local }));
      engineSock?.send(JSON.stringify({ type: "stopped" }));
      engineSock?.close();
      await until(() => closedWith !== null, CLOSE_DRAIN_MS + 2000);
      expect(got.map((g) => JSON.parse(g).type)).toEqual([
        "recorded",
        "stopped",
      ]);
      expect(JSON.parse(got[0]).file).toBe(`${remote}/take.wav`);
    });

    it("delivers what a loopback client sent before its channel opened, then closes", async () => {
      const cache = mkdtempSync(join(tmpdir(), "drain-cache-"));
      const got: string[] = [];
      let closedWith: number | null = null;
      const engine = new WebSocketServer({ port: 0, host: "127.0.0.1" });
      closers.push(() => engine.close());
      engine.on("connection", (s: WebSocket) => {
        s.on("message", (d) => got.push(d.toString()));
        s.on("close", () => {
          closedWith = got.length;
        });
      });
      await new Promise((r) => engine.once("listening", r));
      const h = await startAgent((engine.address() as AddressInfo).port, cache);
      const path = await h.bindLocal("e1", []);
      const client = new WebSocket(`ws://127.0.0.1:${h.port}${path}`);
      await new Promise((r) => client.once("open", r));
      client.send(JSON.stringify({ type: "transport", action: "stop" }));
      client.send(JSON.stringify({ type: "drop-session" }));
      client.close();
      await until(() => closedWith !== null, CLOSE_DRAIN_MS + 2000);
      expect(got.map((g) => JSON.parse(g).type)).toEqual([
        "transport",
        "drop-session",
      ]);
      expect(closedWith).toBe(2);
    });

    it("lets go of an engine that never answers the close, within the bound", async () => {
      const cache = mkdtempSync(join(tmpdir(), "drain-cache-"));
      const engine = await stuckEngine();
      closers.push(() => engine.server.close());
      const h = await startAgent(engine.port, cache);
      const ch = await h.openReady("e1");
      if (!ch) throw new Error("no channel");
      ch.send(JSON.stringify({ type: "transport", action: "stop" }));
      ch.send(JSON.stringify({ type: "drop-session" }));
      const closedAt = Date.now();
      ch.close();
      await until(() => engine.endAt() !== null, CLOSE_DRAIN_MS + 3000);
      expect(engine.texts.map((t) => JSON.parse(t).type)).toEqual([
        "transport",
        "drop-session",
      ]);
      expect(engine.closeAt()).not.toBeNull();
      expect((engine.endAt() ?? 0) - closedAt).toBeLessThan(
        CLOSE_DRAIN_MS + 1000,
      );
    }, 15_000);
  },
);
