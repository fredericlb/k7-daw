// cameras.spec.ts — the remote cameras' hub as a client sees it: the list and the ticket behind the session,
// the signalling socket behind the ticket, and the frames it carries between one account's camera and viewers.
// Node's own WebSocket client, since a spec needs no browser; a refused upgrade is read with a bare request.
import { request as httpRequest } from "node:http";
import { request as httpsRequest } from "node:https";
import { type APIRequestContext, expect, test } from "@playwright/test";
import { sessionCookies } from "../../src/session.ts";
import { until } from "../../src/until.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";
const wsBase = base.replace(/^http/, "ws");

const headers = () => ({
  cookie: sessionCookies(base)
    .map((c) => `${c.name}=${c.value}`)
    .join("; "),
});

type Frame = Record<string, unknown>;

/** A socket, the frames it receives in order, and how it closed. */
class Socket {
  readonly ws: WebSocket;
  private readonly queue: Frame[] = [];
  private readonly waiting: ((f: Frame) => void)[] = [];
  readonly closed: Promise<number>;

  constructor(query: string) {
    this.ws = new WebSocket(`${wsBase}/api/cameras/ws?${query}`);
    this.ws.addEventListener("message", (e) => {
      const f = JSON.parse(String(e.data)) as Frame;
      const w = this.waiting.shift();
      if (w) w(f);
      else this.queue.push(f);
    });
    this.closed = new Promise((r) =>
      this.ws.addEventListener("close", (e) => r(e.code)),
    );
  }

  opened(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.ws.addEventListener("open", () => resolve());
      this.ws.addEventListener("error", () => reject(new Error("refused")));
    });
  }

  next(): Promise<Frame> {
    const f = this.queue.shift();
    if (f) return Promise.resolve(f);
    return new Promise((r) => this.waiting.push(r));
  }

  send(f: Frame): void {
    this.ws.send(JSON.stringify(f));
  }
}

async function ticket(request: APIRequestContext): Promise<string> {
  const res = await request.post("/api/cameras/ticket", {
    headers: headers(),
  });
  expect(res.status()).toBe(200);
  const body = await res.json();
  expect(Array.isArray(body.iceServers)).toBe(true);
  return body.ticket;
}

async function open(request: APIRequestContext, query: string) {
  const s = new Socket(`ticket=${await ticket(request)}&${query}`);
  await s.opened();
  return s;
}

async function names(request: APIRequestContext): Promise<string[]> {
  const res = await request.get("/api/cameras", { headers: headers() });
  expect(res.status()).toBe(200);
  return (await res.json()).map((c: { name: string }) => c.name);
}

/** The status an upgrade to the camera socket is answered with; 101 when it was taken. */
function upgradeStatus(query: string): Promise<number> {
  const url = new URL(`/api/cameras/ws?${query}`, base);
  const send = url.protocol === "https:" ? httpsRequest : httpRequest;
  return new Promise((resolve, reject) => {
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
    req.on("upgrade", (_res, socket) => {
      socket.destroy();
      resolve(101);
    });
    req.on("error", reject);
    req.end();
  });
}

test("the list and the ticket sit behind the session", async ({
  playwright,
}) => {
  const anonymous = await playwright.request.newContext({ baseURL: base });
  expect((await anonymous.get("/api/cameras")).status()).toBe(401);
  expect((await anonymous.post("/api/cameras/ticket")).status()).toBe(401);
  await anonymous.dispose();
});

test("a camera is listed under its cleaned, unique name while its socket is open", async ({
  request,
}) => {
  const before = await names(request);
  expect(before).not.toContain("Conformance cam");
  const a = await open(request, "role=camera&name=%20%20Conformance%09cam%20");
  expect(await a.next()).toEqual({ type: "hello", name: "Conformance cam" });
  const b = await open(request, "role=camera&name=conformance%20CAM");
  expect(await b.next()).toEqual({
    type: "hello",
    name: "conformance CAM (2)",
  });
  expect(await names(request)).toEqual([
    ...before,
    "Conformance cam",
    "conformance CAM (2)",
  ]);
  a.ws.close();
  b.ws.close();
  await Promise.all([a.closed, b.closed]);
  // A closed socket leaves the list a moment after its close: the list is asked until it has.
  expect(
    await until(
      () => names(request),
      (n) => n.length === before.length,
    ),
  ).toEqual(before);
});

test("a viewer's negotiation reaches the camera it named, and back", async ({
  request,
}) => {
  const cam = await open(request, "role=camera&name=Conformance%20stage");
  const { name } = await cam.next();
  const viewer = await open(request, "role=viewer");
  viewer.send({ type: "want", label: `remote:${String(name).toLowerCase()}` });
  const want = await cam.next();
  expect(want).toMatchObject({
    type: "want",
    label: "remote:conformance stage",
  });
  const from = want.from as string;
  expect(typeof from).toBe("string");
  expect(await cam.next()).toEqual({ type: "viewers", count: 1 });
  cam.send({ type: "offer", to: from, sdp: { type: "offer", sdp: "o" } });
  expect(await viewer.next()).toEqual({
    type: "offer",
    sdp: { type: "offer", sdp: "o" },
    problem: null,
  });
  cam.send({
    type: "ice",
    to: from,
    candidate: { candidate: "c", sdpMid: "0" },
  });
  expect(await viewer.next()).toEqual({
    type: "ice",
    candidate: { candidate: "c", sdpMid: "0" },
  });
  viewer.send({ type: "answer", sdp: { type: "answer", sdp: "a" } });
  expect(await cam.next()).toEqual({
    type: "answer",
    sdp: { type: "answer", sdp: "a" },
    from,
  });
  viewer.send({ type: "ice", candidate: { sdpMLineIndex: 0 } });
  expect(await cam.next()).toEqual({
    type: "ice",
    candidate: { sdpMLineIndex: 0 },
    from,
  });
  viewer.ws.close();
  expect(await cam.next()).toEqual({ type: "gone", from });
  expect(await cam.next()).toEqual({ type: "viewers", count: 0 });
  cam.ws.close();
  await cam.closed;
});

test("a camera that goes off closes its viewers and leaves the list", async ({
  request,
}) => {
  const cam = await open(request, "role=camera&name=Conformance%20off");
  await cam.next();
  const viewer = await open(request, "role=viewer");
  viewer.send({ type: "want", label: "remote:Conformance off" });
  await cam.next();
  cam.ws.close();
  expect(await viewer.closed).toBe(1001);
  expect(
    await until(
      () => names(request),
      (n) => !n.includes("Conformance off"),
    ),
  ).not.toContain("Conformance off");
});

test("an upgrade without a good ticket is refused 401, a bad role 400", async ({
  request,
}) => {
  expect(await upgradeStatus("role=viewer")).toBe(401);
  const t = await ticket(request);
  const [payload] = t.split(".");
  expect(await upgradeStatus(`ticket=${payload}.forged&role=viewer`)).toBe(401);
  expect(await upgradeStatus(`ticket=${t}&role=boss`)).toBe(400);
  expect(await upgradeStatus(`ticket=${t}`)).toBe(400);
});

test("an account past its cameras or its viewers is refused 429", async ({
  request,
}) => {
  const open8: Socket[] = [];
  const on = (await names(request)).length;
  for (let i = on; i < 8; i++)
    open8.push(
      await open(request, `role=camera&name=Conformance%20cap%20${i}`),
    );
  const t = await ticket(request);
  expect(await upgradeStatus(`ticket=${t}&role=camera&name=one-more`)).toBe(
    429,
  );
  const viewers: Socket[] = [];
  for (let i = 0; i < 32; i++) viewers.push(await open(request, "role=viewer"));
  expect(await upgradeStatus(`ticket=${t}&role=viewer`)).toBe(429);
  for (const s of [...open8, ...viewers]) s.ws.close();
  await Promise.all([...open8, ...viewers].map((s) => s.closed));
  expect(
    await until(
      () => names(request),
      (n) => n.length === on,
    ),
  ).toHaveLength(on);
});

test("a ticket expires after a minute", async ({ request }) => {
  test.setTimeout(90_000);
  const t = await ticket(request);
  // Time is the subject: a minute is the ticket's life, and only a minute passing shows it ends.
  await new Promise((r) => setTimeout(r, 61_000));
  expect(await upgradeStatus(`ticket=${t}&role=viewer`)).toBe(401);
});
