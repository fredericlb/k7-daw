// auth.spec.ts — the session as a client sees it: /auth/me hands over the install's token, and only that token signs in.
import { expect, test } from "@playwright/test";
import { E2E_USER, sessionCookies } from "../../src/session.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";

const e2eCookie = (): string =>
  sessionCookies(base)
    .map((c) => `${c.name}=${c.value}`)
    .join("; ");

test("/auth/me is never cached, and hands the token to a page that holds none", async () => {
  const out = await fetch(`${base}/auth/me`);
  expect(out.status).toBe(200);
  expect(out.headers.get("cache-control")).toBe("no-store");
  expect(JSON.parse(await out.text()).user.sub).toBe(E2E_USER.sub);
  const granted = out.headers.getSetCookie()[0] ?? "";
  expect(granted).toMatch(/^k7\.sid=[0-9a-f]+;/);
  expect(granted).toMatch(/HttpOnly/);
  expect(granted).toMatch(/SameSite=Strict/);
  const held = await fetch(`${base}/auth/me`, {
    headers: { cookie: e2eCookie() },
  });
  expect(held.status).toBe(200);
  expect(held.headers.getSetCookie()).toEqual([]);
});

test("a page on another site is given no token", async () => {
  const cross = await fetch(`${base}/auth/me`, {
    headers: { "sec-fetch-site": "cross-site" },
  });
  expect(cross.status).toBe(403);
  expect(cross.headers.getSetCookie()).toEqual([]);
});

test("a route behind the session answers the token alone", async () => {
  const bare = await fetch(`${base}/api/preferences`);
  expect(bare.status).toBe(401);
  const wrong = await fetch(`${base}/api/preferences`, {
    headers: { cookie: "k7.sid=not-the-token" },
  });
  expect(wrong.status).toBe(401);
  const held = await fetch(`${base}/api/preferences`, {
    headers: { cookie: e2eCookie() },
  });
  expect(held.status).toBe(200);
});
