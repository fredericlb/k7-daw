// ready.spec.ts — `/api/ready`, what the image's healthcheck asks: answered to this machine only, by its peer and by
// the Host it names; each volume probed with a write it removes, and the file hub's starts. Raw requests, since a
// fetch will not send a Host of its choosing. The volumes are CONFORMANCE_FILES_ROOT's when it is given.
import { readdirSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { base } from "../../src/instance.ts";
import { filesRoot, NO_FILES_ROOT } from "../../src/sync.ts";
import { rawRequest } from "../../src/wire.ts";

const host = new URL(base).host;

test("a machine asking itself is answered: every volume ok, no problem, and how the file hub is doing", async () => {
  const res = await rawRequest("/api/ready", { host });
  expect(res.status).toBe(200);
  const body = JSON.parse(res.body);
  expect(Object.keys(body)).toEqual(["ok", "problems", "volumes", "files"]);
  expect(body).toMatchObject({ ok: true, problems: [] });
  expect(Object.values(body.volumes).every((v) => v === "ok")).toBe(true);
  expect(Object.keys(body.files)).toEqual([
    "running",
    "starting",
    "cap",
    "oldestStartMs",
    "lastStartError",
  ]);
  expect(body.files.running).toBeGreaterThanOrEqual(0);
  expect(body.files.starting).toBeGreaterThanOrEqual(0);
  expect(body.files.cap).toBeGreaterThanOrEqual(1);
  expect(body.files.cap).toBeLessThanOrEqual(50);
  expect(body.files.oldestStartMs).toBeGreaterThanOrEqual(0);
  // A failed start is said with when it failed, as the TypeScript backend's hub answered it: `{ at, message }`.
  if (body.files.lastStartError !== null)
    expect(body.files.lastStartError).toEqual({
      at: expect.any(Number),
      message: expect.any(String),
    });
});

test("a request naming another host is refused, whatever its peer", async () => {
  for (const name of ["example.com", "k7.example:443", "127.0.0.1.nip.io"]) {
    const res = await rawRequest("/api/ready", { host: name });
    expect([name, res.status, res.body]).toEqual([
      name,
      403,
      "This server answers its own machine only.",
    ]);
  }
});

test.describe("on disk", () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  test("the files root is one of the volumes probed, and the probe leaves nothing behind", async () => {
    const body = JSON.parse((await rawRequest("/api/ready", { host })).body);
    const roots = Object.keys(body.volumes).map((v) =>
      v.replace(/\\/g, "/").toLowerCase(),
    );
    const mine = (filesRoot as string).replace(/\\/g, "/").toLowerCase();
    // Read from WSL the same root is /mnt/<drive>/…: compared by its tail.
    expect(
      roots.some((r) => r === mine || mine.endsWith(r.replace(/^[a-z]:/, ""))),
    ).toBe(true);
    expect(
      readdirSync(filesRoot as string).filter((n) => n.startsWith(".ready-")),
    ).toEqual([]);
  });
});
