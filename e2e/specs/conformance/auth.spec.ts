// auth.spec.ts — the session as a client sees it: /auth/me and logout, and the library's tokens.
import { expect, test } from "@playwright/test";
import { E2E_USER, sessionCookies } from "../../src/session.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";
// A slots read that takes a bearer: a verified one reaches the owner check (404, no such engine), a refused one is a 401.
const bearerRead = "/api/engines/nobody/chains";

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

test("a token minted is listed, accepted and revoked", async () => {
  const cookie = e2eCookie();
  const json = { cookie, "content-type": "application/json" };
  expect((await http(`${base}/api/token`, { method: "POST" })).status).toBe(
    401,
  );
  const made = await http(`${base}/api/token`, {
    method: "POST",
    headers: json,
    body: JSON.stringify({ label: "  conformance  " }),
  });
  expect(made.status).toBe(200);
  const minted = JSON.parse(made.body);
  expect(minted).toMatchObject({ label: "conformance", lastUsed: null });
  expect(minted.token.split(".")).toHaveLength(2);
  try {
    const list = await http(`${base}/api/token`, { headers: { cookie } });
    expect(list.headers.get("cache-control")).toBe("no-store");
    const ids = JSON.parse(list.body).tokens.map((t: { id: string }) => t.id);
    expect(ids).toContain(minted.id);
    const accepted = await http(`${base}${bearerRead}`, {
      headers: { authorization: `Bearer ${minted.token}` },
    });
    expect(accepted.status).toBe(404);
    const beside = await http(`${base}/api/token`, {
      headers: { cookie, authorization: "Bearer not-a-token" },
    });
    expect(beside.status).toBe(401);
    expect(JSON.parse(beside.body).error).toBe("invalid token");
    const gone = await http(`${base}/api/token/${minted.id}`, {
      method: "DELETE",
      headers: { cookie },
    });
    expect(gone.status).toBe(200);
    const read = await http(`${base}${bearerRead}`, {
      headers: { authorization: `Bearer ${minted.token}` },
    });
    expect(read.status).toBe(401);
    const twice = await http(`${base}/api/token/${minted.id}`, {
      method: "DELETE",
      headers: { cookie },
    });
    expect(twice.status).toBe(404);
  } finally {
    await http(`${base}/api/token/${minted.id}`, {
      method: "DELETE",
      headers: { cookie },
    });
  }
});
