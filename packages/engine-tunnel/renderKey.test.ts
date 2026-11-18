// renderKey.test.ts — the server's render machine: a tunnel that presents the deployment's render key
// is the one a render path reaches, a wrong key is refused outright, and a render path bound before the
// machine dialled in reaches it once it has. See docs/jobs.md.

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, expect, it } from "vitest";
import { WebSocket } from "ws";
import { requestJob } from "./jobsClient.ts";
import { type AgentHandle, agentKinds } from "./testAgents.ts";
import {
  describeCore,
  type HubTarget,
  startHub,
  until as untilMs,
} from "./testCore.ts";

const KEY = "the-deployment-key";

const until = (ok: () => boolean | Promise<boolean>) => untilMs(ok, 5000);

describeCore.each(agentKinds)(
  "the render machine — $name agent",
  ({ make }) => {
    let hub: HubTarget;
    let port: number;
    const agents: AgentHandle[] = [];

    beforeEach(async () => {
      hub = await startHub({ renderKey: KEY });
      port = hub.port;
    });

    afterEach(async () => {
      for (const a of agents.splice(0)) a.stop();
      await hub.stop();
    });

    const dial = (id: string, renderKey?: string): AgentHandle => {
      const a = make({
        registryUrl: hub.agentUrl,
        id,
        token: "t",
        renderKey,
        enginePort: 1,
        cacheDir: mkdtempSync(join(tmpdir(), "render-key-")),
        log: () => {},
        jobs: {
          where(req, emit) {
            emit({ type: "where", path: req.source });
          },
        },
      });
      agents.push(a);
      a.start();
      return a;
    };

    it("names the tunnel that presented the key, and no other", async () => {
      dial("laptop");
      dial("server", KEY);
      await until(
        async () => (await hub.has("laptop")) && (await hub.has("server")),
      );
      expect(await hub.renderer()).toBe("server");
    });

    it("refuses a tunnel whose key is wrong", async () => {
      dial("impostor", "not-the-key");
      dial("laptop");
      await until(() => hub.has("laptop"));
      // Given the impostor's redial time to have been refused at least once.
      await new Promise((r) => setTimeout(r, 300));
      expect(await hub.has("impostor")).toBe(false);
      expect(await hub.renderer()).toBeNull();
    });

    it("reaches the machine through a render path bound before it dialled in", async () => {
      const path = await hub.bindRenderer([]);
      dial("server", KEY);
      await until(async () => (await hub.renderer()) === "server");
      const ws = new WebSocket(`ws://127.0.0.1:${port}${path}`);
      try {
        const reply = await requestJob(
          ws,
          { type: "where", source: "here" },
          (r) => r.type === "where",
        );
        expect(reply.path).toBe("here");
      } finally {
        ws.close();
      }
    });
  },
);

describeCore("a hub with no render key", () => {
  it("has no render machine, and says so", async () => {
    const hub = await startHub();
    expect(await hub.rendersEnabled()).toBe(false);
    expect(await hub.renderer()).toBeNull();
    await hub.stop();
  });
});
