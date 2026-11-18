// heartbeatGate.test.ts — who may LIST an engine: the server's render machine (the render key) alone
// lists one; with the gate off, anyone else, and an unproven one is said. See docs/heartbeat.md.

import { afterEach, expect, it } from "vitest";
import { type AsyncRegistry, describeCore, startCore } from "./testCore.ts";

const beat = (id: string) => ({
  id,
  token: `tok-${id}`,
  hostname: "studio",
  machineFingerprint: "fp",
  addresses: { loopback: "127.0.0.1", lanIps: [], port: 8765 },
});

const closers: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const c of closers.splice(0)) await c();
});

async function registryWith(
  requireDevice: boolean,
): Promise<{ reg: AsyncRegistry; logged: string[]; base: string }> {
  const core = await startCore({
    heartbeat: {
      requireDevice,
      renderKey: "the-render-key",
    },
  });
  closers.push(() => core.stop());
  return { reg: core.reg, logged: core.lines, base: core.base };
}

const send = (base: string, id: string, headers: Record<string, string> = {}) =>
  fetch(`${base}/api/registry/heartbeat`, {
    method: "POST",
    headers: { "content-type": "application/json", ...headers },
    body: JSON.stringify(beat(id)),
  });

describeCore("the heartbeat gate", () => {
  it("lists only the render machine when the studio asks for it", async () => {
    const { reg, base } = await registryWith(true);

    const stranger = await send(base, "rogue");
    expect(stranger.status).toBe(401);
    expect(String((await stranger.json()).error)).toMatch(/render key/);
    expect(await reg.get("rogue")).toBeUndefined();

    const forged = await send(base, "forged", {
      authorization: "Bearer not-a-token",
    });
    expect(forged.status).toBe(401);

    const render = await send(base, "server", {
      "x-studio-render-key": "the-render-key",
    });
    expect(render.status).toBe(200);
    const wrongKey = await send(base, "server2", {
      "x-studio-render-key": "nope",
    });
    expect(wrongKey.status).toBe(401);
  });

  it("still lists everyone with the gate off, and says which machines are unpaired", async () => {
    const { reg, logged, base } = await registryWith(false);
    expect((await send(base, "old-supervisor")).status).toBe(200);
    expect(await reg.get("old-supervisor")).toBeDefined();
    expect(logged.join("\n")).toMatch(
      /took a heartbeat from engine old-supervisor.*no render key/,
    );
  });
});
