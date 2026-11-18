// seam.test.ts — the internal seam's TypeScript half over real HTTP: the gate the backend mounts in front of its
// own `/api/internal/files` refuses a peer the front vouched for as remote and a request without the secret, and
// the client finds the front's port. The engine half is the Rust front's (seamCore.test.ts).

import { createServer, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import { FRONT_PEER } from "@studio/web-protocol/services";
import express from "express";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  INTERNAL_FILES_PATH,
  INTERNAL_SECRET_HEADER,
  internalGate,
} from "./seam.ts";
import { seamConfigFromEnv } from "./seamClient.ts";

const SECRET = "s3cret";

describe("the internal seam's gate", () => {
  let server: Server;
  let base: string;
  /** The peer the front vouches for on the next requests, as front.ts records it. */
  let vouched: string | undefined;

  beforeEach(async () => {
    vouched = undefined;
    const app = express();
    app.use((req, _res, next) => {
      if (vouched) (req as { [FRONT_PEER]?: string })[FRONT_PEER] = vouched;
      next();
    });
    app.use(INTERNAL_FILES_PATH, internalGate(SECRET), (_req, res) => {
      res.json({ ok: true });
    });
    server = createServer(app);
    await new Promise<void>((r) => server.listen(0, "127.0.0.1", () => r()));
    base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  });

  afterEach(async () => {
    await new Promise((r) => server.close(r));
  });

  const get = (headers: Record<string, string>) =>
    fetch(`${base}${INTERNAL_FILES_PATH}/anything`, { headers });

  it("refuses a request without the secret, or with another, by 401", async () => {
    expect((await get({})).status).toBe(401);
    expect((await get({ [INTERNAL_SECRET_HEADER]: "nope" })).status).toBe(401);
    expect((await get({ [INTERNAL_SECRET_HEADER]: `${SECRET}x` })).status).toBe(
      401,
    );
    const ok = await get({ [INTERNAL_SECRET_HEADER]: SECRET });
    expect(ok.status).toBe(200);
    expect(ok.headers.get("cache-control")).toBe("no-store");
  });

  it("refuses a peer that is not this machine by 403, even with the secret", async () => {
    vouched = "203.0.113.7";
    expect((await get({ [INTERNAL_SECRET_HEADER]: SECRET })).status).toBe(403);
    // A request that named the front without proving it fails closed too.
    vouched = "front-unverified";
    expect((await get({ [INTERNAL_SECRET_HEADER]: SECRET })).status).toBe(403);
    vouched = "127.0.0.1";
    expect((await get({ [INTERNAL_SECRET_HEADER]: SECRET })).status).toBe(200);
  });
});

describe("where the seam is reached", () => {
  it("is the front's port with its secret when a front runs, this backend's own with a drawn secret otherwise", () => {
    expect(
      seamConfigFromEnv(8790, { K7_FRONT_SECRET: "f", K7_FRONT_PORT: "8789" }),
    ).toEqual({ base: "http://127.0.0.1:8789", secret: "f" });
    const alone = seamConfigFromEnv(8790, {});
    expect(alone.base).toBe("http://127.0.0.1:8790");
    expect(alone.secret).toMatch(/^[0-9a-f]{64}$/);
    expect(seamConfigFromEnv(8790, {}).secret).not.toBe(alone.secret);
    // A front with no port named leaves the backend calling itself, with the front's secret.
    expect(seamConfigFromEnv(8790, { K7_FRONT_SECRET: "f" })).toEqual({
      base: "http://127.0.0.1:8790",
      secret: "f",
    });
  });
});
