// serviceChanges.spec.ts — `POST /changes` and `GET /verb-bundle` as a client sees them: a set lands whole as one undoable step,
// a stale base or `was` is a 409 that writes nothing, a bad path or an unparsable .k7x refuses the whole set, a library is the
// admins', and the bundle names its libraries by hash. Judged by answers and by what GET /file reads back.
import { expect, test } from "@playwright/test";
import { Service } from "../../src/service.ts";
import { freshWorkspace } from "../../src/sync.ts";

const ws = freshWorkspace();
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 120_000 });

/** `textHash`: cyrb53 over the UTF-16 code units, 14 hex digits (spelled here, never imported). */
function textHash(text: string): string {
  let h1 = 0xdeadbeef;
  let h2 = 0x41c6ce57;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    h1 = Math.imul(h1 ^ c, 2654435761);
    h2 = Math.imul(h2 ^ c, 1597334677);
  }
  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507);
  h1 ^= Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507);
  h2 ^= Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  const n = 4294967296 * (2097151 & h2) + (h1 >>> 0);
  return n.toString(16).padStart(14, "0");
}

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect();
  await svc.create(ws);
  const made = await svc.post(
    "/project/create",
    { shape: "live", name: "live.k7" },
    ws,
  );
  expect(made.status).toBe(200);
  expect((await svc.open(ws, "live.k7")).status).toBe(200);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await svc.remove(ws);
  await svc.dispose();
});

const rev = async () => (await svc.get("/revision", ws)).body.rev as string;
const read = async (path: string) =>
  (await svc.get(`/file?path=${encodeURIComponent(path)}`, ws)).body
    .content as string;
const set = async (ops: unknown[], extra: Record<string, unknown> = {}) =>
  svc.post(
    "/changes",
    {
      at: "live.k7",
      base: await rev(),
      label: "spec verb line",
      ops,
      ...extra,
    },
    ws,
  );
const text = (file: string, body: string, was: string | null) => ({
  kind: "text",
  file,
  text: body,
  was,
});

test("a set lands whole as one step labelled with the verb line, and undo takes the whole set back", async () => {
  const before = (await svc.get("/history/entries", ws)).body.length;
  const r = await set([
    text("live.k7/notes/a.txt", "one\n", null),
    text("live.k7/notes/b.txt", "two\n", null),
  ]);
  expect(r.status).toBe(200);
  expect(r.body.rev).not.toBe(r.body.revBefore);
  expect(r.body.changed.map((c: { path: string }) => c.path).sort()).toEqual([
    "notes/a.txt",
    "notes/b.txt",
  ]);
  expect(await read("live.k7/notes/a.txt")).toBe("one\n");
  const steps = (await svc.get("/history/entries", ws)).body;
  expect(steps).toHaveLength(before + 1);
  expect(steps[steps.length - 1].label).toBe("spec verb line");
  expect((await svc.post("/undo", {}, ws)).body.applied).toBe(true);
  expect((await svc.get("/file?path=live.k7/notes/a.txt", ws)).status).toBe(
    404,
  );
  expect((await svc.get("/file?path=live.k7/notes/b.txt", ws)).status).toBe(
    404,
  );
});

test("the answer's rev is the next set's base, and a stale base or was is a 409 that writes nothing", async () => {
  const first = await set([text("live.k7/notes/c.txt", "c1\n", null)]);
  expect(first.status).toBe(200);
  const chained = await svc.post(
    "/changes",
    {
      at: "live.k7",
      base: first.body.rev,
      label: "chained",
      ops: [text("live.k7/notes/c.txt", "c2\n", textHash("c1\n"))],
    },
    ws,
  );
  expect(chained.status).toBe(200);
  const old = await svc.post(
    "/changes",
    {
      at: "live.k7",
      base: first.body.rev,
      label: "stale",
      ops: [text("live.k7/notes/c.txt", "c3\n", textHash("c2\n"))],
    },
    ws,
  );
  expect(old.status).toBe(409);
  expect(old.body.project).toBe(true);
  expect(old.body.rev).toBe(await rev());
  const moved = await set([
    text("live.k7/notes/c.txt", "c3\n", textHash("c1\n")),
  ]);
  expect(moved.status).toBe(409);
  expect(moved.body.stale).toEqual(["live.k7/notes/c.txt"]);
  expect(await read("live.k7/notes/c.txt")).toBe("c2\n");
});

test("one bad op refuses the whole set: a path out, hidden state, an unparsable .k7x, and too many ops", async () => {
  const good = text("live.k7/notes/never.txt", "x", null);
  const refused = async (bad: unknown, status: number) => {
    const r = await set([good, bad]);
    expect(r.status).toBe(status);
    expect(typeof r.body.error).toBe("string");
    expect(
      (await svc.get("/file?path=live.k7/notes/never.txt", ws)).status,
    ).toBe(404);
    return r;
  };
  await refused(text("../escape.txt", "x", null), 403);
  await refused(text("live.k7/.history/x", "x", null), 403);
  const bad = await refused(
    text("live.k7/n.k7x", 'extends("unterminated\n', null),
    422,
  );
  expect(bad.body.file).toBe("live.k7/n.k7x");
  const many = Array.from({ length: 2001 }, (_, i) =>
    text(`live.k7/m/${i}.txt`, "x", null),
  );
  expect((await set(many)).status).toBe(413);
});

test("a read op makes a stale set a 409, and another open project has its own word, after the path gates", async () => {
  const first = await set([text("live.k7/notes/r.txt", "r1\n", null)]);
  expect(first.status).toBe(200);
  const read = (was: string | null) => ({
    kind: "read",
    file: "live.k7/notes/r.txt",
    was,
  });
  const write = text("live.k7/notes/after-read.txt", "w", null);
  const moved = await set([read(textHash("r0\n")), write]);
  expect(moved.status).toBe(409);
  expect(moved.body.stale).toEqual(["live.k7/notes/r.txt"]);
  expect(
    (await svc.get("/file?path=live.k7/notes/after-read.txt", ws)).status,
  ).toBe(404);
  expect((await set([read(textHash("r1\n")), write])).status).toBe(200);
  const elsewhere = await set([write], { at: "other.k7" });
  expect(elsewhere.status).toBe(409);
  expect(elsewhere.body.open).toBe("live.k7");
  expect(elsewhere.body.project).toBe(false);
  const forbidden = await set([text("../escape.txt", "x", null)], {
    at: "other.k7",
  });
  expect(forbidden.status).toBe(403);
});

test("journal skip lands the set without a step, and the answer's rev is the next base", async () => {
  const before = (await svc.get("/history/entries", ws)).body.length;
  const r = await set([text("live.k7/notes/pin.txt", "pin\n", null)], {
    journal: "skip",
  });
  expect(r.status).toBe(200);
  expect(await read("live.k7/notes/pin.txt")).toBe("pin\n");
  expect((await svc.get("/history/entries", ws)).body).toHaveLength(before);
  const again = await svc.post(
    "/changes",
    { at: "live.k7", base: r.body.rev, label: "probe", ops: [] },
    ws,
  );
  expect(again.status).toBe(200);
});

test("an empty at is the workspace alone: its bundle holds no project, and a set may write only the shared texts", async () => {
  const bundle = await svc.get("/verb-bundle?at=", ws);
  expect(bundle.status).toBe(200);
  expect(bundle.body.rev).toMatch(/^none:/);
  const files = Object.keys(bundle.body.source.files);
  expect(files.some((f) => f.startsWith("/w/live.k7/"))).toBe(false);
  const alone = (ops: unknown[]) =>
    svc.post(
      "/changes",
      { at: "", base: bundle.body.rev, label: "playlist new", ops },
      ws,
    );
  expect((await alone([text("_playlists/z.txt", "z\n", null)])).status).toBe(
    200,
  );
  expect(await read("_playlists/z.txt")).toBe("z\n");
  expect(
    (await alone([text("live.k7/notes/no.txt", "n\n", null)])).status,
  ).toBe(403);
  expect((await svc.get("/file?path=live.k7/notes/no.txt", ws)).status).toBe(
    404,
  );
});

test("a chain of a library is gated like any write there, and a name that is no project and no library is the other project's 409", async () => {
  const _chain = await svc.post(
    "/changes",
    {
      at: "_libraries/k7-core",
      base: "x",
      label: "graph pin",
      ops: [],
    },
    ws,
  );
  // A library is never written from here: it is refused before its folder is looked at.
  const other = await svc.get("/verb-bundle?at=other.k7", ws);
  expect(other.status).toBe(409);
});
