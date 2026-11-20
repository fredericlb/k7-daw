// protocol.test.ts — a supervisor states the engine wire's version on the upgrade, and a hub refuses
// one below its minimum with a 426 whose body says to update, rather than serving a wire it would
// misread. A supervisor that states none speaks 1. See ../engine-protocol/docs/version.md.

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import { WebSocket } from "ws";
import { TunnelAgent } from "./agent.ts";
import { TUNNEL_PATH, TUNNEL_TOKEN_HEADER } from "./frames.ts";
import {
  describeCore,
  type HubTarget,
  startHub,
  until as untilMs,
} from "./testCore.ts";

const until = (ok: () => boolean | Promise<boolean>) => untilMs(ok, 5000);

describeCore("the engine protocol on the tunnel", () => {
  let hub: HubTarget | undefined;
  const agents: TunnelAgent[] = [];

  const listen = async (minProtocol?: number): Promise<number> => {
    hub = await startHub({ minProtocol });
    return hub.port;
  };

  const dial = (port: number, id: string, protocol?: number): TunnelAgent => {
    const a = new TunnelAgent({
      registryUrl: `http://127.0.0.1:${port}`,
      id,
      token: "t",
      enginePort: 1,
      cacheDir: mkdtempSync(join(tmpdir(), "protocol-")),
      protocol,
      log: () => {},
    });
    agents.push(a);
    a.start();
    return a;
  };

  /** The upgrade's answer to a bare client sending `headers`: its status and its body. */
  const upgrade = (
    port: number,
    headers: Record<string, string>,
  ): Promise<{ status: number; body: string }> =>
    new Promise((resolve, reject) => {
      const ws = new WebSocket(`ws://127.0.0.1:${port}${TUNNEL_PATH}?id=bare`, {
        headers: { [TUNNEL_TOKEN_HEADER]: "t", ...headers },
      });
      ws.on("open", () => {
        ws.terminate();
        resolve({ status: 101, body: "" });
      });
      ws.on("unexpected-response", (req, res) => {
        let body = "";
        res.on("data", (c: Buffer) => {
          body += c.toString("utf8");
        });
        res.on("end", () => {
          req.destroy();
          resolve({ status: res.statusCode ?? 0, body });
        });
      });
      ws.on("error", reject);
    });

  afterEach(async () => {
    for (const a of agents.splice(0)) a.stop();
    await hub?.stop();
    hub = undefined;
  });

  it("serves an agent at the minimum", async () => {
    const port = await listen(2);
    dial(port, "current", 2);
    await until(async () => (await hub?.has("current")) ?? false);
  });

  it("refuses an agent below it, and says so once however often it redials", async () => {
    const port = await listen(2);
    dial(port, "old", 1);
    dial(port, "current", 2);
    await until(async () => (await hub?.has("current")) ?? false);
    await new Promise((r) => setTimeout(r, 300));
    expect(await hub?.has("old")).toBe(false);
    const said = (hub?.lines ?? []).filter((l) => l.startsWith("old refused"));
    expect(said).toHaveLength(1);
    expect(said[0]).toContain("update k7 on this machine");
  });

  it("answers 426 with the reason in the body", async () => {
    const port = await listen(3);
    const r = await upgrade(port, { "x-studio-engine-protocol": "2" });
    expect(r.status).toBe(426);
    expect(r.body).toContain("protocol 2");
    expect(r.body).toContain("needs 3");
  });

  it("reads a supervisor that states no protocol as protocol 1", async () => {
    const refused = await upgrade(await listen(2), {});
    expect(refused.status).toBe(426);
    expect(refused.body).toContain("protocol 1");
    await hub?.stop();
    expect((await upgrade(await listen(), {})).status).toBe(101);
  });
});
