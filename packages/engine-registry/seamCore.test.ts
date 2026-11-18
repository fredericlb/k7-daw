// seamCore.test.ts — the internal seam's engine half held to docs/internalSeam.md against the Rust front that
// serves it, the testbed ENGINE_CORE_BIN names (testCore.ts). Its gate,
// the engine views, the hub's render machine and the loopback bindings; the local engine is not this half's.

import { request } from "node:http";
import { lanAddress, NO_LAN_PEER } from "@studio/engine-tunnel/test-core";
import { afterEach, beforeEach, expect, it } from "vitest";
import { WebSocket } from "ws";
import { INTERNAL_ENGINE_PATH, INTERNAL_SECRET_HEADER } from "./seam.ts";
import { EngineSeam } from "./seamClient.ts";
import {
  type CoreTarget,
  describeCore,
  RUST_CORE,
  startCore,
} from "./testCore.ts";

const keyOf = (path: string): string => path.split("/").pop() ?? "";

/** The status a loopback dial of bound key `key` gets: 403 unbound, 503 bound to an engine without a tunnel. */
const dialStatus = (base: string, key: string): Promise<number> =>
  new Promise((resolve, reject) => {
    const ws = new WebSocket(
      `${base.replace(/^http/, "ws")}/api/engine-tunnel/local/${key}`,
    );
    ws.on("unexpected-response", (_req, res) => {
      resolve(res.statusCode ?? 0);
      ws.terminate();
    });
    ws.on("open", () => {
      resolve(101);
      ws.close();
    });
    ws.on("error", (e) => reject(e));
  });

describeCore("the internal seam's engine half", () => {
  let core: CoreTarget;
  let seam: EngineSeam;

  beforeEach(async () => {
    core = await startCore({ seam: true });
    await core.reg.upsert({
      id: "e1",
      token: "t1",
      hostname: "rig",
      machineFingerprint: "f",
      addresses: { loopback: "127.0.0.1", lanIps: [], port: 8765 },
      performance: {
        score: 700,
        cores: 8,
        singleThread: 1,
        diskWriteMBps: null,
        fsyncPerSec: null,
        memoryGB: 8,
      },
    });
    await core.reg.owners.assign("e1", "ann@x");
    seam = new EngineSeam({ base: () => core.base, secret: core.seamSecret });
  });
  afterEach(() => core.stop());

  /** GET /hub from `host`, with `headers`: its status and its Cache-Control. */
  const hub = (
    headers: Record<string, string>,
    host = "127.0.0.1",
    localAddress?: string,
  ) =>
    new Promise<{ status: number; cache?: string }>((resolve, reject) => {
      const r = request(
        {
          host,
          port: core.port,
          path: `${INTERNAL_ENGINE_PATH}/hub`,
          headers,
          localAddress,
        },
        (res) => {
          res.resume();
          resolve({
            status: res.statusCode ?? 0,
            cache: res.headers["cache-control"],
          });
        },
      );
      r.on("error", reject);
      r.end();
    });

  it("refuses a request without the secret, or with another, by 401", async () => {
    expect((await hub({})).status).toBe(401);
    expect((await hub({ [INTERNAL_SECRET_HEADER]: "nope" })).status).toBe(401);
    expect(
      (await hub({ [INTERNAL_SECRET_HEADER]: `${core.seamSecret}x` })).status,
    ).toBe(401);
    const ok = await hub({ [INTERNAL_SECRET_HEADER]: core.seamSecret });
    expect(ok.status).toBe(200);
    expect(ok.cache).toBe("no-store");
  });

  it.skipIf(RUST_CORE && NO_LAN_PEER)(
    "refuses a peer that is not this machine by 403, even with the secret and whatever it says of itself",
    async () => {
      const secret = { [INTERNAL_SECRET_HEADER]: core.seamSecret };
      if (RUST_CORE) {
        const lan = lanAddress();
        if (!lan) throw new Error("no LAN address to dial from");
        expect(
          (
            await hub(
              { ...secret, "x-k7-peer": "127.0.0.1", "x-k7-front": "guess" },
              lan,
              lan,
            )
          ).status,
        ).toBe(403);
      } else {
        expect(
          (await hub({ ...secret, "x-test-peer": "203.0.113.7" })).status,
        ).toBe(403);
        // A request that named the front without proving it fails closed too.
        expect(
          (await hub({ ...secret, "x-test-peer": "front-unverified" })).status,
        ).toBe(403);
      }
      expect((await hub(secret)).status).toBe(200);
    },
  );

  it("reads an engine as one account sees it, and none for an unknown id", async () => {
    expect(await seam.engine("e1", "ann@x")).toEqual({
      id: "e1",
      hostname: "rig",
      role: "owner",
      holds: false,
      tunnel: false,
      renderer: false,
      score: 700,
    });
    expect((await seam.engine("e1", "bob@x"))?.role).toBe("none");
    expect((await seam.engine("e1", ""))?.role).toBe("none");
    expect(await seam.engine("zz", "ann@x")).toBeNull();
    const claimed = await core.reg.claim("e1", {
      project: "r/p",
      holder: "bob@x",
    });
    expect(claimed.ok).toBe(true);
    expect((await seam.engine("e1", "bob@x"))?.holds).toBe(true);
    expect((await seam.engines("bob@x")).map((e) => [e.id, e.holds])).toEqual([
      ["e1", true],
    ]);
  });

  it("says whether the server renders, and which machine does", async () => {
    expect(await seam.hub()).toEqual({
      rendersEnabled: false,
      renderer: null,
      epoch: expect.stringMatching(/^[A-Za-z0-9_-]+$/),
    });
  });

  it("binds a loopback path, refuses a malformed binding, and drops a path twice without error", async () => {
    const path = await seam.bind("e1", [], "jobs", { synced: false });
    expect(path).toMatch(/^\/api\/engine-tunnel\/local\/[A-Za-z0-9_-]+$/);
    expect(await seam.bind(null, [], "engine")).toMatch(/\/local\//);
    const post = (body: unknown) =>
      fetch(`${core.base}${INTERNAL_ENGINE_PATH}/bindings`, {
        method: "POST",
        headers: {
          [INTERNAL_SECRET_HEADER]: core.seamSecret,
          "content-type": "application/json",
        },
        body: JSON.stringify(body),
      });
    expect(
      (await post({ engineId: "e1", roots: [], kind: "files" })).status,
    ).toBe(400);
    expect((await post({ roots: [], kind: "jobs" })).status).toBe(400);
    expect(
      (
        await post({
          engineId: "e1",
          roots: [{ remote: "", name: "a", writable: true }],
          kind: "jobs",
        })
      ).status,
    ).toBe(400);
    const bad = await fetch(
      `${core.base}${INTERNAL_ENGINE_PATH}/bindings/not!a!key`,
      {
        method: "DELETE",
        headers: { [INTERNAL_SECRET_HEADER]: core.seamSecret },
      },
    );
    expect(bad.status).toBe(400);
    await seam.unbind(path);
    await seam.unbind(path);
  });

  it("binds under a key its caller held, and refuses a key shorter than a drawn one", async () => {
    const post = (body: unknown) =>
      fetch(`${core.base}${INTERNAL_ENGINE_PATH}/bindings`, {
        method: "POST",
        headers: {
          [INTERNAL_SECRET_HEADER]: core.seamSecret,
          "content-type": "application/json",
        },
        body: JSON.stringify(body),
      });
    const key = "k".repeat(32);
    const r = await post({ engineId: "e1", roots: [], kind: "engine", key });
    expect(r.status).toBe(201);
    const { epoch } = await seam.hub();
    expect(await r.json()).toEqual({
      path: `/api/engine-tunnel/local/${key}`,
      epoch,
    });
    // Bound, so the engine's missing tunnel answers, not the key.
    expect(await dialStatus(core.base, key)).toBe(503);
    expect(await dialStatus(core.base, "u".repeat(32))).toBe(403);
    for (const bad of ["short", "k".repeat(129), `${"k".repeat(32)}!`])
      expect(
        (await post({ engineId: null, roots: [], kind: "jobs", key: bad }))
          .status,
      ).toBe(400);
  });

  it("re-establishes its kept bindings under their keys once another hub answers", async () => {
    const log: string[] = [];
    let base = core.base;
    const kept = new EngineSeam({
      base: () => base,
      secret: core.seamSecret,
      watchMs: 60_000,
      log: (m) => log.push(m),
    });
    const path = await kept.bind("e1", [], "engine", { synced: true });
    const render = await kept.bind(null, [], "jobs");
    const once = await kept.bind("e1", [], "jobs");
    await kept.unbind(once);
    expect(await kept.rebind()).toBe(0);
    // Another process holding the hub: none of the keys is bound there.
    const next = await startCore({ seam: true });
    try {
      await next.reg.upsert({
        id: "e1",
        token: "t1",
        hostname: "rig",
        machineFingerprint: "f",
        addresses: { loopback: "127.0.0.1", lanIps: [], port: 8765 },
      });
      base = next.base;
      expect(await dialStatus(next.base, keyOf(path))).toBe(403);
      expect(await kept.rebind()).toBe(2);
      expect(await dialStatus(next.base, keyOf(path))).toBe(503);
      expect(await dialStatus(next.base, keyOf(render))).toBe(503);
      expect(await dialStatus(next.base, keyOf(once))).toBe(403);
      expect(log).toEqual([
        "the engine hub restarted: 2 loopback binding(s) re-established",
      ]);
      expect(await kept.rebind()).toBe(0);
      await kept.unbind(path);
      await kept.unbind(render);
      expect(await dialStatus(next.base, keyOf(path))).toBe(403);
    } finally {
      await next.stop();
    }
  });

  it("answers no channel for an engine without a tunnel", async () => {
    expect(await seam.open("e1", [], "jobs", { timeoutMs: 2000 })).toBeNull();
    expect(await seam.open(null, [], "engine", { timeoutMs: 2000 })).toBeNull();
  });
});
