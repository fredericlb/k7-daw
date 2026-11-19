// engines.spec.ts — the engine registry as an engine and a browser see it: the open hello, heartbeat and
// deregister, the session's list, select, release and WebRTC offer, the relay's upgrade
// and the internal seam's gate. An engine of its own per run, with no secret, so nothing stays.

import { randomBytes } from "node:crypto";
import { request as httpRequest } from "node:http";
import { request as httpsRequest } from "node:https";
import { expect, test } from "@playwright/test";
import { sessionCookies } from "../../src/session.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";

const signedIn = () => ({
  cookie: sessionCookies(base)
    .map((c) => `${c.name}=${c.value}`)
    .join("; "),
});

const id = `conformance-${randomBytes(6).toString("hex")}`;
const beat = (over: Record<string, unknown> = {}) => ({
  id,
  token: "tok-a",
  hostname: "conformance",
  machineFingerprint: "fp",
  addresses: { loopback: "127.0.0.1", lanIps: ["192.168.1.20"], port: 9 },
  performance: {
    score: 1,
    cores: 0,
    singleThread: 1,
    diskWriteMBps: null,
    fsyncPerSec: null,
    memoryGB: 1,
  },
  ...over,
});

/** A bare upgrade's status: an engine id, a ticket. */
function upgrade(path: string): Promise<number> {
  const u = new URL(path, base);
  const request = u.protocol === "https:" ? httpsRequest : httpRequest;
  return new Promise((resolve, reject) => {
    const r = request(u, {
      headers: {
        connection: "Upgrade",
        upgrade: "websocket",
        "sec-websocket-version": "13",
        "sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==",
      },
      rejectUnauthorized: false,
    });
    r.on("upgrade", (res, socket) => {
      socket.destroy();
      resolve(res.statusCode ?? 0);
    });
    r.on("response", (res) => {
      res.resume();
      resolve(res.statusCode ?? 0);
    });
    r.on("error", reject);
    r.end();
  });
}

test.describe.configure({ mode: "serial" });

test("says what it is, to anyone", { tag: "@tier4" }, async ({ request }) => {
  const res = await request.get("/api/registry/hello");
  expect(res.status()).toBe(200);
  expect(await res.json()).toEqual({ service: "k7-registry" });
});

test("refuses a heartbeat it cannot use, the reason in the body", {
  tag: "@tier4",
}, async ({ request }) => {
  const res = await request.post("/api/registry/heartbeat", { data: {} });
  expect(res.status()).toBe(400);
  expect((await res.json()).error).toBe(
    "heartbeat refused — id: Invalid input: expected string, received undefined; token: Invalid input: expected string, received undefined; addresses: Invalid input: expected object, received undefined",
  );
});

test("lists an engine whose rating is out of bounds, and says what it cleaned", {
  tag: "@tier4",
}, async ({ request }) => {
  const res = await request.post("/api/registry/heartbeat", { data: beat() });
  expect(res.status()).toBe(200);
  expect(await res.json()).toEqual({
    ok: true,
    cleaned: [
      "performance left out — cores: Too small: expected number to be >=1",
    ],
  });
  const other = await request.post("/api/registry/heartbeat", {
    data: beat({ token: "tok-b" }),
  });
  expect(other.status()).toBe(409);
  expect((await other.json()).error).toMatch(/freed 15 s after/);
});

test("lists it to the session, with a ticket and never its token", {
  tag: "@tier4",
}, async ({ request }) => {
  expect((await request.get("/api/engines")).status()).toBe(401);
  const res = await request.get("/api/engines", { headers: signedIn() });
  expect(res.status()).toBe(200);
  const mine = ((await res.json()) as Record<string, unknown>[]).find(
    (e) => e.id === id,
  );
  expect(mine).toMatchObject({
    id,
    hostname: "conformance",
    addresses: { loopback: "127.0.0.1", lanIps: ["192.168.1.20"], port: 9 },
    protocol: 1,
    assigned: false,
    heldProjects: [],
    legacy: true,
    sameSourceIp: true,
    access: "owner",
  });
  expect(mine?.token).toBeUndefined();
  expect(mine?.performance).toBeUndefined();
  expect(typeof mine?.proxyTicket).toBe("string");
});

test("tells a lapsed engine from a wrong token, and claims, renews and lets go", {
  tag: "@tier4",
}, async ({ request }) => {
  const select = (data: object) =>
    request.post("/api/engine/select", { data, headers: signedIn() });
  expect(
    (
      await request.post("/api/engine/select", { data: { id, token: "tok-a" } })
    ).status(),
  ).toBe(401);
  const gone = await select({ id: `${id}-gone`, token: "t" });
  expect(gone.status()).toBe(404);
  expect((await gone.json()).error).toMatch(/^engine gone/);
  const wrong = await select({ id, token: "nope" });
  expect(wrong.status()).toBe(403);
  expect((await wrong.json()).error).toMatch(/^bad token/);
  expect((await select({ id })).status()).toBe(400);
  const open = await select({ id, token: "tok-a", project: null });
  expect(open.status()).toBe(200);
  const { secret } = await open.json();
  expect(
    await (await select({ id, token: "tok-a", project: null, secret })).json(),
  ).toEqual({ ok: true, secret });
  const offer = await request.post(`/api/engine/${id}/rtc`, {
    data: { sdp: "v=0" },
    headers: signedIn(),
  });
  expect(offer.status()).toBe(503);
  expect(
    (
      await request.post(`/api/engine/${id}/rtc`, {
        data: { sdp: "" },
        headers: signedIn(),
      })
    ).status(),
  ).toBe(400);
  const release = await request.post("/api/engine/release", {
    data: { id, secret },
    headers: signedIn(),
  });
  expect(await release.json()).toEqual({ ok: true });
  const lost = await select({ id, token: "tok-a", project: null, secret });
  expect(lost.status()).toBe(409);
  expect(await lost.json()).toEqual({
    error: "project taken",
    heldBy: "",
    lost: "lapsed",
  });
  const refused = await request.post(`/api/engine/${id}/rtc`, {
    data: { sdp: "v=0" },
    headers: signedIn(),
  });
  expect(refused.status()).toBe(403);
  expect(await refused.json()).toEqual({
    error: "claim this engine before opening a direct link",
  });
});

test("relays only for a ticket of its own engine, and unlists on the engine's word", {
  tag: "@tier4",
}, async ({ request }) => {
  const list = (await (
    await request.get("/api/engines", { headers: signedIn() })
  ).json()) as { id: string; proxyTicket: string }[];
  const ticket = list.find((e) => e.id === id)?.proxyTicket ?? "";
  expect(
    await upgrade(
      `/api/engine/${id}-other/ws?ticket=${encodeURIComponent(ticket)}`,
    ),
  ).toBe(401);
  expect(await upgrade(`/api/engine/${id}/ws?ticket=forged`)).toBe(401);
  const mismatch = await request.post("/api/registry/deregister", {
    data: { id, token: "nope" },
  });
  expect([mismatch.status(), await mismatch.json()]).toEqual([
    403,
    { error: "id/token mismatch" },
  ]);
  const bye = await request.post("/api/registry/deregister", {
    data: { id, token: "tok-a" },
  });
  expect(await bye.json()).toEqual({ ok: true });
  expect(
    await upgrade(`/api/engine/${id}/ws?ticket=${encodeURIComponent(ticket)}`),
  ).toBe(404);
});

test("keeps the internal seam to this machine's processes that hold its secret", {
  tag: "@tier4",
}, async ({ request }) => {
  const res = await request.get("/api/internal/engine/hub");
  expect([res.status(), await res.json()]).toEqual([
    401,
    { error: "the internal API needs this process's secret" },
  ]);
});
