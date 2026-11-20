// serviceHistory.spec.ts — the undo journal as a client and the next process see it: each save and verb a step in
// `.history/<project>-<hash>/journal.json`, undo and redo on disk, a gesture joined, a write skipped, and a journal
// read back from disk when its project is opened again, a seeded process step with its blobs included — which is
// what a restarted service, or another implementation, finds. Without CONFORMANCE_FILES_ROOT only the answers run.
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { clickWav } from "../../src/instance.ts";
import {
  journalDir,
  onDisk,
  onDiskBytes,
  Service,
  wsDir,
} from "../../src/service.ts";
import { filesRoot, freshWorkspace, NO_FILES_ROOT } from "../../src/sync.ts";

const ws = freshWorkspace();
let svc: Service;
let root = "";
const journals: string[] = [];

test.describe.configure({ mode: "serial", timeout: 120_000 });

test.beforeAll(async () => {
  test.setTimeout(180_000);
  svc = await Service.connect();
  await svc.create(ws);
  root = await svc.root(ws);
  for (const name of ["live.k7", "other.k7", "seeded.k7"]) {
    const r = await svc.post("/project/create", { shape: "live", name }, ws);
    expect(r.status).toBe(200);
    if (filesRoot) journals.push(journalDir(root, name));
  }
  expect((await svc.open(ws, "live.k7")).status).toBe(200);
});

test.afterAll(async () => {
  if (!svc) return;
  test.setTimeout(180_000);
  await svc.remove(ws);
  // No route removes a journal: the spec removes the ones its projects made, by their names.
  for (const dir of journals) rmSync(dir, { recursive: true, force: true });
  await svc.dispose();
});

const journal = (project: string) =>
  JSON.parse(
    readFileSync(join(journalDir(root, project), "journal.json"), "utf8"),
  );
const save = (path: string, content: string) =>
  svc.put("/file", { path, content }, ws);

test("each save of the open project is a step, listed by its project's paths and kept as journal.json", async () => {
  expect((await svc.get("/history", ws)).body).toEqual({
    canUndo: false,
    canRedo: false,
  });
  expect((await save("live.k7/notes.txt", "one\n")).status).toBe(200);
  expect((await save("live.k7/notes.txt", "two\n")).status).toBe(200);
  expect((await svc.get("/history", ws)).body).toEqual({
    canUndo: true,
    canRedo: false,
    nextUndo: {
      kind: "text",
      label: "edit live.k7/notes.txt",
      files: ["live.k7/notes.txt"],
    },
  });
  const entries = (await svc.get("/history/entries", ws)).body;
  expect(entries).toEqual(
    [1, 2].map((id) => ({
      id,
      ts: expect.any(Number),
      kind: "text",
      label: "edit live.k7/notes.txt",
      paths: ["notes.txt"],
      undone: false,
    })),
  );
  test.skip(!filesRoot, NO_FILES_ROOT);
  // Byte for byte, key order included: the next process reads this file as it is.
  const text = readFileSync(
    join(journalDir(root, "live.k7"), "journal.json"),
    "utf8",
  );
  expect(text).toBe(
    JSON.stringify({
      nextId: 3,
      cursor: 2,
      entries: [
        {
          id: 1,
          ts: entries[0].ts,
          kind: "text",
          label: "edit live.k7/notes.txt",
          changes: [
            { path: "live.k7/notes.txt", before: null, after: "one\n" },
          ],
          blobs: [],
        },
        {
          id: 2,
          ts: entries[1].ts,
          kind: "text",
          label: "edit live.k7/notes.txt",
          changes: [
            { path: "live.k7/notes.txt", before: "one\n", after: "two\n" },
          ],
          blobs: [],
        },
      ],
    }),
  );
});

test("a write outside the open project is no step", async () => {
  expect((await save("loose.txt", "x\n")).status).toBe(200);
  expect((await svc.get("/history/entries", ws)).body).toHaveLength(2);
});

test("undo and redo rewrite the files and move the cursor on disk; past either end nothing is applied", async () => {
  const undo = await svc.post("/undo", {}, ws);
  expect([undo.status, undo.body]).toEqual([
    200,
    {
      applied: true,
      kind: "text",
      label: "edit live.k7/notes.txt",
      files: ["live.k7/notes.txt"],
    },
  ]);
  expect((await svc.get("/file?path=live.k7/notes.txt", ws)).body.content).toBe(
    "one\n",
  );
  expect(
    (await svc.get("/history/entries", ws)).body.map(
      (e: { undone: boolean }) => e.undone,
    ),
  ).toEqual([false, true]);
  if (filesRoot) {
    expect(onDisk(ws, "live.k7/notes.txt")).toBe("one\n");
    expect(journal("live.k7").cursor).toBe(1);
  }
  expect((await svc.post("/undo", {}, ws)).body.applied).toBe(true);
  // The first step made the file: undone, it is gone.
  if (filesRoot)
    expect(existsSync(join(wsDir(ws), "live.k7", "notes.txt"))).toBe(false);
  expect((await svc.post("/undo", {}, ws)).body).toEqual({ applied: false });
  expect((await svc.get("/history", ws)).body).toMatchObject({
    canUndo: false,
    canRedo: true,
  });
  expect((await svc.post("/redo", {}, ws)).body.applied).toBe(true);
  expect((await svc.post("/redo", {}, ws)).body.applied).toBe(true);
  expect((await svc.post("/redo", {}, ws)).body).toEqual({ applied: false });
  if (filesRoot) expect(onDisk(ws, "live.k7/notes.txt")).toBe("two\n");
});

test("a step after an undo drops what could be redone, and its id goes on counting", async () => {
  await svc.post("/undo", {}, ws);
  expect((await save("live.k7/notes.txt", "three\n")).status).toBe(200);
  const entries = (await svc.get("/history/entries", ws)).body;
  expect(
    entries.map((e: { id: number; undone: boolean }) => [e.id, e.undone]),
  ).toEqual([
    [1, false],
    [3, false],
  ]);
  if (filesRoot) {
    const j = journal("live.k7");
    expect([j.nextId, j.cursor, j.entries[1].changes]).toEqual([
      4,
      2,
      [{ path: "live.k7/notes.txt", before: "one\n", after: "three\n" }],
    ]);
  }
});

test("a verb is a step; `join` folds the next into it, `skip` journals nothing", async () => {
  const run = (line: string, journal?: "join" | "skip") =>
    svc.verb(ws, line, { want: [], ...(journal ? { journal } : {}) });
  const a = await run("track new gtr");
  expect(a.body.changed).toEqual([
    { path: "chains/gtr.k7x", kind: "added" },
    { path: "tracks/gtr.k7x", kind: "added" },
  ]);
  expect(a.body.revBefore).not.toBe(a.body.rev);
  await run("track new bass", "join");
  const skipped = await run("track new keys", "skip");
  // The reply's tokens and files come from the diff a skipped write is still taken by.
  expect(skipped.body.changed).toEqual([
    { path: "chains/keys.k7x", kind: "added" },
    { path: "tracks/keys.k7x", kind: "added" },
  ]);
  const entries = (await svc.get("/history/entries", ws)).body;
  expect(entries.slice(2)).toEqual([
    {
      id: 4,
      ts: expect.any(Number),
      kind: "text",
      label: "track new gtr",
      paths: [
        "chains/gtr.k7x",
        "tracks/gtr.k7x",
        "chains/bass.k7x",
        "tracks/bass.k7x",
      ],
      undone: false,
    },
  ]);
  const undo = await svc.post("/undo", {}, ws);
  expect(undo.body).toEqual({
    applied: true,
    kind: "text",
    label: "track new gtr",
    files: [
      "live.k7/chains/gtr.k7x",
      "live.k7/tracks/gtr.k7x",
      "live.k7/chains/bass.k7x",
      "live.k7/tracks/bass.k7x",
    ],
  });
  if (filesRoot) {
    for (const f of ["tracks/gtr.k7x", "tracks/bass.k7x"])
      expect(existsSync(join(wsDir(ws), "live.k7", f)), f).toBe(false);
    expect(existsSync(join(wsDir(ws), "live.k7", "tracks", "keys.k7x"))).toBe(
      true,
    );
  }
  const refused = await run("track new keys");
  expect(refused.body).toMatchObject({ refused: true, changed: [] });
  // A refusal lands nothing: the token is the one it began with.
  expect(refused.body.rev).toBe(refused.body.revBefore);
});

test("a pick on a graph's arrow is one step: the node made, then spliced in with `join`, both taken back by one undo", async () => {
  const graph = 'modules({ MIX: ["OUT"] })\n';
  for (const [path, content] of [
    ["pick.k7/project.k7x", 'project({ name: "pick", tempo: 120 })\n'],
    ["pick.k7/graph.k7x", graph],
  ])
    expect((await save(path, content)).status).toBe(200);
  if (filesRoot) journals.push(journalDir(root, "pick.k7"));
  expect((await svc.open(ws, "pick.k7")).status).toBe(200);
  // The two lines the chains view sends for a module picked on the MIX → OUT arrow (apps/k7-daw-frontend/src/App.tsx).
  const run = (line: string, journal?: "join") =>
    svc.verb(ws, line, { want: ["graph"], ...(journal ? { journal } : {}) });
  const made = await run(
    "node new audio/tremolo MIX_tremolo_1 @k7-core/audio/tremolo",
  );
  expect(made.body.refused).toBeFalsy();
  const spliced = await run("node insert MIX OUT MIX_tremolo_1 250,91", "join");
  expect(spliced.body.refused).toBeFalsy();
  const read = async (path: string) =>
    svc.get(`/file?path=${encodeURIComponent(path)}`, ws);
  expect((await read("pick.k7/graph.k7x")).body.content).toMatch(
    /MIX: \["MIX_tremolo_1"\][\s\S]*MIX_tremolo_1: \["OUT"\]/,
  );
  const entries = (await svc.get("/history/entries", ws)).body;
  expect(entries).toHaveLength(1);
  expect(entries[0].paths.sort()).toEqual([
    "graph.k7x",
    "nodes/MIX_tremolo_1.k7x",
  ]);
  expect((await svc.get("/history", ws)).body).toEqual({
    canUndo: true,
    canRedo: false,
    nextUndo: {
      kind: "text",
      label: entries[0].label,
      files: expect.arrayContaining([
        "pick.k7/graph.k7x",
        "pick.k7/nodes/MIX_tremolo_1.k7x",
      ]),
    },
  });
  const undo = await svc.post("/undo", {}, ws);
  expect(undo.body).toMatchObject({ applied: true, kind: "text" });
  expect((await read("pick.k7/graph.k7x")).body.content).toBe(graph);
  expect((await read("pick.k7/nodes/MIX_tremolo_1.k7x")).status).toBe(404);
  expect((await svc.get("/history", ws)).body).toMatchObject({
    canUndo: false,
    canRedo: true,
  });
  // Back to the project the later tests read.
  expect((await svc.open(ws, "live.k7")).status).toBe(200);
});

test("a verb's output names each file once, spelled with /, whatever the server's platform", async () => {
  const r = await svc.verb(ws, "track new pad", { want: [] });
  expect(r.body.output).toEqual([
    "+ track pad (tracks/pad.k7x, chains/pad.k7x)",
  ]);
});

test("a journal changed on disk is what the project's next open reads", async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  expect((await svc.open(ws, "other.k7")).status).toBe(200);
  // Asked of the other project, the journal in memory is that one's: live.k7's is read again at its next use.
  expect((await svc.get("/history", ws)).body).toEqual({
    canUndo: false,
    canRedo: false,
  });
  // Kept to its first step, as another process would have left it.
  const j = journal("live.k7");
  const kept = { nextId: 2, cursor: 1, entries: [j.entries[0]] };
  writeFileSync(
    join(journalDir(root, "live.k7"), "journal.json"),
    JSON.stringify(kept),
  );
  expect((await svc.open(ws, "live.k7")).status).toBe(200);
  const entries = (await svc.get("/history/entries", ws)).body;
  expect(entries.map((e: { id: number }) => e.id)).toEqual([1]);
  expect((await svc.post("/undo", {}, ws)).body.applied).toBe(true);
  expect(existsSync(join(wsDir(ws), "live.k7", "notes.txt"))).toBe(false);
  expect(journal("live.k7")).toEqual({ ...kept, cursor: 0 });
});

test("a process step found on disk asks to be confirmed, then restores its audio from the journal's blobs", async () => {
  test.skip(!filesRoot, NO_FILES_ROOT);
  const before = clickWav(1);
  const after = clickWav(2);
  const up = await svc.call(
    "POST",
    "/upload?dir=seeded.k7/clips&name=take.wav",
    {
      ws,
      data: after,
      headers: { "content-type": "application/octet-stream" },
    },
  );
  expect(up.status).toBe(200);
  expect((await save("seeded.k7/clips/take.k7x", "new\n")).status).toBe(200);
  // What a bounce journals: the clip repointed as text, the render's two images as blobs, kept on no engine.
  const dir = journalDir(root, "seeded.k7");
  mkdirSync(join(dir, "blobs"), { recursive: true });
  writeFileSync(join(dir, "blobs", "1-b0"), before);
  writeFileSync(join(dir, "blobs", "1-a0"), after);
  const step = {
    id: 1,
    ts: 1_700_000_000_000,
    kind: "process",
    label: "track process take",
    changes: [
      { path: "seeded.k7/clips/take.k7x", before: "old\n", after: "new\n" },
    ],
    blobs: [
      { path: "seeded.k7/clips/take.wav", before: "1-b0", after: "1-a0" },
    ],
    keptOn: null,
  };
  writeFileSync(
    join(dir, "journal.json"),
    JSON.stringify({ nextId: 2, cursor: 1, entries: [step] }),
  );
  expect((await svc.open(ws, "seeded.k7")).status).toBe(200);
  const peek = {
    kind: "process",
    label: "track process take",
    files: ["seeded.k7/clips/take.k7x", "seeded.k7/clips/take.wav"],
  };
  expect((await svc.get("/history", ws)).body).toEqual({
    canUndo: true,
    canRedo: false,
    nextUndo: peek,
  });
  const ask = await svc.post("/undo", {}, ws);
  expect([ask.status, ask.body]).toEqual([
    200,
    { needsConfirm: true, ...peek },
  ]);
  expect(onDiskBytes(ws, "seeded.k7/clips/take.wav").equals(after)).toBe(true);
  const undo = await svc.post("/undo", { confirm: true }, ws);
  expect(undo.body).toEqual({ applied: true, ...peek });
  expect(onDiskBytes(ws, "seeded.k7/clips/take.wav").equals(before)).toBe(true);
  expect(onDisk(ws, "seeded.k7/clips/take.k7x")).toBe("old\n");
  expect(journal("seeded.k7")).toEqual({
    nextId: 2,
    cursor: 0,
    entries: [step],
  });
  // Kept on no engine, the blobs stay in the account's cache for the redo.
  expect(existsSync(join(dir, "blobs", "1-a0"))).toBe(true);
  expect((await svc.post("/redo", {}, ws)).body).toEqual({
    needsConfirm: true,
    ...peek,
  });
  expect((await svc.post("/redo", { confirm: true }, ws)).body.applied).toBe(
    true,
  );
  expect(onDiskBytes(ws, "seeded.k7/clips/take.wav").equals(after)).toBe(true);
});
