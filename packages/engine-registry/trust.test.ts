// trust.test.ts — who may list and tunnel under an engine id: the first tunnel secret binds it,
// on disk, and a token alone — which every relay user can read — never takes a bound id back.
// docs/token.md is the prose; this is the contract. Runs against either core (testCore.ts).

import {
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, expect, it } from "vitest";
import { WebSocket } from "ws";
import {
  BINDINGS_FILE,
  type CoreTarget,
  describeCore,
  type HeartbeatInput,
  REGISTRY_TTL_MS,
  startCore,
} from "./testCore.ts";

const SECRET = "s".repeat(64);
const OTHER = "o".repeat(64);

const beat = (over: Partial<HeartbeatInput> = {}): HeartbeatInput => ({
  id: "e1",
  token: "tok",
  hostname: "studio",
  machineFingerprint: "fp",
  addresses: { loopback: "127.0.0.1", lanIps: [], port: 8765 },
  publicIpHint: "203.0.113.5",
  ...over,
});

describeCore("the secret bindings", () => {
  let dir: string;
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "bindings-"));
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it("binds an id to its first secret, on disk, readable by its owner alone", async () => {
    const file = join(dir, BINDINGS_FILE);
    const a = await startCore({ stateDir: dir });
    expect(await a.reg.binding("e1", SECRET)).toBe("fresh");
    await a.reg.upsert(beat({ tunnelSecret: SECRET }));
    await a.stop();
    // A restart reads it back: the binding is not the registry's 15 s memory.
    const b = await startCore({ stateDir: dir });
    expect(await b.reg.binding("e1", SECRET)).toBe("bound");
    expect(await b.reg.binding("e1", OTHER)).toBe("refused");
    expect(await b.reg.binding("e1", undefined)).toBe("refused");
    expect(await b.reg.binding("e2", undefined)).toBe("legacy");
    await b.stop();
    // Only a digest is kept.
    expect(readFileSync(file, "utf8")).not.toContain(SECRET);
    if (process.platform !== "win32")
      expect(statSync(file).mode & 0o077).toBe(0);
  });

  it("sets an unreadable file aside rather than trusting or overwriting it", async () => {
    const file = join(dir, BINDINGS_FILE);
    const a = await startCore({ stateDir: dir });
    await a.reg.upsert(beat({ tunnelSecret: SECRET }));
    await a.stop();
    rmSync(file);
    writeFileSync(file, "{not json");
    const b = await startCore({ stateDir: dir });
    expect(await b.reg.binding("e1", SECRET)).toBe("fresh");
    expect(b.lines.join("\n")).toMatch(/unreadable/);
    await b.stop();
  });
});

describeCore("the registry under the bindings", () => {
  let core: CoreTarget;
  beforeEach(async () => {
    core = await startCore();
    core.clock.fake();
  });
  afterEach(async () => {
    core.clock.real();
    await core.stop();
  });

  it("refuses a different secret for a bound id, even after the entry lapsed", async () => {
    const reg = core.reg;
    expect(await reg.upsert(beat({ tunnelSecret: SECRET }))).toEqual({
      ok: true,
    });
    await core.clock.advance(REGISTRY_TTL_MS + 1);
    expect(await reg.get("e1")).toBeUndefined();
    expect(await reg.upsert(beat({ tunnelSecret: OTHER }))).toEqual({
      ok: false,
      why: "secret",
    });
    expect(await reg.get("e1")).toBeUndefined();
  });

  it("never lets a legacy heartbeat (no secret) take a bound id, with the right token or not", async () => {
    const reg = core.reg;
    await reg.upsert(beat({ tunnelSecret: SECRET }));
    expect(await reg.upsert(beat())).toEqual({ ok: false, why: "secret" });
    await core.clock.advance(REGISTRY_TTL_MS + 1);
    expect(await reg.upsert(beat({ token: "fresh" }))).toEqual({
      ok: false,
      why: "secret",
    });
  });

  it("lets the bound machine change its token, the secret being the proof", async () => {
    const reg = core.reg;
    await reg.upsert(beat({ tunnelSecret: SECRET }));
    expect(
      await reg.upsert(beat({ token: "new", tunnelSecret: SECRET })),
    ).toEqual({
      ok: true,
    });
    expect(await reg.verifyToken("e1", "new")).toBe(true);
  });

  it("lists a legacy engine as such, and still pins its first token", async () => {
    const reg = core.reg;
    expect(await reg.upsert(beat())).toEqual({ ok: true });
    expect((await reg.list())[0]?.legacy).toBe(true);
    expect(await reg.upsert(beat({ token: "other" }))).toEqual({
      ok: false,
      why: "token",
    });
    // A legacy engine that updates gains a binding with the token it already had.
    expect(await reg.upsert(beat({ tunnelSecret: SECRET }))).toEqual({
      ok: true,
    });
    expect((await reg.list())[0]?.legacy).toBeUndefined();
  });

  it("opens a tunnel only with the bound secret; a legacy id on its token", async () => {
    const reg = core.reg;
    await reg.upsert(beat({ tunnelSecret: SECRET }));
    expect(await reg.verifyTunnel("e1", "tok", SECRET)).toBe(true);
    expect(await reg.verifyTunnel("e1", "tok", OTHER)).toBe(false);
    expect(await reg.verifyTunnel("e1", "tok", undefined)).toBe(false);
    expect(await reg.verifyTunnel("e1", "nope", SECRET)).toBe(false);
    await reg.upsert(beat({ id: "old" }));
    expect(await reg.verifyTunnel("old", "tok", undefined)).toBe(true);
  });

  it("keeps a bound id listed against a deregister carrying the token alone", async () => {
    const reg = core.reg;
    await reg.upsert(beat({ tunnelSecret: SECRET }));
    expect(await reg.remove("e1", "tok")).toBe(false);
    expect(await reg.remove("e1", "tok", SECRET)).toBe(true);
  });

  it("caps the list, and the new ids one address may add in a minute", async () => {
    const small = await startCore({ maxEngines: 2, newIdsPerIp: 1 });
    try {
      const reg = small.reg;
      expect(await reg.upsert(beat({ id: "a" }))).toEqual({ ok: true });
      expect(await reg.upsert(beat({ id: "b" }))).toEqual({
        ok: false,
        why: "rate",
      });
      // Renewing an id already listed is not a new one.
      expect(await reg.upsert(beat({ id: "a" }))).toEqual({ ok: true });
      expect(
        await reg.upsert(beat({ id: "b", publicIpHint: "198.51.100.1" })),
      ).toEqual({ ok: true });
      expect(
        await reg.upsert(beat({ id: "c", publicIpHint: "198.51.100.2" })),
      ).toEqual({ ok: false, why: "full" });
    } finally {
      await small.stop();
    }
  });
});

describeCore("the heartbeat and tunnel routes", () => {
  let core: CoreTarget;
  let logged: string[];

  const post = async (path: string, body: unknown) => {
    const r = await core.post(path, body);
    return { status: r.status, body: r.body as { error?: string } };
  };

  /** Open a tunnel as a supervisor would, resolving the socket or the refusal's status. */
  const tunnel = (secret?: string): Promise<WebSocket | number> =>
    new Promise((resolve) => {
      const ws = new WebSocket(
        `${core.base.replace("http", "ws")}/api/engine-tunnel?id=e1`,
        {
          headers: {
            "x-studio-engine-token": "tok",
            ...(secret ? { "x-studio-tunnel-secret": secret } : {}),
          },
        },
      );
      ws.once("open", () => resolve(ws));
      ws.once("unexpected-response", (_req, res) =>
        resolve(res.statusCode ?? 0),
      );
      ws.once("error", () => resolve(0));
    });

  beforeEach(async () => {
    core = await startCore({ hub: true });
    logged = core.lines;
  });
  afterEach(async () => {
    await core.stop();
  });

  const body = (over: Record<string, unknown> = {}) => ({
    id: "e1",
    token: "tok",
    addresses: { loopback: "127.0.0.1", lanIps: [], port: 8765 },
    ...over,
  });

  it("answers 403 to a heartbeat with another secret for a bound id, and says so in the log", async () => {
    expect(
      (await post("/registry/heartbeat", body({ tunnelSecret: SECRET })))
        .status,
    ).toBe(200);
    const r = await post("/registry/heartbeat", body({ tunnelSecret: OTHER }));
    expect(r.status).toBe(403);
    expect(r.body.error).toMatch(/bound to another machine/);
    expect(logged.join("\n")).toMatch(/refused a heartbeat from engine e1/);
  });

  it("refuses an id or a token past its bounds", async () => {
    expect(
      (await post("/registry/heartbeat", body({ id: "a/b" }))).status,
    ).toBe(400);
    expect(
      (await post("/registry/heartbeat", body({ id: "x".repeat(129) }))).status,
    ).toBe(400);
    expect(
      (await post("/registry/heartbeat", body({ token: "t".repeat(257) })))
        .status,
    ).toBe(400);
    expect(
      (await post("/registry/heartbeat", body({ tunnelSecret: "short" })))
        .status,
    ).toBe(400);
  });

  it("refuses a tunnel with the wrong secret and keeps the incumbent; the right one replaces it", async () => {
    await post("/registry/heartbeat", body({ tunnelSecret: SECRET }));
    const first = await tunnel(SECRET);
    expect(first).toBeInstanceOf(WebSocket);
    expect(await core.hubHas("e1")).toBe(true);

    expect(await tunnel(OTHER)).toBe(403);
    expect(await tunnel()).toBe(403);
    expect((first as WebSocket).readyState).toBe(WebSocket.OPEN);

    const closed = new Promise((r) => (first as WebSocket).once("close", r));
    const second = await tunnel(SECRET);
    expect(second).toBeInstanceOf(WebSocket);
    await closed;
    expect(await core.hubHas("e1")).toBe(true);
    (second as WebSocket).close();
  });
});
