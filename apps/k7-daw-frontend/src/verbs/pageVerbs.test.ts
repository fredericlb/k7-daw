// pageVerbs.test.ts — the page's host over fake POSTs: a line, a completion, the picture and its warnings, an import, a long run.
// What a reader gets wrong: the reads go through `read`, never `send`, so the reads around them stay shared.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPageVerbs } from "./pageVerbs.ts";

function host(answers: Record<string, unknown> = {}) {
  const log: string[] = [];
  let slow = 0;
  const tracked: { label: string; ended: string | undefined | null }[] = [];
  const post =
    (kind: string) =>
    async <T>(path: string, body: unknown): Promise<T> => {
      log.push(`${kind} ${path} ${JSON.stringify(body)}`);
      if (slow) await new Promise((ok) => setTimeout(ok, slow));
      const a = answers[path];
      if (a instanceof Error) throw a;
      return a as T;
    };
  const verbs = createPageVerbs({
    send: post("send"),
    read: post("read"),
    catalogue: async () => [],
    track: (label) => {
      const t = { label, ended: null as string | undefined | null };
      tracked.push(t);
      return {
        end: (e) => {
          t.ended = e;
        },
      };
    },
  });
  const slowBy = (n: number): void => {
    slow = n;
  };
  return { verbs, log, tracked, slowBy };
}

const ran = {
  log: ["done"],
  refuse: [],
  refused: false,
  landed: true,
  changed: [{ path: "live.k7/project.k7x", kind: "text" }],
  rev: "b",
  revBefore: "a",
};

describe("the page's verbs host", () => {
  beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
  afterEach(() => vi.useRealTimers());

  it("runs a line through POST /verb, with its want, folder and journal, and words the answer", async () => {
    const h = host({ "/verb": { ...ran, threw: "boom" } });
    const r = await h.verbs.run("project tempo 96", {
      want: ["graph"],
      at: "live.k7",
      journal: "join",
    });
    expect(h.log).toEqual([
      'send /verb {"line":"project tempo 96","want":["graph"],"at":"live.k7","journal":"join"}',
    ]);
    expect(r).toEqual({
      output: ["done", "error: boom"],
      refused: false,
      rev: "b",
      revBefore: "a",
      changed: ran.changed,
    });
  });

  it("completes through POST /verb/complete as a read, and a failure is no items", async () => {
    const h = host({ "/verb/complete": { items: ["node new"] } });
    expect(await h.verbs.complete("node n")).toEqual(["node new"]);
    expect(h.log).toEqual([
      'read /verb/complete {"line":"node n","labelled":true}',
    ]);
    const broken = host({ "/verb/complete": new Error("down") });
    expect(await broken.verbs.complete("x")).toEqual([]);
  });

  it("reads the picture and its warnings from POST /picture, a refusal as an answer", async () => {
    const doc = { name: "p", videoTracks: [] };
    const h = host({ "/picture": { doc, warnings: ["w"] } });
    expect(await h.verbs.videoDoc()).toEqual({ doc });
    expect(await h.verbs.videoErrors()).toEqual({
      errors: ["w"],
      diagnostics: [],
    });
    const bad = host({ "/picture": { error: "no viewport" } });
    expect(await bad.verbs.videoDoc()).toEqual({ error: "no viewport" });
    expect((await bad.verbs.videoErrors()).errors).toEqual(["no viewport"]);
    expect(h.log.every((l) => l.startsWith("read /picture"))).toBe(true);
  });

  it("imports a workspace file through POST /import and lists it while it runs", async () => {
    const h = host({ "/import": { project: "song", log: ["wrote song"] } });
    expect(await h.verbs.importPath("Live/Song.als", "song")).toEqual({
      project: "song",
      log: ["wrote song"],
    });
    expect(h.log).toEqual(['send /import {"rel":"Live/Song.als","as":"song"}']);
    expect(h.tracked).toEqual([
      { label: "import Live/Song.als", ended: undefined },
    ]);
    const refused = host({ "/import": new Error("taken") });
    await expect(refused.verbs.importPath("a.als")).rejects.toThrow("taken");
    expect(refused.tracked[0]?.ended).toBe("taken");
  });

  it("lists a run past a second in the process list and ends it", async () => {
    const h = host({ "/verb": ran });
    h.slowBy(1500);
    const r = h.verbs.run("event trim drums", { want: [] });
    await vi.advanceTimersByTimeAsync(1600);
    await r;
    expect(h.tracked).toEqual([
      { label: "event trim drums", ended: undefined },
    ]);
  });
});
