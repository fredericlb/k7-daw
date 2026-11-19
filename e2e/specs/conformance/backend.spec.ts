// backend.spec.ts — what the front answers: the token cookie, a body, a stream and an upgrade (e2e/docs/conformance.md).
import { request as httpRequest } from "node:http";
import { request as httpsRequest } from "node:https";
import { expect, test } from "@playwright/test";
import { E2E_USER, sessionCookies } from "../../src/session.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";

/** The Cookie header a signed-in e2e account sends. */
function cookie(): string {
  return sessionCookies(base)
    .map((c) => `${c.name}=${c.value}`)
    .join("; ");
}

test("health answers without a session", async ({ request }) => {
  const res = await request.get("/api/health");
  expect(res.status()).toBe(200);
  expect((await res.json()).ok).toBe(true);
});

test("the install's token is read", async ({ request }) => {
  expect((await request.get("/api/preferences")).status()).toBe(401);
  const res = await request.get("/auth/me", { headers: { cookie: cookie() } });
  expect(res.status()).toBe(200);
  expect((await res.json()).user.email).toBe(E2E_USER.email);
});

test("a body reaches the backend and comes back", async ({ request }) => {
  const headers = { cookie: cookie() };
  const prefs = await (
    await request.get("/api/preferences", { headers })
  ).json();
  const put = await request.put("/api/preferences", { headers, data: prefs });
  expect(put.status()).toBe(200);
  expect(
    await (await request.get("/api/preferences", { headers })).json(),
  ).toEqual(prefs);
});

test("an upgrade nobody serves is refused with 404", async () => {
  const url = new URL("/api/no-such-socket", base);
  const send = url.protocol === "https:" ? httpsRequest : httpRequest;
  const status = await new Promise<number>((resolve, reject) => {
    const req = send(url, {
      rejectUnauthorized: false,
      headers: {
        connection: "Upgrade",
        upgrade: "websocket",
        "sec-websocket-version": "13",
        "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
      },
    });
    req.on("response", (res) => resolve(res.statusCode ?? 0));
    req.on("upgrade", () => resolve(101));
    req.on("error", reject);
    req.end();
  });
  expect(status).toBe(404);
});

test("health names the release", async ({ request }) => {
  const body = await (await request.get("/api/health")).json();
  expect(typeof body.version).toBe("string");
  expect(typeof body.build).toBe("string");
});

test("preferences refuse what they would store short or wrong", async ({
  request,
  playwright,
}) => {
  const headers = { cookie: cookie() };
  const unknown = await request.put("/api/preferences", {
    headers,
    data: { conformanceUnknown: 1 },
  });
  expect(unknown.status()).toBe(400);
  expect((await unknown.json()).error).toContain("conformanceUnknown");
  const wrong = await request.put("/api/preferences", {
    headers,
    data: { engineId: 3 },
  });
  expect(wrong.status()).toBe(400);
  // A context of its own: a device's backend answered the PUTs with a local session, which `request` keeps.
  const anonymous = await playwright.request.newContext({ baseURL: base });
  expect((await anonymous.get("/api/preferences")).status()).toBe(401);
  await anonymous.dispose();
});

test("a write from another site is refused, and its preflight granted nothing", async ({
  request,
}) => {
  const evil = "https://evil.example";
  const put = await request.put("/api/preferences", {
    headers: { cookie: cookie(), origin: evil },
    data: {},
  });
  expect(put.status()).toBe(403);
  const pre = await request.fetch("/api/preferences", {
    method: "OPTIONS",
    headers: { origin: evil },
  });
  expect(pre.status()).toBe(204);
  expect(pre.headers()["access-control-allow-origin"]).toBeUndefined();
});

test("the front owns its own health route", async ({ request }) => {
  const res = await request.get("/api/front/health");
  expect((await res.json()).ok).toBe(true);
});
