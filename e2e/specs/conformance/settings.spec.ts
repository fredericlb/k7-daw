// settings.spec.ts — an account's AI settings: their gates, their refusals, and a
// write read back; only the front serves them (e2e/docs/conformance.md). Every write is put back.
import { expect, test } from "@playwright/test";
import { E2E_USER, sessionCookies } from "../../src/session.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";

test.describe.configure({ mode: "serial" });

const cookieOf = (groups: string[] = []) => ({
  cookie: sessionCookies(base, { ...E2E_USER, groups })
    .map((c) => `${c.name}=${c.value}`)
    .join("; "),
});

test("the AI settings answer the account only, and show its key as a hint", async ({
  request,
  playwright,
}) => {
  const anonymous = await playwright.request.newContext({ baseURL: base });
  expect((await anonymous.get("/api/ai/settings")).status()).toBe(401);
  expect((await anonymous.get("/api/ai/organize")).status()).toBe(401);
  await anonymous.dispose();
  const headers = cookieOf();
  const view = await (
    await request.get("/api/ai/settings", { headers })
  ).json();
  expect(Object.keys(view)).toEqual(["model", "defaultModel", "keyHint"]);
  const organize = await (
    await request.get("/api/ai/organize", { headers })
  ).json();
  expect(organize.enabled).toBe(view.keyHint !== null);
  const same = await request.put("/api/ai/settings", { headers, data: {} });
  expect(await same.json()).toEqual(view);
});

test("a model turn answers the account only, and is refused before Claude is asked", async ({
  request,
  playwright,
}) => {
  const anonymous = await playwright.request.newContext({ baseURL: base });
  const turn = await anonymous.post("/api/ai/organize/turn", { data: {} });
  expect(turn.status()).toBe(401);
  await anonymous.dispose();
  const headers = cookieOf();
  const view = await (
    await request.get("/api/ai/settings", { headers })
  ).json();
  // No key: nothing to spend. A key: a body that is no turn is refused before a request leaves.
  const res = await request.post("/api/ai/organize/turn", {
    headers,
    data: {},
  });
  expect(res.status()).toBe(view.keyHint === null ? 404 : 400);
});

test("an AI settings patch is refused with zod's own words", async ({
  request,
}) => {
  const headers = cookieOf();
  const put = (data: unknown) =>
    request.put("/api/ai/settings", { headers, data });
  const stranger = await put({ conformance: 1 });
  expect(stranger.status()).toBe(400);
  expect(await stranger.json()).toEqual({
    error: 'Unrecognized key: "conformance"',
  });
  const long = await put({ model: "m".repeat(201), apiKey: 5 });
  expect(await long.json()).toEqual({
    error:
      "Invalid input: expected string, received number; Too big: expected string to have <=200 characters",
  });
});

test("the AI settings saved through the front read back, and are put back through it", async ({
  request,
}) => {
  const headers = cookieOf();
  const before = await (
    await request.get("/api/ai/settings", { headers })
  ).json();
  const model = `conformance-${Date.now()}`;
  const key =
    before.keyHint === null ? `sk-conformance-${Date.now()}-key` : undefined;
  try {
    expect(
      (
        await request.put("/api/ai/settings", {
          headers,
          data: { model, apiKey: key },
        })
      ).status(),
    ).toBe(200);
    const read = await (
      await request.get("/api/ai/settings", { headers })
    ).json();
    expect(read.model).toBe(model);
    if (key) expect(read.keyHint).toBe(`${key.slice(0, 7)}…${key.slice(-4)}`);
    const organize = await (
      await request.get("/api/ai/organize", { headers })
    ).json();
    expect(organize).toEqual({ enabled: true, model });
  } finally {
    // Through the front, the only implementation that serves the AI settings now.
    const back = await request.put("/api/ai/settings", {
      headers,
      data: { model: before.model, ...(key ? { apiKey: null } : {}) },
    });
    expect(await back.json()).toEqual(before);
  }
});
