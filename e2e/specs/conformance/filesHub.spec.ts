// filesHub.spec.ts — the file hub as a client sees it, in front of an account's file service: the hello and its signed
// ticket, `/api/files` gated by the ticket (header or ?token=, an engine after `~`), the identity headers a client
// cannot choose, a body streamed up and back, the service's answers passed through, and the internal seam's refusals.
import { createHash, createHmac, randomBytes } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { type APIRequestContext, expect, test } from "@playwright/test";
import { base, signedIn } from "../../src/instance.ts";
import { backendEnv, E2E_USER } from "../../src/session.ts";
import {
  accountDir,
  FILES_USER,
  filesRoot,
  freshWorkspace,
} from "../../src/sync.ts";
import { until } from "../../src/until.ts";

const ws = freshWorkspace();
let ticket = "";
const email = E2E_USER.email as string;

/** A ticket signed as the hub signs them: base64url JSON `{u, exp, a?}`, a dot, its HMAC-SHA256 under `<SESSION_SECRET>:files`. */
function signTicket(payload: object): string {
  const key = `${backendEnv().SESSION_SECRET}:files`;
  const body = Buffer.from(JSON.stringify(payload)).toString("base64url");
  return `${body}.${createHmac("sha256", key).update(body).digest("base64url")}`;
}

const withTicket = (t = ticket, extra: Record<string, string> = {}) => ({
  "x-studio-engine-token": t,
  ...extra,
});
const inWorkspace = (extra: Record<string, string> = {}) =>
  withTicket(ticket, { "x-studio-workspace": ws, ...extra });

async function names(
  request: APIRequestContext,
  headers: Record<string, string>,
  query = "",
) {
  const res = await request.get(`/api/files/workspaces${query}`, { headers });
  return {
    status: res.status(),
    names: res.ok()
      ? ((await res.json()).workspaces as { name: string }[]).map((w) => w.name)
      : [],
  };
}

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ request }) => {
  const hello = await request.get("/api/files-hub", {
    headers: signedIn(FILES_USER),
  });
  ticket = (await hello.json()).ticket;
  const made = await request.post("/api/files/workspaces", {
    data: { name: ws },
    headers: withTicket(),
  });
  expect(await made.json()).toEqual({ created: true });
});

test.afterAll(async ({ request }) => {
  if (!ticket) return;
  // A workspace removed before its replica joined the relay can be brought back by a listing: removed until it stays gone.
  const gone = async () =>
    !(await names(request, withTicket())).names.includes(ws) &&
    !(filesRoot && existsSync(join(accountDir(FILES_USER), "workspace", ws)));
  for (let round = 0; round < 3; round++) {
    await request.post("/api/files/workspaces/remove", {
      data: { name: ws },
      headers: withTicket(),
    });
    if ((await until(gone, (g) => g, 5_000)) && (await gone())) break;
  }
});

test("the hello turns a session into a ticket naming the account, signed, for twelve hours", async ({
  request,
}) => {
  expect((await request.get("/api/files-hub")).status()).toBe(401);
  const res = await request.get("/api/files-hub", {
    headers: signedIn(FILES_USER),
  });
  const hello = await res.json();
  expect(Object.keys(hello)).toEqual(["path", "ticket"]);
  expect(hello).toMatchObject({ path: "/api/files" });
  const [body] = hello.ticket.split(".");
  const payload = JSON.parse(Buffer.from(body, "base64url").toString("utf8"));
  expect(Object.keys(payload)).toEqual(["u", "exp"]);
  expect(payload.u).toBe(email);
  expect(Math.abs(payload.exp - (Date.now() + 12 * 3600_000))).toBeLessThan(
    60_000,
  );
  expect(hello.ticket).toBe(signTicket(payload));
});

test("/api/files takes the ticket in its header or in ?token=, and refuses a forged or expired one", async ({
  request,
}) => {
  const none = await request.get("/api/files/workspaces");
  expect([none.status(), await none.json()]).toEqual([
    401,
    { error: "no valid files ticket, and no session" },
  ]);
  expect(await names(request, withTicket())).toEqual({
    status: 200,
    names: expect.arrayContaining([ws]),
  });
  expect(
    await names(request, {}, `?token=${encodeURIComponent(ticket)}`),
  ).toEqual({ status: 200, names: expect.arrayContaining([ws]) });
  // One the spec signs with the instance's key is as good as the hub's own.
  expect(
    (
      await names(
        request,
        withTicket(signTicket({ u: email, exp: Date.now() + 60_000 })),
      )
    ).names,
  ).toContain(ws);
  for (const bad of [
    signTicket({ u: email, exp: Date.now() - 1000 }),
    signTicket({ u: "", exp: Date.now() + 60_000 }),
    signTicket({ exp: Date.now() + 60_000 }),
    `${ticket.split(".")[0]}.${randomBytes(32).toString("base64url")}`,
    ticket.split(".")[0],
    "not-a-ticket",
  ])
    expect((await names(request, withTicket(bad))).status).toBe(401);
});

test("a same-origin caller with a session and no ticket is let through", async ({
  request,
}) => {
  expect((await names(request, signedIn(FILES_USER))).status).toBe(200);
});

test("an engine named after `~` and not claimed leaves the files reachable", async ({
  request,
}) => {
  const engine = `conformance-engine-${randomBytes(4).toString("hex")}`;
  expect(
    (await names(request, withTicket(`${ticket}~${engine}`))).names,
  ).toContain(ws);
  expect(
    (await names(request, withTicket(ticket, { "x-studio-engine-id": engine })))
      .names,
  ).toContain(ws);
});

test("the identity a client sends is replaced by the ticket's: user and hub secret", async ({
  request,
}) => {
  // Each of these, passed on, would make the account's service refuse: another owner, a secret not its own.
  const spoofed = withTicket(ticket, {
    "x-studio-user": "e2e@st0.app",
    "x-studio-hub-secret": "forged",
    authorization: "Bearer forged",
  });
  expect((await names(request, spoofed)).names).toContain(ws);
});

test("a body streams up through the hub and back, whole and by range", async ({
  request,
}) => {
  const bytes = randomBytes(6 * 1024 * 1024);
  const sha = (b: Buffer) => createHash("sha256").update(b).digest("hex");
  const up = await request.post("/api/files/upload?dir=.&name=stream.bin", {
    data: bytes,
    headers: inWorkspace({ "content-type": "application/octet-stream" }),
  });
  expect([up.status(), await up.json()]).toEqual([200, { path: "stream.bin" }]);
  // Chunked, with no length: the hub must pipe it rather than wait for a length.
  const chunked = await fetch(
    `${base}/api/files/upload?dir=.&name=chunked.bin`,
    {
      method: "POST",
      headers: inWorkspace({ "content-type": "application/octet-stream" }),
      body: new ReadableStream({
        start(c) {
          for (let i = 0; i < 8; i++)
            c.enqueue(bytes.subarray(i * 65536, (i + 1) * 65536));
          c.close();
        },
      }),
      duplex: "half",
    } as RequestInit,
  );
  expect([chunked.status, await chunked.json()]).toEqual([
    200,
    { path: "chunked.bin" },
  ]);
  const again = await request.post("/api/files/upload?dir=.&name=stream.bin", {
    data: Buffer.from("x"),
    headers: inWorkspace({ "content-type": "application/octet-stream" }),
  });
  expect(again.status()).toBe(409);
  const media = (path: string) =>
    `/api/files/media?${new URLSearchParams({ user: email, workspace: ws, path, token: ticket })}`;
  const whole = await request.get(media("stream.bin"));
  expect([whole.status(), whole.headers()["content-length"]]).toEqual([
    200,
    String(bytes.length),
  ]);
  expect(sha(await whole.body())).toBe(sha(bytes));
  const part = await request.get(media("stream.bin"), {
    headers: { range: "bytes=1048576-1048675" },
  });
  expect([part.status(), part.headers()["content-range"]]).toEqual([
    206,
    `bytes 1048576-1048675/${bytes.length}`,
  ]);
  expect(
    Buffer.compare(await part.body(), bytes.subarray(1048576, 1048676)),
  ).toBe(0);
  expect(sha(await (await request.get(media("chunked.bin"))).body())).toBe(
    sha(bytes.subarray(0, 8 * 65536)),
  );
  if (filesRoot) {
    const dir = join(accountDir(FILES_USER), "workspace", ws);
    expect(sha(readFileSync(join(dir, "stream.bin")))).toBe(sha(bytes));
    expect(existsSync(join(dir, "chunked.bin"))).toBe(true);
  }
});

test("the service's own answers come back as it gave them", async ({
  request,
}) => {
  const health = await request.get("/api/files/health", {
    headers: withTicket(),
  });
  expect([health.status(), await health.json()]).toEqual([200, { ok: true }]);
  const noWs = await request.get("/api/files/file?path=a.k7x", {
    headers: withTicket(),
  });
  expect([noWs.status(), await noWs.json()]).toEqual([
    400,
    { error: "missing workspace header" },
  ]);
  const gone = await request.get("/api/files/file?path=a.k7x", {
    headers: withTicket(ticket, { "x-studio-workspace": `${ws}-none` }),
  });
  expect(gone.status()).toBe(404);
  const missing = await request.get("/api/files/file?path=none.k7x", {
    headers: inWorkspace(),
  });
  expect(missing.status()).toBe(404);
});

test("the internal seam answers only its own file services' secrets", async ({
  request,
}) => {
  for (const [method, path] of [
    ["POST", "/libraries/sync"],
    ["GET", "/libraries/sync"],
    ["GET", "/nowhere"],
  ] as const)
    for (const headers of <Record<string, string>[]>[
      {},
      { "x-studio-hub-secret": "forged" },
      { authorization: "Bearer forged" },
      signedIn(FILES_USER),
      withTicket(),
    ]) {
      const res = await request.fetch(`/api/files-hub/internal${path}`, {
        method,
        headers,
        data: method === "POST" ? {} : undefined,
      });
      expect([method, path, res.status(), await res.json()]).toEqual([
        method,
        path,
        401,
        { error: "not one of this hub's file services" },
      ]);
    }
});
