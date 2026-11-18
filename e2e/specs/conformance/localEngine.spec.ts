// localEngine.spec.ts — a machine's own backend as its pages and its tray see it: /api/local, the engine's snapshot and
// event stream, the commands a page may send, the tray's status, the remote forward, and the
// internal seam's local half. Skipped against a backend that is no machine's own (apps/k7/backend-rs/docs/local.md).
import { type APIRequestContext, expect, test } from "@playwright/test";
import { sessionCookies } from "../../src/session.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";
const trayToken = process.env.CONFORMANCE_TRAY_TOKEN;
const internalSecret = process.env.CONFORMANCE_INTERNAL_SECRET;

const cookie = () =>
  sessionCookies(base)
    .map((c) => `${c.name}=${c.value}`)
    .join("; ");

/** What `/api/local` answers, or null when this backend is no machine's own. */
async function localInfo(
  request: APIRequestContext,
): Promise<{ engine: boolean; version: string } | null> {
  const res = await request.get("/api/local");
  if (res.status() !== 200) return null;
  const body = await res.json().catch(() => null);
  return body && typeof body.engine === "boolean" ? body : null;
}

test.describe("a machine's own backend", () => {
  test.beforeEach(async ({ request }) => {
    test.skip(!(await localInfo(request)), "not a machine's own backend");
  });

  test("names itself and its release", async ({ request }) => {
    const info = await localInfo(request);
    expect(typeof info?.version).toBe("string");
  });

  test("the tray's status answers its bearer only", async ({ request }) => {
    const none = await request.get("/api/local/tray");
    expect(none.status()).toBe(401);
    expect(await none.json()).toEqual({ error: "unauthorized" });
    const wrong = await request.get("/api/local/tray", {
      headers: { authorization: "Bearer not-the-tray" },
    });
    expect(wrong.status()).toBe(401);
    test.skip(!trayToken, "CONFORMANCE_TRAY_TOKEN not given");
    const res = await request.get("/api/local/tray", {
      headers: { authorization: `Bearer ${trayToken}` },
    });
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(Object.keys(body)).toEqual(["icon", "line", "downloads"]);
    expect(["stopped", "loading", "running", "error"]).toContain(body.icon);
  });

  test("the internal seam's local half needs this machine and its secret", async ({
    request,
  }) => {
    const res = await request.get("/api/internal/engine/local");
    expect(res.status()).toBe(401);
    test.skip(!internalSecret, "CONFORMANCE_INTERNAL_SECRET not given");
    const headers = { "x-k7-internal": internalSecret ?? "" };
    const local = await request.get("/api/internal/engine/local", { headers });
    expect([200, 404]).toContain(local.status());
    expect(local.headers()["cache-control"]).toBe("no-store");
    if (local.status() === 404) return;
    expect(typeof (await local.json()).answered).toBe("boolean");
    const bad = await request.post("/api/internal/engine/local/commands", {
      headers,
      data: { cmd: "engine.stop", arg: "" },
    });
    expect(bad.status()).toBe(400);
    const gone = await request.post("/api/internal/engine/local/commands", {
      headers,
      data: { cmd: "weburi.remove", arg: "https://conformance.invalid" },
    });
    expect(gone.status()).toBe(204);
  });
});

test.describe("the machine's engine", () => {
  test.beforeEach(async ({ request }) => {
    test.skip(
      !(await localInfo(request))?.engine,
      "this backend runs no engine",
    );
  });

  test("its snapshot sits behind the session", async ({
    request,
    playwright,
  }) => {
    const anonymous = await playwright.request.newContext({ baseURL: base });
    expect((await anonymous.get("/api/local/engine")).status()).toBe(401);
    await anonymous.dispose();
    const res = await request.get("/api/local/engine", {
      headers: { cookie: cookie() },
    });
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(Object.keys(body)).toEqual(["snapshot", "down"]);
    if (body.snapshot) {
      expect(typeof body.snapshot.status).toBe("string");
      expect(Array.isArray(body.snapshot.slots)).toBe(true);
      expect(body.snapshot.owns).toBeUndefined();
    }
  });

  test("its event stream's first frame arrives at once", async () => {
    const controller = new AbortController();
    const res = await fetch(new URL("/api/local/engine/events", base), {
      headers: { cookie: cookie() },
      signal: controller.signal,
    });
    expect(res.headers.get("content-type")).toBe(
      "text/event-stream; charset=utf-8",
    );
    expect(res.headers.get("cache-control")).toBe("no-cache, no-transform");
    const reader = res.body?.getReader();
    let text = "";
    const deadline = Date.now() + 5_000;
    while (!text.includes("\n\n")) {
      const chunk = await Promise.race([
        reader?.read(),
        new Promise<never>((_, reject) =>
          setTimeout(
            () => reject(new Error("no frame within 5 s: buffered")),
            Math.max(0, deadline - Date.now()),
          ),
        ),
      ]);
      text += new TextDecoder().decode(chunk?.value);
    }
    controller.abort();
    expect(text.startsWith("retry: 2000\n")).toBe(true);
    const data = /data: (.*)\n\n/.exec(text)?.[1] ?? "{}";
    expect(Object.keys(JSON.parse(data))).toEqual(["snapshot", "down"]);
  });

  test("a page's commands are the allow-list's only", async ({ request }) => {
    const headers = { cookie: cookie() };
    const send = (data: unknown) =>
      request.post("/api/local/engine/cmd", { headers, data });
    const refused = await send({ cmd: "shutdown" });
    expect(refused.status()).toBe(400);
    expect(await refused.json()).toEqual({ error: "command not allowed" });
    const pairing = await send({ cmd: "weburi.pair", arg: "x" });
    expect(pairing.status()).toBe(400);
    const typed = await send({ cmd: "engine.start", arg: 3 });
    expect(typed.status()).toBe(400);
    expect(await typed.json()).toEqual({ error: "arg must be a string" });
    const harmless = await send({
      cmd: "weburi.remove",
      arg: "https://conformance.invalid",
    });
    expect(harmless.status()).toBe(200);
    expect(await harmless.json()).toEqual({ ok: true });
  });

  test("a slot must name itself and carry its document", async ({
    request,
  }) => {
    const res = await request.post("/api/local/engine/slots", {
      headers: { cookie: cookie() },
      data: { id: 3, doc: {} },
    });
    expect(res.status()).toBe(400);
    expect(await res.json()).toEqual({ error: "a slot is {id, …, doc}" });
  });
});
