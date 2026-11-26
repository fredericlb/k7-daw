// pageVerbs.test.ts — the page's host over a fake POST and a fake Worker handle: a line, a completion, the picture's project, a long run, a foreign change.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPageVerbs } from "./pageVerbs.ts";
import { VERBS_RESET, type Verbs } from "./verbsWorker.ts";

vi.mock("@studio/project-shell/fileApi", () => ({
  fget: async () => ({ rev: "r1" }),
}));

function fakeVerbs() {
  const log: string[] = [];
  let slow = 0;
  let answer: Record<string, unknown> = {
    log: ["done"],
    refuse: [],
    refused: false,
    landed: true,
    changed: [{ path: "live.k7/project.k7x", kind: "text" }],
    rev: "b",
    revBefore: "a",
  };
  const verbs = {
    ready: async () => {},
    open: async (at: string) => {
      log.push(`open ${at}`);
    },
    videoDoc: async (rev: string, at?: string) => {
      log.push(`videoDoc ${rev} @${at}`);
      return { doc: { videoTracks: [] } };
    },
    videoErrors: async () => ({ errors: [], diagnostics: [] }),
    refresh: async (files: string[], rev?: string, at?: string) => {
      log.push(`refresh ${files.join(",")} ${rev} @${at}`);
    },
    close: async () => {},
    reset: () => void log.push("reset"),
  };
  const send = async <T>(path: string, body: unknown): Promise<T> => {
    log.push(`${path} ${JSON.stringify(body)}`);
    if (slow) await new Promise((ok) => setTimeout(ok, slow));
    if (path === "/verb/complete") return { items: ["node new"] } as T;
    return answer as T;
  };
  return {
    log,
    verbs: verbs as unknown as Verbs,
    send,
    slowBy: (n: number) => (slow = n),
    answer: (a: Record<string, unknown>) => (answer = a),
  };
}

let key = "k1";
let project = "live.k7";
const tracked: { label: string; ended: string | undefined | null }[] = [];

function make() {
  const f = fakeVerbs();
  const page = createPageVerbs({
    verbs: f.verbs,
    send: f.send,
    catalogue: async () => [],
    session: () => ({ key }),
    activeProject: async () => project,
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
  return { ...f, page };
}

beforeEach(() => {
  key = "k1";
  project = "live.k7";
  tracked.length = 0;
});
afterEach(() => vi.useRealTimers());

describe("the page's verbs", () => {
  it("runs a line through POST /verb, with its want, folder and journal, and words the answer", async () => {
    const { page, log, answer } = make();
    const r = await page.run("project tempo 96", {
      want: ["graph"],
      at: "_libraries/k7-core/chains/c",
      journal: "join",
    });
    expect(log).toEqual([
      '/verb {"line":"project tempo 96","want":["graph"],"at":"_libraries/k7-core/chains/c","journal":"join"}',
    ]);
    expect(r).toEqual({
      output: ["done"],
      refused: false,
      rev: "b",
      revBefore: "a",
      changed: [{ path: "live.k7/project.k7x", kind: "text" }],
    });
    answer({ log: [], refuse: ['no track "x"'], refused: true, threw: "boom" });
    const no = await page.run("track rename x y", { want: [] });
    expect(no).toEqual({
      output: ['no track "x"', "error: boom"],
      refused: true,
    });
  });

  it("opens no Worker for a verb, and hands the files a verb wrote to one that holds the project", async () => {
    const { page, log } = make();
    await page.run("a", { want: [] });
    expect(log.some((l) => l.startsWith("open"))).toBe(false);
    await page.videoDoc();
    log.length = 0;
    await page.run("b", { want: [] });
    expect(log).toContain("refresh live.k7/project.k7x b @live.k7");
  });

  it("opens the project once for the picture views a page asks as it opens", async () => {
    const { page, log } = make();
    await Promise.all([page.videoDoc(), page.videoErrors(), page.videoDoc()]);
    expect(log.filter((l) => l.startsWith("open"))).toEqual(["open live.k7"]);
  });

  it("completes through POST /verb/complete, labelled", async () => {
    const { page, log } = make();
    expect(await page.complete("node n")).toEqual(["node new"]);
    expect(log).toEqual(['/verb/complete {"line":"node n","labelled":true}']);
  });

  it("closes the runtime of the project the page left", async () => {
    const { page, log, verbs } = make();
    const closed: string[] = [];
    (verbs as unknown as { close: (at?: string) => Promise<void> }).close =
      async (at) => {
        closed.push(at ?? "");
      };
    page.setProject("a.k7");
    await page.videoDoc();
    page.setProject("b.k7");
    expect(closed).toEqual(["a.k7"]);
    await page.videoDoc();
    expect(log.filter((l) => l.startsWith("open"))).toEqual([
      "open a.k7",
      "open b.k7",
    ]);
  });

  it("starts the Worker over when the session changes, and opens the project again", async () => {
    const { page, log } = make();
    await page.videoDoc();
    key = "k2";
    await page.videoDoc();
    expect(log.filter((l) => l === "reset")).toHaveLength(1);
    expect(log.filter((l) => l.startsWith("open"))).toHaveLength(2);
  });

  it("asks the picture again when the session's change failed it in flight", async () => {
    const { page, verbs } = make();
    let asked = 0;
    (verbs as unknown as { videoDoc: () => Promise<unknown> }).videoDoc =
      async () => {
        asked++;
        if (asked === 1) throw new Error(VERBS_RESET);
        return { doc: { videoTracks: [{ id: "main" }] } };
      };
    await expect(page.videoDoc()).resolves.toEqual({
      doc: { videoTracks: [{ id: "main" }] },
    });
    expect(asked).toBe(2);
  });

  it("lists a run past a second in the process list and ends it", async () => {
    vi.useFakeTimers();
    const { page, slowBy } = make();
    slowBy(1500);
    const run = page.run("track process", { want: [] });
    await vi.advanceTimersByTimeAsync(1200);
    expect(tracked.map((t) => t.label)).toEqual(["track process"]);
    await vi.advanceTimersByTimeAsync(600);
    await run;
    expect(tracked[0]?.ended).toBeUndefined();
  });

  it("reopens the project in the background after a change it did not make", async () => {
    vi.useFakeTimers();
    const { page, log } = make();
    await page.videoDoc();
    await page.run("a", { want: [] });
    log.length = 0;
    page.foreignChange("b");
    await vi.advanceTimersByTimeAsync(1000);
    expect(log).toEqual([]);
    page.foreignChange("c");
    page.foreignChange("c");
    await vi.advanceTimersByTimeAsync(600);
    expect(log).toEqual(["open live.k7"]);
  });
});
