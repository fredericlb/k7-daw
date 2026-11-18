// The shared JSON parser runs before any guard, so its limit is what a stranger can make
// this process parse. See ../docs/hardening.md.

import type { Server } from "node:http";
import express from "express";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { jsonBodies } from "./bodyLimits.ts";

let server: Server;
let base: string;

beforeAll(async () => {
  const app = express();
  app.use(jsonBodies());
  // Every route answers whether the body was parsed, and how much of it there was.
  app.use((req, res) =>
    res.json({
      parsed: req.readableEnded,
      size: JSON.stringify(req.body ?? null).length,
    }),
  );
  server = await new Promise<Server>((ok) => {
    const s = app.listen(0, "127.0.0.1", () => ok(s));
  });
  base = `http://127.0.0.1:${(server.address() as { port: number }).port}`;
});

afterAll(() => new Promise<void>((ok) => server.close(() => ok())));

const send = (path: string, kb: number): Promise<Response> =>
  fetch(`${base}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ text: "x".repeat(kb * 1024) }),
  });

describe("the shared JSON parser", () => {
  it("takes a small body anywhere", async () => {
    const r = await send("/api/prefs", 100);
    expect(r.status).toBe(200);
    expect(((await r.json()) as { parsed: boolean }).parsed).toBe(true);
  });

  it("refuses a large one before any guard reads it", async () => {
    expect((await send("/api/prefs", 400)).status).toBe(413);
    expect((await send("/api/engines/heartbeat", 400)).status).toBe(413);
  });
});
