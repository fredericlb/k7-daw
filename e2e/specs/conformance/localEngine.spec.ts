// localEngine.spec.ts — a machine's own backend as its pages see it: /api/local and the
// internal seam's local half. Skipped against a backend that is no machine's own (apps/k7/backend-rs/docs/local.md).
// The engine routes (snapshot, events, commands, slots) run in k7-local's tests/all/front.rs, against a fake supervisor.
import { type APIRequestContext, expect, test } from "@playwright/test";

const internalSecret = process.env.CONFORMANCE_INTERNAL_SECRET;

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

  test("the internal seam's local half needs this machine and its secret", async ({
    request,
  }) => {
    const res = await request.get("/api/internal/engine/local");
    expect(res.status()).toBe(401);
    test.skip(!internalSecret, "CONFORMANCE_INTERNAL_SECRET not given");
    const headers = { "x-k7-internal": internalSecret ?? "" };
    const local = await request.get("/api/internal/engine/local", { headers });
    // A front that runs no local engine answers nothing here: the path is unclaimed, and asks a session.
    if (local.status() !== 200)
      return expect([401, 404]).toContain(local.status());
    expect(local.headers()["cache-control"]).toBe("no-store");
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
