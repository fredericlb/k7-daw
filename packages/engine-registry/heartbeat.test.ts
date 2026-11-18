// heartbeat.test.ts — a heartbeat is refused only for what the registry cannot do without, and
// every other odd field is cleaned and SAID. What this pins is the Windows machine that was listed
// once and then refused every heartbeat for one out-of-bounds number: listed, lapsed fifteen
// seconds later, and a play answered "unknown engine or bad token". docs/heartbeat.md.

import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { type CoreTarget, describeCore, startCore } from "./testCore.ts";

/** What a supervisor sends, well-formed. */
const good = () => ({
  id: "e1",
  token: "tok-e1",
  hostname: "studio",
  machineFingerprint: "fp",
  port: 8765,
  addresses: { loopback: "127.0.0.1", lanIps: ["192.168.1.20"], port: 8765 },
  performance: {
    score: 1200,
    cores: 8,
    singleThread: 70,
    diskWriteMBps: 900,
    fsyncPerSec: 400,
    memoryGB: 32,
  },
});

describeCore("the routes", () => {
  let core: CoreTarget;
  let logged: string[];

  const post = async (path: string, body: unknown) => {
    const r = await core.post(path, body);
    return { status: r.status, body: r.body as Record<string, unknown> };
  };

  beforeEach(async () => {
    // A signed-in account: an anonymous session may claim nothing — docs/ownership.md.
    core = await startCore({ user: "ann@x" });
    // The core logs on its own stdout, read after its answer may have arrived: a log line is waited for.
    logged = core.lines;
  });
  afterEach(() => core.stop());

  it("LISTS an engine whose heartbeat carried an out-of-bounds rating, and logs what it cleaned", async () => {
    const b = good();
    b.performance.cores = 0;
    const r = await post("/registry/heartbeat", b);
    expect(r.status).toBe(200);
    expect(r.body.cleaned).toEqual([expect.stringMatching(/cores/)]);
    expect((await core.reg.get("e1"))?.performance).toBeUndefined();
    await vi.waitFor(() =>
      expect(logged.join("\n")).toMatch(/engine e1 \(studio\).*cleaned/),
    );
  });

  it("refuses a heartbeat it cannot use with the REASON in the body, for the supervisor's log", async () => {
    const r = await post("/registry/heartbeat", { ...good(), token: "" });
    expect(r.status).toBe(400);
    expect(String(r.body.error)).toMatch(/heartbeat refused — token:/);
    await vi.waitFor(() =>
      expect(logged.join("\n")).toMatch(/refused a heartbeat from engine e1/),
    );
  });

  it("logs a token mismatch and names the way out", async () => {
    await post("/registry/heartbeat", good());
    const r = await post("/registry/heartbeat", { ...good(), token: "other" });
    expect(r.status).toBe(409);
    expect(String(r.body.error)).toMatch(/freed 15 s after/);
    await vi.waitFor(() =>
      expect(logged.join("\n")).toMatch(/registered with another token/),
    );
  });

  it("tells a LAPSED engine from a wrong token on select", async () => {
    const gone = await post("/engine/select", { id: "nobody", token: "t" });
    expect(gone.status).toBe(404);
    expect(String(gone.body.error)).toMatch(/engine gone.*heartbeat/);

    await post("/registry/heartbeat", good());
    const wrong = await post("/engine/select", { id: "e1", token: "nope" });
    expect(wrong.status).toBe(403);
    expect(String(wrong.body.error)).toMatch(/bad token/);

    const right = await post("/engine/select", { id: "e1", token: "tok-e1" });
    expect(right.status).toBe(200);
    await vi.waitFor(() =>
      expect(logged.join("\n")).toMatch(/select of engine nobody refused/),
    );
  });
});
