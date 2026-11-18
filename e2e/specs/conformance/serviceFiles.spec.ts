// serviceFiles.spec.ts — one file at a time through the file service: a read, a write and its stale base, the paths
// it refuses, a dot folder, a folder made, an entry renamed, moved and recycled, an upload's refusals and `/media`.
// Every write is read back from disk under CONFORMANCE_FILES_ROOT; the workspace is the spec's own.
import { existsSync, readFileSync } from "node:fs";
import { request as httpRequest } from "node:http";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { base } from "../../src/instance.ts";
import {
  filesUnder,
  onDisk,
  SERVICE_USER,
  Service,
  spelledOnWindows,
  wsDir,
} from "../../src/service.ts";
import { acct, deviceBed } from "../../src/session.ts";
import { filesRoot, freshWorkspace, textHash } from "../../src/sync.ts";

const ws = freshWorkspace();
let svc: Service;

test.describe.configure({ mode: "serial", timeout: 90_000 });

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

const write = (
  path: string,
  content: string,
  extra: Record<string, unknown> = {},
) => svc.put("/file", { path, content, ...extra }, ws);

test("a file written is read back as text, byte for byte, and lands where it was named", async () => {
  const text = "line one\nlíne twö — ✓\n";
  const put = await write("notes/a.txt", text);
  expect([put.status, put.body]).toEqual([200, { ok: true }]);
  const read = await svc.get("/file?path=notes/a.txt", ws);
  expect([read.status, read.body]).toEqual([
    200,
    { path: "notes/a.txt", binary: false, content: text },
  ]);
  // Read back after a save: the same answer, nothing formatted.
  expect((await svc.post("/fmt", { path: "notes/a.txt" }, ws)).body).toEqual(
    read.body,
  );
  if (filesRoot) expect(onDisk(ws, "notes/a.txt")).toBe(text);
  const none = await svc.get("/file", ws);
  expect([none.status, none.body]).toEqual([400, { error: "path required" }]);
  const missing = await svc.get("/file?path=notes/none.txt", ws);
  expect(missing.status).toBe(404);
});

test("a write naming the base it was edited from is refused once the file moved on, and leaves it alone", async () => {
  const now = "line one\n";
  await write("notes/b.txt", now);
  const stale = await write("notes/b.txt", "theirs\n", {
    base: textHash("an older text\n"),
  });
  expect([stale.status, stale.body]).toEqual([
    409,
    {
      error:
        "notes/b.txt changed on disk since it was opened — not overwritten",
      stale: true,
    },
  ]);
  if (filesRoot) expect(onDisk(ws, "notes/b.txt")).toBe(now);
  const fresh = await write("notes/b.txt", "mine\n", { base: textHash(now) });
  expect([fresh.status, fresh.body]).toEqual([200, { ok: true }]);
  // A file not there yet has nothing to overwrite, whatever base is named.
  const created = await write("notes/new.txt", "new\n", {
    base: textHash("x"),
  });
  expect(created.status).toBe(200);
  if (filesRoot) expect(onDisk(ws, "notes/b.txt")).toBe("mine\n");
});

test("a path leaving the workspace is refused for every verb, and writes nothing", async () => {
  // A drive is an absolute path on a Windows server only: its root says which, never the runner.
  const drive = spelledOnWindows(await svc.root(ws))
    ? ["C:/Windows/win.ini"]
    : [];
  for (const path of [
    "../x.txt",
    "notes/../../x.txt",
    "/etc/passwd",
    ...drive,
  ]) {
    const error = `path escapes the workspace: ${path}`;
    const read = await svc.get(`/file?path=${encodeURIComponent(path)}`, ws);
    expect([path, read.status, read.body]).toEqual([path, 400, { error }]);
    const put = await write(path, "x");
    expect([path, put.status, put.body]).toEqual([path, 400, { error }]);
  }
  const moved = await svc.post(
    "/move",
    { path: "notes/a.txt", toDir: "../" },
    ws,
  );
  expect([moved.status, moved.body]).toEqual([
    400,
    { error: "path escapes the workspace: ../" },
  ]);
  const root = await svc.post("/entry/delete", { path: "." }, ws);
  expect([root.status, root.body]).toEqual([
    400,
    { error: "can't delete the workspace root" },
  ]);
  if (filesRoot) expect(existsSync(join(wsDir(ws), "..", "x.txt"))).toBe(false);
});

test("a dot folder is written and read like any other, and no tree lists it", async () => {
  expect((await write(".hidden/x.txt", "x\n")).status).toBe(200);
  expect((await svc.get("/file?path=.hidden/x.txt", ws)).body.content).toBe(
    "x\n",
  );
  const tree = JSON.stringify((await svc.get("/tree", ws)).body);
  expect(tree).not.toContain(".hidden");
  expect(tree).toContain('"path":"notes/a.txt"');
  if (filesRoot) expect(onDisk(ws, ".hidden/x.txt")).toBe("x\n");
});

test("a folder is made once, a name with a separator keeps its last part, and the recycle bin takes none", async () => {
  const made = await svc.post("/folder", { dir: ".", name: "sub" }, ws);
  expect([made.status, made.body]).toEqual([200, { path: "sub" }]);
  const twice = await svc.post("/folder", { dir: ".", name: "sub" }, ws);
  expect([twice.status, twice.body]).toEqual([
    400,
    { error: "already exists: sub" },
  ]);
  const nested = await svc.post("/folder", { dir: "sub", name: "a/inner" }, ws);
  expect([nested.status, nested.body]).toEqual([200, { path: "sub/inner" }]);
  const bin = await svc.post("/folder", { dir: "_recycle_bin", name: "x" }, ws);
  expect([bin.status, bin.body]).toEqual([
    400,
    { error: "can't create a folder in the recycle bin" },
  ]);
  if (filesRoot) expect(existsSync(join(wsDir(ws), "sub", "inner"))).toBe(true);
});

test("an entry is renamed in its folder, never over another", async () => {
  const r = await svc.post(
    "/rename",
    { path: "notes/new.txt", name: "renamed.txt" },
    ws,
  );
  expect([r.status, r.body]).toEqual([200, { path: "notes/renamed.txt" }]);
  const clobber = await svc.post(
    "/rename",
    { path: "notes/renamed.txt", name: "a.txt" },
    ws,
  );
  expect([clobber.status, clobber.body]).toEqual([
    400,
    { error: "already exists: notes/a.txt" },
  ]);
  // A name with a separator keeps its last part: a rename never leaves the folder.
  const up = await svc.post(
    "/rename",
    { path: "notes/renamed.txt", name: "../up.txt" },
    ws,
  );
  expect([up.status, up.body]).toEqual([200, { path: "notes/up.txt" }]);
  if (filesRoot) {
    expect(onDisk(ws, "notes/up.txt")).toBe("new\n");
    expect(existsSync(join(wsDir(ws), "notes", "new.txt"))).toBe(false);
  }
});

test("an entry moves into another folder, never into itself or a folder that is not there", async () => {
  const moved = await svc.post(
    "/move",
    { path: "notes/up.txt", toDir: "sub" },
    ws,
  );
  expect([moved.status, moved.body]).toEqual([200, { path: "sub/up.txt" }]);
  const nowhere = await svc.post(
    "/move",
    { path: "sub/up.txt", toDir: "nowhere" },
    ws,
  );
  expect([nowhere.status, nowhere.body]).toEqual([
    400,
    { error: "not a folder: nowhere" },
  ]);
  const self = await svc.post("/move", { path: "sub", toDir: "sub/inner" }, ws);
  expect(self.status).toBe(400);
  if (filesRoot) expect(onDisk(ws, "sub/up.txt")).toBe("new\n");
});

test("a delete recycles the entry, keeps both of two of one name, and refuses what is already recycled", async () => {
  const first = await svc.post("/entry/delete", { path: "sub/up.txt" }, ws);
  expect([first.status, first.body]).toEqual([
    200,
    { recycled: "_recycle_bin/up.txt" },
  ]);
  const into = await svc.post(
    "/move",
    { path: "notes/a.txt", toDir: "_recycle_bin" },
    ws,
  );
  expect([into.status, into.body]).toEqual([
    400,
    { error: "can't move something into the recycle bin — delete it instead" },
  ]);
  await write("sub/up.txt", "second\n");
  const second = await svc.post("/entry/delete", { path: "sub/up.txt" }, ws);
  // The second of one name is numbered after its whole name, extension included.
  expect([second.status, second.body]).toEqual([
    200,
    { recycled: "_recycle_bin/up.txt-2" },
  ]);
  const again = await svc.post(
    "/entry/delete",
    { path: "_recycle_bin/up.txt" },
    ws,
  );
  expect([again.status, again.body]).toEqual([
    400,
    { error: "'_recycle_bin/up.txt' is already in the recycle bin" },
  ]);
  if (filesRoot) {
    expect(onDisk(ws, "_recycle_bin/up.txt")).toBe("new\n");
    expect(onDisk(ws, second.body.recycled)).toBe("second\n");
    expect(existsSync(join(wsDir(ws), "sub", "up.txt"))).toBe(false);
  }
});

test("an entry that is not there is a 404 naming no folder of the server", async () => {
  const root = await svc.root(ws);
  for (const [path, body] of [
    ["/rename", { path: "none.txt", name: "z.txt" }],
    ["/move", { path: "none.txt", toDir: "sub" }],
    ["/entry/delete", { path: "none.txt" }],
    ["/fmt", { path: "none.txt" }],
  ] as const) {
    const r = await svc.post(path, body, ws);
    expect([path, r.status, r.body]).toEqual([
      path,
      404,
      { error: "no such file or folder: none.txt" },
    ]);
    expect(JSON.stringify(r.body)).not.toContain(
      JSON.stringify(root).slice(1, -1),
    );
  }
});

test("/fmt of a folder is the caller's mistake, never the service's", async () => {
  const r = await svc.post("/fmt", { path: "sub" }, ws);
  expect([r.status, r.body]).toEqual([
    400,
    { error: "'sub' is a folder, not a file" },
  ]);
});

/** An upload's answer to a declared length, sent before any byte: the refusal must come without the body. */
function declared(length: number): Promise<{ status: number; body: string }> {
  const url = new URL(`/api/files/upload?dir=.&name=big.bin`, base);
  return new Promise((resolve, reject) => {
    const req = httpRequest(url, {
      method: "POST",
      headers: {
        ...svc.headers(ws),
        "content-type": "application/octet-stream",
        "content-length": String(length),
      },
    });
    req.on("response", (res) => {
      let body = "";
      res.setEncoding("utf8");
      res.on("data", (d) => {
        body += d;
      });
      res.on("end", () => {
        req.destroy();
        resolve({ status: res.statusCode ?? 0, body });
      });
    });
    req.on("error", reject);
    req.flushHeaders();
  });
}

test("an upload declaring a length past the ceiling is a 413 in words, before any byte is sent", async () => {
  const big = await declared(513 * 1024 * 1024);
  expect(big.status).toBe(413);
  expect(JSON.parse(big.body).error).toMatch(/512 MB/);
});

test("an upload is refused before its bytes: no name, JSON, a path out", async () => {
  const send = (query: string, type = "application/octet-stream") =>
    svc.call("POST", `/upload?${query}`, {
      ws,
      data: Buffer.from("x"),
      headers: { "content-type": type },
    });
  const noName = await send("dir=.");
  expect([noName.status, noName.body]).toEqual([
    400,
    { error: "name required" },
  ]);
  const json = await svc.call("POST", "/upload?dir=.&name=j.bin", {
    ws,
    data: { a: 1 },
  });
  expect([json.status, json.body]).toEqual([
    415,
    { error: "send the bytes as application/octet-stream" },
  ]);
  const out = await send("dir=..&name=e.bin");
  expect([out.status, out.body]).toEqual([
    400,
    { error: "path escapes the workspace: ../e.bin" },
  ]);
  // A folder that is not there yet is made, and a name may carry one.
  const deep = await send("dir=media&name=take%2Fone.bin");
  expect([deep.status, deep.body]).toEqual([
    200,
    { path: "media/take/one.bin" },
  ]);
  if (filesRoot) {
    expect(readFileSync(join(wsDir(ws), "media", "take", "one.bin"))).toEqual(
      Buffer.from("x"),
    );
    expect(
      filesUnder(wsDir(ws)).filter(
        (f) => f.includes("big.bin") || f.includes("j.bin"),
      ),
    ).toEqual([]);
  }
});

test("/media takes its identity in the query, answers the bytes, and refuses what it cannot serve", async () => {
  const media = (q: Record<string, string>) =>
    `/api/files/media?${new URLSearchParams(q)}`;
  const ok = {
    user: acct(SERVICE_USER).email as string,
    workspace: ws,
    token: svc.ticket,
  };
  const read = await svc.request.get(media({ ...ok, path: "notes/a.txt" }));
  expect([read.status(), (await read.body()).toString("utf8")]).toEqual([
    200,
    "line one\nlíne twö — ✓\n",
  ]);
  const noTicket = await svc.request.get(
    media({ user: ok.user, workspace: ws, path: "notes/a.txt" }),
  );
  // A device answers every request as its one local user, ticket or not.
  expect(noTicket.status()).toBe(deviceBed() ? 200 : 401);
  for (const [q, status, error] of [
    [{ token: svc.ticket }, 400, "user, workspace and path required"],
    [{ ...ok, path: "../x" }, 400, "path escapes the workspace: ../x"],
    [
      { ...ok, workspace: `${ws}-none`, path: "x" },
      404,
      `unknown workspace: ${ws}-none`,
    ],
  ] as const) {
    const r = await svc.request.get(media(q));
    expect([r.status(), await r.json()]).toEqual([status, { error }]);
  }
  const missing = await svc.request.get(media({ ...ok, path: "none.wav" }));
  expect(missing.status()).toBe(404);
});

test("a lane saved with a start in seconds raises its project's format, and a musical one does not", async () => {
  await write("fmt.k7/project.k7x", 'project({ name: "fmt" })\n');
  const root = () => svc.get("/file?path=fmt.k7/project.k7x", ws);
  const musical =
    'extends("@meta/track")\nevents([ref("hit").start("2:1:1")])\n';
  await write("fmt.k7/nodes/fx.k7x", musical);
  expect((await root()).body.content).not.toContain("format");
  const seconds = 'extends("@meta/track")\nevents([ref("hit").start(4.5)])\n';
  const put = await write("fmt.k7/nodes/fx.k7x", seconds);
  expect([put.status, put.body]).toEqual([200, { ok: true }]);
  expect((await root()).body.content).toContain("format(2)");
});
