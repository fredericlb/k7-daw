// auth.spec.ts — the session as a client sees it: /auth/me and logout.
import { expect, test } from "@playwright/test";
import { E2E_USER, sessionCookies } from "../../src/session.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";
// A slots read that takes a bearer: a verified one reaches the engine check (404, no such engine), a refused one is a 401.
const _bearerRead = "/api/engines/nobody/chains";

const e2eCookie = (): string =>
  sessionCookies(base)
    .map((c) => `${c.name}=${c.value}`)
    .join("; ");

interface Reply {
  status: number;
  location: string;
  /** The `name=value` pairs the answer set, as a browser sends them back. */
  cookies: string;
  setCookies: string[];
  headers: Headers;
  body: string;
}

async function http(
  url: string,
  init: RequestInit & { headers?: Record<string, string> } = {},
): Promise<Reply> {
  const res = await fetch(url, { redirect: "manual", ...init });
  const setCookies = res.headers.getSetCookie();
  return {
    status: res.status,
    location: res.headers.get("location") ?? "",
    cookies: setCookies
      .map((c) => c.split(";")[0])
      .filter((p) => p.indexOf("=") < p.length - 1)
      .join("; "),
    setCookies,
    headers: res.headers,
    body: await res.text(),
  };
}

test("/auth/me is never cached, signed in or out", async () => {
  const out = await http(`${base}/auth/me`);
  expect(out.status).toBe(401);
  expect(out.headers.get("cache-control")).toBe("no-store");
  const signed = await http(`${base}/auth/me`, {
    headers: { cookie: e2eCookie() },
  });
  expect(signed.status).toBe(200);
  expect(signed.headers.get("cache-control")).toBe("no-store");
  expect(JSON.parse(signed.body).user.sub).toBe(E2E_USER.sub);
});

test("logout refuses another site and clears the cookie for this one", async () => {
  const cross = await http(`${base}/auth/logout`, {
    method: "POST",
    headers: { cookie: e2eCookie(), origin: "https://evil.example" },
  });
  expect(cross.status).toBe(403);
  const out = await http(`${base}/auth/logout`, {
    method: "POST",
    headers: { cookie: e2eCookie() },
  });
  expect(out.status).toBe(200);
  expect(JSON.parse(out.body)).toEqual({ ok: true });
  expect(out.setCookies[0]).toMatch(
    /^studio\.sid=; path=\/; expires=Thu, 01 Jan 1970 00:00:00 GMT;.* httponly$/,
  );
  expect(out.setCookies[1]).toMatch(/^studio\.sid\.sig=[\w-]+; path=\//);
});
