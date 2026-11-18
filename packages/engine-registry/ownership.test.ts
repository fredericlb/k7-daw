// ownership.test.ts — whose an engine is: the store outlives a restart, no heartbeat names an owner,
// an open enrols nobody, `select` refuses everyone the
// owner did not share it with (or with everyone), an engine on the backend's own machine is every
// account's, the relay never hands the token to an account that may not use it.

import { mkdtempSync, readFileSync } from "node:fs";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, expect, it } from "vitest";
import { WebSocket, WebSocketServer } from "ws";
import {
  type AsyncRegistry,
  type CoreTarget,
  describeCore,
  OWNERS_FILE,
  startCore,
} from "./testCore.ts";

describeCore("the owners store", () => {
  it("persists an owner, its shares and its everyone flag across a restart, and forgets a released one", async () => {
    const dir = mkdtempSync(join(tmpdir(), "owners-"));
    const file = join(dir, OWNERS_FILE);
    const ca = await startCore({ stateDir: dir });
    const a = ca.reg.owners;
    expect(await a.assign("e1", "ann@x")).toBe("new");
    expect(await a.assign("e1", "ann@x")).toBe("same");
    await a.share("e1", ["bob@x"], true);
    await a.assign("e2", "ann@x");
    await a.release("e2");
    await ca.stop();
    const cb = await startCore({ stateDir: dir });
    const b = cb.reg.owners;
    expect(await b.get("e1")).toMatchObject({
      owner: "ann@x",
      shared: ["bob@x"],
      everyone: true,
    });
    expect(await b.get("e2")).toBeUndefined();
    await cb.stop();
    expect(JSON.parse(readFileSync(file, "utf8")).version).toBe(1);
  });

  it("moves an engine to another account, which does not inherit the old shares", async () => {
    const c = await startCore();
    const o = c.reg.owners;
    await o.assign("e1", "ann@x");
    await o.share("e1", ["bob@x"], true);
    expect(await o.assign("e1", "cat@x")).toBe("moved");
    expect(await o.get("e1")).toMatchObject({
      owner: "cat@x",
      shared: [],
      everyone: false,
    });
    await c.stop();
  });

  it("leaves the everyone flag as it was when a share edit does not name it", async () => {
    const c = await startCore();
    const o = c.reg.owners;
    await o.assign("e1", "ann@x");
    await o.share("e1", [], true);
    await o.share("e1", ["bob@x"]);
    expect((await o.get("e1"))?.everyone).toBe(true);
    await c.stop();
  });
});

/** A registry behind its routes, as k7 mounts it: proxies believed, `root@x` an admin, a heartbeat gate. */
async function routes(server: { renderer?: string } = {}) {
  const core = await startCore({
    // So a test heartbeat can come from the LAN rather than from this loopback.
    trustProxy: true,
    admins: ["root@x"],
    heartbeat: {
      requireDevice: false,
      renderKey: "the-render-key",
    },
    renderer: server.renderer,
  });
  const call = (user: string, method: string, path: string, body?: unknown) =>
    core.call(user, method, path, body);
  /** A heartbeat over the route, from the LAN unless told otherwise, with these credentials. */
  const heartbeat = (id: string, headers: Record<string, string> = {}) =>
    core.post(
      "/registry/heartbeat",
      {
        id,
        token: `tok-${id}`,
        hostname: `host-${id}`,
        machineFingerprint: "fp",
        addresses: { loopback: "127.0.0.1", lanIps: [], port: 8765 },
      },
      { "x-forwarded-for": "10.0.0.9", ...headers },
    );
  return { reg: core.reg, call, heartbeat, close: () => core.stop() };
}

const beat = (reg: AsyncRegistry, id: string, ip = "10.0.0.9") =>
  reg.upsert({
    id,
    token: `tok-${id}`,
    hostname: `host-${id}`,
    machineFingerprint: "",
    addresses: { loopback: "127.0.0.1", lanIps: ["192.168.1.9"], port: 8765 },
    publicIpHint: ip,
  });

describeCore("the heartbeat names no owner", () => {
  let r: Awaited<ReturnType<typeof routes>>;
  beforeEach(async () => {
    r = await routes();
  });
  afterEach(() => r.close());

  it("owns nothing on a heartbeat that proves no account, the render machine's included", async () => {
    await r.heartbeat("e1");
    await r.heartbeat("e2", { authorization: "Bearer forged" });
    await r.heartbeat("srv", { "x-studio-render-key": "the-render-key" });
    expect(await r.reg.owners.all()).toEqual([]);
  });
});

describeCore("select, listing and the share routes", () => {
  let r: Awaited<ReturnType<typeof routes>>;
  beforeEach(async () => {
    r = await routes({ renderer: "srv" });
    await beat(r.reg, "e1");
    await beat(r.reg, "srv");
  });
  afterEach(() => r.close());

  const select = (user: string, id = "e1", secret?: string) =>
    r.call(user, "POST", "/engine/select", {
      id,
      token: `tok-${id}`,
      project: null,
      secret,
    });

  it("refuses to open an unowned engine, and an open enrols nobody", async () => {
    const ann = await select("ann@x");
    expect(ann.status).toBe(403);
    expect(ann.body).toMatchObject({ error: "unowned" });
    expect(await r.reg.owners.get("e1")).toBeUndefined();
  });

  it("refuses an anonymous session, which can own nothing", async () => {
    await r.reg.owners.assign("e1", "ann@x");
    expect((await select("")).status).toBe(403);
  });

  it("lets the owner and a shared account claim, and every account the server's engine", async () => {
    await r.reg.owners.assign("e1", "ann@x");
    await r.reg.owners.share("e1", ["bob@x"]);
    expect((await select("ann@x")).status).toBe(200);
    const bob = await select("bob@x");
    expect(bob.status).toBe(200);
    // Between allowed accounts the claim rules still hold: a claim with no project displaces nobody.
    expect((bob.body as { tookOver?: unknown }).tookOver).toBeUndefined();
    const eve = await select("eve@x");
    expect(eve.status).toBe(403);
    expect(eve.body).toMatchObject({ error: "not-yours" });
    expect((await select("eve@x", "srv")).status).toBe(200);
    expect(await r.reg.owners.get("srv")).toBeUndefined();
  });

  it("lists owned engines to their accounts alone, the server's to all, an unowned one without its LAN or a ticket", async () => {
    await r.reg.owners.assign("e1", "ann@x");
    const ann = (await r.call("ann@x", "GET", "/engines")).body as {
      id: string;
      access: string;
      proxyTicket?: string;
    }[];
    expect(ann.map((e) => [e.id, e.access])).toEqual([
      ["e1", "owner"],
      ["srv", "server"],
    ]);
    expect(ann[0]?.proxyTicket).toBeTruthy();
    const eve = (await r.call("eve@x", "GET", "/engines")).body as {
      id: string;
    }[];
    expect(eve.map((e) => e.id)).toEqual(["srv"]);

    await beat(r.reg, "e3");
    const fresh = (
      (await r.call("eve@x", "GET", "/engines")).body as {
        id: string;
        access: string;
        proxyTicket?: string;
        addresses: { lanIps: string[] };
      }[]
    ).find((e) => e.id === "e3");
    expect(fresh?.access).toBe("unowned");
    expect(fresh?.proxyTicket).toBeUndefined();
    expect(fresh?.addresses.lanIps).toEqual([]);
  });

  it("lists the server's engine as its admins' own, still taken unasked, and everyone else's to use", async () => {
    const list = async (user: string) =>
      (
        (await r.call(user, "GET", "/engines")).body as {
          id: string;
          access: string;
          server?: boolean;
        }[]
      ).find((e) => e.id === "srv");
    expect(await list("root@x")).toMatchObject({
      access: "owner",
      server: true,
    });
    expect(await list("eve@x")).toMatchObject({
      access: "server",
      server: true,
    });
  });

  it("names the owner to a shared account", async () => {
    await r.reg.owners.assign("e1", "ann@x");
    await r.reg.owners.share("e1", ["bob@x"]);
    const bob = (await r.call("bob@x", "GET", "/engines")).body as {
      id: string;
      access: string;
      ownedBy?: string;
    }[];
    expect(bob[0]).toMatchObject({
      id: "e1",
      access: "shared",
      ownedBy: "ann@x",
    });
  });

  it("lets the owner alone read and replace the shares, and a withdrawn share ends that claim", async () => {
    await r.reg.owners.assign("e1", "ann@x");
    expect(
      (
        await r.call("bob@x", "PUT", "/engines/e1/shares", {
          emails: ["bob@x"],
        })
      ).status,
    ).toBe(404);
    const put = await r.call("ann@x", "PUT", "/engines/e1/shares", {
      emails: ["Bob@x", "bob@x"],
    });
    expect(put.body).toMatchObject({
      ok: true,
      shared: ["bob@x"],
      everyone: false,
    });
    expect((await r.call("bob@x", "GET", "/engines/e1/shares")).status).toBe(
      404,
    );
    expect(
      (await r.call("ann@x", "GET", "/engines/e1/shares")).body,
    ).toMatchObject({ owner: "ann@x", shared: ["bob@x"], everyone: false });
    expect(
      (await r.call("ann@x", "PUT", "/engines/e1/shares", { emails: ["nope"] }))
        .status,
    ).toBe(400);

    expect((await select("bob@x")).status).toBe(200);
    await r.call("ann@x", "PUT", "/engines/e1/shares", { emails: [] });
    expect(await r.reg.holds("e1", "bob@x")).toBe(false);
  });

  it("shares with everyone on the server, and withdrawing it ends their claims", async () => {
    await r.reg.owners.assign("e1", "ann@x");
    expect((await select("eve@x")).status).toBe(403);
    const put = await r.call("ann@x", "PUT", "/engines/e1/shares", {
      emails: [],
      everyone: true,
    });
    expect(put.body).toMatchObject({ ok: true, everyone: true });
    const eve = (await r.call("eve@x", "GET", "/engines")).body as {
      id: string;
      access: string;
    }[];
    expect(eve.find((e) => e.id === "e1")?.access).toBe("shared");
    expect((await select("eve@x")).status).toBe(200);
    await r.call("ann@x", "PUT", "/engines/e1/shares", {
      emails: [],
      everyone: false,
    });
    expect(await r.reg.holds("e1", "eve@x")).toBe(false);
  });

  it("lets the owner give it up and an admin reset it, nobody else", async () => {
    await r.reg.owners.assign("e1", "ann@x");
    expect((await r.call("eve@x", "DELETE", "/engines/e1/owner")).status).toBe(
      404,
    );
    expect((await r.call("eve@x", "GET", "/engines/owners")).status).toBe(403);
    const all = await r.call("root@x", "GET", "/engines/owners");
    expect(all.body).toEqual([
      expect.objectContaining({
        id: "e1",
        owner: "ann@x",
        hostname: "host-e1",
        online: true,
      }),
    ]);
    expect((await r.call("root@x", "DELETE", "/engines/e1/owner")).status).toBe(
      200,
    );
    expect(await r.reg.owners.get("e1")).toBeUndefined();
    await r.reg.owners.assign("e1", "ann@x");
    expect((await r.call("ann@x", "DELETE", "/engines/e1/owner")).status).toBe(
      200,
    );
    expect(await r.reg.owners.get("e1")).toBeUndefined();
  });
});

describeCore("the backend's own machine", () => {
  let r: Awaited<ReturnType<typeof routes>>;
  beforeEach(async () => {
    r = await routes();
    // The backend machine's engine beats from loopback; a LAN one from another machine.
    await beat(r.reg, "own", "::ffff:127.0.0.1");
    await beat(r.reg, "lan");
    await r.reg.owners.assign("own", "local@x");
    await r.reg.owners.assign("lan", "local@x");
  });
  afterEach(() => r.close());

  it("lists its engine to every signed-in account, and nobody else's", async () => {
    const fred = (await r.call("fred@x", "GET", "/engines")).body as {
      id: string;
      access: string;
    }[];
    expect(fred.map((e) => [e.id, e.access])).toEqual([["own", "owner"]]);
    expect((await r.call("", "GET", "/engines")).body).toEqual([]);
  });

  it("lets another account select it, without changing whose it is", async () => {
    const res = await r.call("fred@x", "POST", "/engine/select", {
      id: "own",
      token: "tok-own",
      project: null,
    });
    expect(res.status).toBe(200);
    expect((await r.reg.owners.get("own"))?.owner).toBe("local@x");
  });

  it("stays this machine's when it also beats through a proxy, which hides that it is local", async () => {
    await beat(r.reg, "own", "192.168.87.10");
    const fred = (await r.call("fred@x", "GET", "/engines")).body as {
      id: string;
      access: string;
    }[];
    expect(fred.map((e) => [e.id, e.access])).toEqual([["own", "owner"]]);
  });

  it("needs no owner at all", async () => {
    await beat(r.reg, "bare", "127.0.0.1");
    const res = await r.call("fred@x", "POST", "/engine/select", {
      id: "bare",
      token: "tok-bare",
      project: null,
    });
    expect(res.status).toBe(200);
  });
});

describeCore("the relay and the token", () => {
  let core: CoreTarget;
  let engine: WebSocketServer;

  beforeEach(async () => {
    engine = new WebSocketServer({ port: 0, host: "127.0.0.1" });
    engine.on("connection", (ws) => {
      let first = true;
      ws.on("message", (d) => {
        const m = JSON.parse(String(d));
        if (first) {
          first = false;
          ws.send(JSON.stringify({ type: "paired" }));
          return;
        }
        if (m.type === "whoami")
          ws.send(JSON.stringify({ type: "whoami", id: "e1", token: "tok" }));
      });
    });
    await new Promise((r) => engine.once("listening", r));
    core = await startCore();
    // The relay dials where the heartbeat came from, so the engine is the backend machine's own here.
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

  /** The whoami a browser of account `holder` reads through the relay. */
  const whoami = async (holder: string): Promise<Record<string, unknown>> => {
    const ticket = encodeURIComponent(await core.ticket("e1", holder));
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(
        `ws://127.0.0.1:${core.port}/api/engine/e1/ws?ticket=${ticket}`,
      );
      ws.once("open", () => ws.send(JSON.stringify({ type: "whoami" })));
      ws.on("message", (d) => {
        ws.close();
        resolve(JSON.parse(String(d)));
      });
      ws.once("error", reject);
    });
  };

  // The owner, shared and everyone cases go through the same `engineRole` the routes above test.
  it("hands the token of an engine on this machine to every signed-in account, owned or not", async () => {
    expect((await whoami("eve@x")).token).toBe("tok");
    await core.reg.owners.assign("e1", "ann@x");
    expect((await whoami("eve@x")).token).toBe("tok");
  });

  it("never to an anonymous ticket", async () => {
    expect(await whoami("")).toEqual({ type: "whoami", id: "e1" });
  });
});
