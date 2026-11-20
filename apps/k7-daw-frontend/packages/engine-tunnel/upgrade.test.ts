// upgrade.test.ts — a socket accepted through `acceptSocket` survives a peer's protocol fault by
// ending itself, and `heartbeat` drops a peer that stopped answering pings.

import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WebSocket, WebSocketServer } from "ws";
import { sendBadFrame } from "./testBadFrame.ts";
import { acceptSocket, heartbeat } from "./upgrade.ts";

let server: Server;
afterEach(async () => {
  server.closeAllConnections();
  await new Promise((r) => server.close(r));
});

async function serving(serve: (ws: WebSocket) => void): Promise<number> {
  const wss = new WebSocketServer({ noServer: true });
  server = createServer();
  server.on("upgrade", (req, socket, head) =>
    acceptSocket(wss, req, socket, head, serve),
  );
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
  return (server.address() as AddressInfo).port;
}

describe("acceptSocket", () => {
  it("terminates a socket that sent an RSV1 frame, even one already closing", async () => {
    const closed = vi.fn();
    const port = await serving((ws) => {
      ws.on("close", closed);
      ws.close(1008, "refused");
    });
    expect(await sendBadFrame(port, "/x")).toContain("101");
    await vi.waitFor(() => expect(closed).toHaveBeenCalled());
  });
});

describe("heartbeat", () => {
  it("terminates a peer that leaves a ping unanswered", async () => {
    // The server's own close is awaited: the client can hear the hang-up before the terminated socket reads CLOSED.
    let servedClosed!: Promise<WebSocket>;
    const port = await serving((ws) => {
      servedClosed = new Promise((r) => ws.once("close", () => r(ws)));
      heartbeat(ws, 30);
    });
    const client = new WebSocket(`ws://127.0.0.1:${port}/x`, {
      autoPong: false,
    });
    await new Promise((r) => client.on("close", r));
    expect((await servedClosed).readyState).toBe(WebSocket.CLOSED);
  });

  // The interval is faked and each round waits on its pong, so a loaded host cannot make a pong late.
  it("keeps a peer that answers", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    try {
      let served!: (ws: WebSocket) => void;
      const accepted = new Promise<WebSocket>((r) => {
        served = r;
      });
      const port = await serving((ws) => {
        heartbeat(ws, 20);
        served(ws);
      });
      const client = new WebSocket(`ws://127.0.0.1:${port}/x`);
      const opened = new Promise((r) => client.on("open", r));
      const ws = await accepted;
      await opened;
      for (let round = 0; round < 3; round++) {
        const pong = new Promise((r) => ws.once("pong", r));
        vi.advanceTimersByTime(20);
        await pong;
      }
      vi.advanceTimersByTime(20);
      expect(ws.readyState).toBe(WebSocket.OPEN);
      expect(client.readyState).toBe(WebSocket.OPEN);
      client.close();
    } finally {
      vi.useRealTimers();
    }
  });
});
