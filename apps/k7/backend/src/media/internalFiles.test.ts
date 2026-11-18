// internalFiles.test.ts — what the Rust front asks of this backend through the seam: a bearer's subject, and that
// this backend runs no file hub; every other file question is the front's.

import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import {
  INTERNAL_FILES_PATH,
  INTERNAL_SECRET_HEADER,
  internalGate,
} from "@studio/engine-registry/seam";
import express from "express";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { internalFilesRouter } from "./internalFiles.ts";

let server: Server;
let base: string;

beforeAll(async () => {
  const app = express();
  app.use(
    INTERNAL_FILES_PATH,
    internalGate("k"),
    internalFilesRouter({
      verifyToken: (t) => (t === "bearer-ann" ? "ann@x" : null),
    }),
  );
  server = createServer(app);
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", () => r()));
  base = `http://127.0.0.1:${(server.address() as AddressInfo).port}${INTERNAL_FILES_PATH}`;
});
afterAll(async () => {
  await new Promise((r) => server.close(r));
});

const post = (path: string, body: unknown, secret = "k") =>
  fetch(`${base}${path}`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      [INTERNAL_SECRET_HEADER]: secret,
    },
    body: JSON.stringify(body),
  });

describe("the internal seam's file half", () => {
  it("answers a bearer's subject, and null for one that does not verify", async () => {
    expect(
      await (await post("/verify-token", { token: "bearer-ann" })).json(),
    ).toEqual({ sub: "ann@x" });
    expect(
      await (await post("/verify-token", { token: "forged" })).json(),
    ).toEqual({ sub: null });
    expect((await post("/verify-token", {})).status).toBe(400);
    expect(
      (await post("/verify-token", { token: "bearer-ann" }, "wrong")).status,
    ).toBe(401);
  });

  it("says this backend runs no file hub, and no médiathèque to ask about", async () => {
    const hub = await fetch(`${base}/hub`, {
      headers: { [INTERNAL_SECRET_HEADER]: "k" },
    });
    expect(await hub.json()).toEqual({ runs: false });
    const gone = await fetch(`${base}/mediatek`, {
      headers: { [INTERNAL_SECRET_HEADER]: "k" },
    });
    expect(gone.status).toBe(404);
  });
});
