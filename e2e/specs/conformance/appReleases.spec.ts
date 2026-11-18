// appReleases.spec.ts — the apps' downloads, whichever language serves them: the CI's bearer, a publish, the feed,
// the page, ranges and validators; only the front serves them (e2e/docs/conformance.md).
// Uploads need the instance's token and store folder (CONFORMANCE_RELEASE_TOKEN, CONFORMANCE_RELEASES_DIR).
import { randomBytes } from "node:crypto";
import { readdirSync, readFileSync, rmdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { type APIRequestContext, expect, test } from "@playwright/test";
import { deviceBed } from "../../src/session.ts";

const base = process.env.E2E_BASE_URL ?? "http://127.0.0.1:8789";
const token = process.env.CONFORMANCE_RELEASE_TOKEN;
const storeDir = process.env.CONFORMANCE_RELEASES_DIR;
const maxMb = Number(process.env.CONFORMANCE_RELEASE_MAX_MB) || 0;

test.describe.configure({ mode: "serial" });

// The tray's snapshot channel, which a dev instance never serves: the spec refuses to replace a build a store holds.
const CHANNEL_DIR = storeDir ? join(storeDir, "tray", "snapshot") : "";
const created: string[] = [];
const auth = { authorization: `Bearer ${token}` };
const exe = Buffer.from(Array.from({ length: 4096 }, (_, i) => i % 251));

const newStamp = () => {
  const stamp = `snapshot-${randomBytes(6).toString("hex")}`;
  created.push(stamp);
  return stamp;
};
const exeName = (stamp: string) => `k7_${stamp}_windows-x64.exe`;
const at = (root: string, path: string) => new URL(path, root).href;

/** Uploads a build's two files into the tray's store at `root`, then publishes it. */
async function publish(
  request: APIRequestContext,
  root: string,
  stamp: string,
) {
  const put = (file: string, data: Buffer) =>
    request.put(at(root, `/api/app-releases/tray/${stamp}/${file}`), {
      headers: { ...auth, "content-type": "application/octet-stream" },
      data,
    });
  const saved = await put(exeName(stamp), exe);
  expect(saved.status()).toBe(201);
  expect(await saved.json()).toEqual({ name: exeName(stamp), bytes: 4096 });
  expect(
    (await put("latest.json", Buffer.from(`{"v":"${stamp}"}`))).status(),
  ).toBe(201);
  const done = await request.post(
    at(root, `/api/app-releases/tray/${stamp}/publish`),
    { headers: auth },
  );
  expect(done.status()).toBe(200);
  return done.json();
}

/** The build `root` feeds on the tray's snapshot channel, or null. */
async function fed(request: APIRequestContext, root: string) {
  const res = await request.get(at(root, "/app_downloads/tray/snapshot.json"));
  return res.status() === 404 ? null : res.json();
}

test.afterAll(() => {
  if (!CHANNEL_DIR) return;
  // By name: the builds this run published, and a pointer only when it names one of them.
  for (const stamp of created) {
    rmSync(join(CHANNEL_DIR, stamp), { recursive: true, force: true });
    rmSync(join(CHANNEL_DIR, "incoming", stamp), {
      recursive: true,
      force: true,
    });
  }
  for (const pointer of ["current", "previous"]) {
    const file = join(CHANNEL_DIR, pointer);
    try {
      if (created.includes(readFileSync(file, "utf8").trim())) rmSync(file);
    } catch {}
  }
  for (const dir of [join(CHANNEL_DIR, "incoming"), CHANNEL_DIR]) {
    try {
      if (readdirSync(dir).length === 0) rmdirSync(dir);
    } catch {}
  }
});

test("an upload without the CI's bearer is refused", async ({ request }) => {
  test.skip(
    deviceBed(),
    "a device answers no app release: app_releases.rs holds the refusals",
  );
  const path = "/api/app-releases/v0.0.1/conformance.txt";
  const none = await request.put(path, { data: Buffer.from("x") });
  expect(none.status()).toBe(401);
  expect(await none.json()).toEqual({ error: "unauthorized" });
  const wrong = await request.put(path, {
    headers: { authorization: "Bearer not-the-token" },
    data: Buffer.from("x"),
  });
  expect(wrong.status()).toBe(401);
  const publishing = await request.post("/api/app-releases/v0.0.1/publish");
  expect(publishing.status()).toBe(401);
});

test("a feed with nothing published, and an unknown file, are 404", async ({
  request,
}) => {
  test.skip(
    deviceBed(),
    "a device answers no app release: app_releases.rs holds the feed",
  );
  const feed = await request.get("/app_downloads/nochannel.json");
  expect(feed.status()).toBe(404);
  expect(feed.headers()["cache-control"]).toBe("no-cache");
  expect(await feed.json()).toEqual({ error: "not found" });
  const file = await request.get("/app_downloads/release/conformance-none.exe");
  expect(file.status()).toBe(404);
  expect(await file.text()).toBe("not found");
  const page = await request.get("/app_downloads");
  expect(page.status()).toBe(200);
  expect(page.headers()["content-type"]).toBe("text/html; charset=utf-8");
});

test.describe("with the CI's token", () => {
  test.skip(
    !token || !storeDir,
    "CONFORMANCE_RELEASE_TOKEN and CONFORMANCE_RELEASES_DIR name the instance's",
  );

  test.beforeAll(async ({ playwright }) => {
    const request = await playwright.request.newContext();
    const live = await fed(request, base);
    await request.dispose();
    test.skip(
      live !== null && !created.includes(live.stamp),
      "the tray's snapshot channel already serves a build",
    );
  });

  test("an invalid stamp or file is refused", async ({ request }) => {
    const bad = await request.put("/api/app-releases/v1.2/a.exe", {
      headers: auth,
      data: Buffer.from("x"),
    });
    expect(bad.status()).toBe(400);
    expect(await bad.json()).toEqual({ error: "invalid stamp or file" });
    const nothing = await request.post(
      `/api/app-releases/tray/${newStamp()}/publish`,
      { headers: auth },
    );
    expect(nothing.status()).toBe(404);
  });

  test("a published build is fed, paged, served by ranges and revalidated", async ({
    request,
  }) => {
    const stamp = newStamp();
    const release = await publish(request, base, stamp);
    expect(release).toMatchObject({ channel: "snapshot", stamp });
    expect(release.files).toEqual([
      { name: exeName(stamp), bytes: 4096 },
      { name: "latest.json", bytes: Buffer.byteLength(`{"v":"${stamp}"}`) },
    ]);
    expect(Object.keys(release)).toEqual([
      "channel",
      "stamp",
      "publishedAt",
      "files",
    ]);
    expect(await fed(request, base)).toEqual(release);

    const url = `/app_downloads/tray/snapshot/${exeName(stamp)}`;
    const whole = await request.get(url);
    expect(whole.status()).toBe(200);
    expect(Buffer.compare(await whole.body(), exe)).toBe(0);
    expect(whole.headers()["content-disposition"]).toBe("attachment");
    expect(whole.headers()["accept-ranges"]).toBe("bytes");
    const etag = whole.headers().etag;
    expect(etag).toMatch(/^W\/"1000-[0-9a-f]+"$/);

    const part = await request.get(url, { headers: { range: "bytes=10-19" } });
    expect(part.status()).toBe(206);
    expect(part.headers()["content-range"]).toBe("bytes 10-19/4096");
    expect(Buffer.compare(await part.body(), exe.subarray(10, 20))).toBe(0);
    const tail = await request.get(url, { headers: { range: "bytes=-5" } });
    expect(Buffer.compare(await tail.body(), exe.subarray(4091))).toBe(0);
    const past = await request.get(url, { headers: { range: "bytes=5000-" } });
    expect(past.status()).toBe(416);
    expect(past.headers()["content-range"]).toBe("bytes */4096");
    const held = await request.get(url, { headers: { "if-none-match": etag } });
    expect(held.status()).toBe(304);
    const stale = await request.get(url, {
      headers: { range: "bytes=0-1", "if-range": '"stale"' },
    });
    expect(stale.status()).toBe(200);

    const json = await request.get("/app_downloads/tray/snapshot/latest.json");
    expect(json.headers()["content-type"]).toBe("text/plain");
    const page = await (
      await request.get("/app_downloads", {
        headers: { "accept-language": "fr" },
      })
    ).text();
    expect(page).toContain(
      `<a href="app_downloads/tray/snapshot/${exeName(stamp)}">Windows</a>`,
    );
    expect(page).toContain("Tray — Dernière build de main");
  });

  test("an upload past the ceiling is refused", async ({ request }) => {
    test.skip(
      !maxMb,
      "CONFORMANCE_RELEASE_MAX_MB names the instance's APP_RELEASE_MAX_MB",
    );
    const stamp = newStamp();
    const res = await request.put(`/api/app-releases/tray/${stamp}/big.zip`, {
      headers: auth,
      data: Buffer.alloc(maxMb * 1024 * 1024 + 1),
    });
    expect(res.status()).toBe(413);
    expect(await res.json()).toEqual({
      error: `file exceeds ${maxMb * 1024 * 1024} bytes`,
    });
  });
});
