// serviceListings.spec.ts — the workspace's playlists and references, listed: empty, then after each file made, renamed
// and removed. Every answer is asserted whole, so a listing written in Rust and the TypeScript's own are held to one body.
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { putTree, REPO } from "../../src/projectTree.ts";
import { Service } from "../../src/service.ts";
import { freshWorkspace } from "../../src/sync.ts";

const ws = freshWorkspace();
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect();
  await svc.create(ws);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await svc.remove(ws);
  await svc.dispose();
});

const playlists = async () => {
  const r = await svc.get("/playlists", ws);
  expect(r.status).toBe(200);
  return r.body;
};
const references = async () => {
  const r = await svc.get("/references", ws);
  expect(r.status).toBe(200);
  return r.body;
};
const write = async (path: string, content: string) =>
  expect((await svc.put("/file", { path, content }, ws)).status).toBe(200);
const upload = async (dir: string, name: string, bytes: Buffer) =>
  expect((await svc.upload(ws, dir, name, bytes)).status).toBe(200);

test("a workspace with neither folder lists no playlist and no reference", async () => {
  expect(await playlists()).toEqual({ playlists: [] });
  expect(await references()).toEqual({ references: [] });
});

test("playlists are listed at any depth, by path, with a title trimmed, a count of the lines that name a song and ok", async () => {
  await write(
    "_playlists/a.k7x",
    'title("  Padded  ")\nitems(["s/one", "s/two"])\n',
  );
  expect(await playlists()).toEqual({
    playlists: [
      {
        path: "_playlists/a.k7x",
        name: "a",
        title: "Padded",
        count: 2,
        ok: true,
      },
    ],
  });
  await write(
    "_playlists/gig/b.k7x",
    'items(["x", {song: "y", variant: "live"}, "   ", {variant: "z"}])\n',
  );
  await write("_playlists/gig/deep/c.k7x", "items([])\n");
  expect(await playlists()).toEqual({
    playlists: [
      {
        path: "_playlists/a.k7x",
        name: "a",
        title: "Padded",
        count: 2,
        ok: true,
      },
      { path: "_playlists/gig/b.k7x", name: "b", count: 2, ok: true },
      { path: "_playlists/gig/deep/c.k7x", name: "c", count: 0, ok: true },
    ],
  });
});

test("a playlist that does not parse is listed as not ok, one named in capitals is a playlist, a .json beside them is not", async () => {
  await write("_playlists/bad.k7x", "items([\n");
  await write("_playlists/UP.K7X", 'title("Shout")\nitems(["q"])\n');
  await write("_playlists/notes.json", '{"items": ["no"]}\n');
  expect(await playlists()).toEqual({
    playlists: [
      {
        path: "_playlists/UP.K7X",
        name: "UP",
        title: "Shout",
        count: 1,
        ok: true,
      },
      {
        path: "_playlists/a.k7x",
        name: "a",
        title: "Padded",
        count: 2,
        ok: true,
      },
      { path: "_playlists/bad.k7x", name: "bad", count: 0, ok: false },
      { path: "_playlists/gig/b.k7x", name: "b", count: 2, ok: true },
      { path: "_playlists/gig/deep/c.k7x", name: "c", count: 0, ok: true },
    ],
  });
});

test("a playlist renamed, rewritten and recycled changes the listing at once", async () => {
  const moved = await svc.post(
    "/rename",
    { path: "_playlists/gig/b.k7x", name: "moved.k7x" },
    ws,
  );
  expect(moved.status).toBe(200);
  await write(
    "_playlists/a.k7x",
    'title("Second")\nitems(["one", "two", "three"])\n',
  );
  const gone = await svc.post(
    "/entry/delete",
    { path: "_playlists/bad.k7x" },
    ws,
  );
  expect(gone.status).toBe(200);
  expect(await playlists()).toEqual({
    playlists: [
      {
        path: "_playlists/UP.K7X",
        name: "UP",
        title: "Shout",
        count: 1,
        ok: true,
      },
      {
        path: "_playlists/a.k7x",
        name: "a",
        title: "Second",
        count: 3,
        ok: true,
      },
      { path: "_playlists/gig/deep/c.k7x", name: "c", count: 0, ok: true },
      { path: "_playlists/gig/moved.k7x", name: "moved", count: 2, ok: true },
    ],
  });
});

test("references are listed at any depth by their extension in any case, with the bytes they hold", async () => {
  await upload("_references", "one.wav", Buffer.alloc(100, 1));
  expect(await references()).toEqual({
    references: [{ path: "_references/one.wav", name: "one", bytes: 100 }],
  });
  await upload("_references/sets", "Loud.WAV", Buffer.alloc(2048, 2));
  await upload("_references/sets/live", "take.mp3", Buffer.alloc(7, 3));
  await write("_references/sets/notes.txt", "skipped");
  await write("_references/sets/noext", "skipped");
  expect(await references()).toEqual({
    references: [
      { path: "_references/one.wav", name: "one", bytes: 100 },
      { path: "_references/sets/Loud.WAV", name: "Loud", bytes: 2048 },
      { path: "_references/sets/live/take.mp3", name: "take", bytes: 7 },
    ],
  });
});

test("a reference removed or replaced is listed as it now is, and the project fixtures list as the editor's fixture does", async () => {
  expect(
    (await svc.post("/entry/delete", { path: "_references/one.wav" }, ws))
      .status,
  ).toBe(200);
  await upload("_references/sets", "flac.FLAC", Buffer.alloc(33, 4));
  expect(await references()).toEqual({
    references: [
      { path: "_references/sets/Loud.WAV", name: "Loud", bytes: 2048 },
      { path: "_references/sets/flac.FLAC", name: "flac", bytes: 33 },
      { path: "_references/sets/live/take.mp3", name: "take", bytes: 7 },
    ],
  });
  await putTree(svc, ws, join(REPO, "e2e/fixtures/_playlists"), "_playlists");
  const list = (await playlists()).playlists as { path: string }[];
  expect(list.map((p) => p.path)).toContain("_playlists/e2e.k7x");
  expect(list.find((p) => p.path === "_playlists/e2e.k7x")).toEqual({
    path: "_playlists/e2e.k7x",
    name: "e2e",
    title: "E2E playlist",
    count: 1,
    ok: true,
  });
});
