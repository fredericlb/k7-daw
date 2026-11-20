// pageVerbs.test.ts — the page's host over a fake Worker handle: the project it opens, the session that ends the Worker, a long run, a foreign change.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createPageVerbs } from "./pageVerbs.ts";
import { VERBS_RESET, type Verbs } from "./verbsWorker.ts";

vi.mock("@studio/project-shell/fileApi", () => ({
  fget: async () => ({ rev: "r1" }),
}));

function fakeVerbs() {
  const log: string[] = [];
  let slow = 0;
  const verbs = {
    ready: async () => {},
    catalogue: async () => {
      log.push("catalogue");
      return [];
    },
    open: async (at: string) => {
      log.push(`open ${at}`);
    },
    run: async (line: string, o: unknown, at?: string) => {
      log.push(`run ${line} @${at} ${JSON.stringify(o)}`);
      if (slow) await new Promise((ok) => setTimeout(ok, slow));
      return {
        output: [line],
        before: "a",
        after: "b",
        changed: [],
        runs: 1,
        committed: true,
        refused: false,
      };
    },
    complete: async (line: string, _l?: boolean, at?: string) => {
      log.push(`complete ${line} @${at}`);
      return [line];
    },
    refresh: async () => {},
    close: async () => {},
    reset: () => void log.push("reset"),
  };
  return {
    log,
    verbs: verbs as unknown as Verbs,
    slowBy: (n: number) => (slow = n),
  };
}

let key = "k1";
let project = "live.k7";
const tracked: { label: string; ended: string | undefined | null }[] = [];

function make() {
  const f = fakeVerbs();
  const page = createPageVerbs({
    verbs: f.verbs,
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
  it("opens the active project once and runs on it, the graph being the caller's want", async () => {
    const { page, log } = make();
    await page.run("a", { want: ["graph"] });
    await page.run("b", { want: [] });
    expect(log.filter((l) => l.startsWith("open"))).toEqual(["open live.k7"]);
    expect(log).toContain('run b @live.k7 {"want":[]}');
  });

  it("opens the project once for the calls a page makes as it opens", async () => {
    const { page, log } = make();
    await Promise.all([
      page.complete("node"),
      page.run("graph pin", { want: [] }),
      page.run("node param master Width 101", { want: [] }),
      page.run("node param master Width 102", { want: [] }),
    ]);
    expect(log.filter((l) => l.startsWith("open"))).toEqual(["open live.k7"]);
  });

  it("closes the runtime of the project the page left", async () => {
    const { page, log, verbs } = make();
    const closed: string[] = [];
    (verbs as unknown as { close: (at?: string) => Promise<void> }).close =
      async (at) => {
        closed.push(at ?? "");
      };
    page.setProject("a.k7");
    await page.run("x", { want: [] });
    page.setProject("b.k7");
    expect(closed).toEqual(["a.k7"]);
    await page.run("y", { want: [] });
    expect(log.filter((l) => l.startsWith("open"))).toEqual([
      "open a.k7",
      "open b.k7",
    ]);
  });

  it("runs on the folder it is given, a runtime of its own", async () => {
    const { page, log } = make();
    await page.run("x", { want: [], at: "_libraries/k7-core/chains/c" });
    expect(log).toContain("open _libraries/k7-core/chains/c");
    expect(log.some((l) => l.includes("@_libraries/k7-core/chains/c"))).toBe(
      true,
    );
  });

  it("starts the Worker over when the session changes, and opens the project again", async () => {
    const { page, log } = make();
    await page.run("a", { want: [] });
    key = "k2";
    await page.run("b", { want: [] });
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

  it("answers a completion with no items when the Worker fails", async () => {
    const { page, verbs } = make();
    (verbs as unknown as { complete: () => Promise<never> }).complete =
      async () => {
        throw new Error("loading");
      };
    expect(await page.complete("x")).toEqual([]);
  });

  it("lists a run past a second in the process list and ends it with the error", async () => {
    vi.useFakeTimers();
    const { page, slowBy, verbs } = make();
    slowBy(1500);
    const run = page.run("track process", { want: [] });
    await vi.advanceTimersByTimeAsync(1200);
    expect(tracked.map((t) => t.label)).toEqual(["track process"]);
    await vi.advanceTimersByTimeAsync(600);
    await run;
    expect(tracked[0]?.ended).toBeUndefined();
    (verbs as unknown as { run: () => Promise<never> }).run = async () => {
      throw new Error("boom");
    };
    vi.useRealTimers();
    await expect(page.run("quick", { want: [] })).rejects.toThrow("boom");
  });

  it("reopens the project in the background after a change it did not make", async () => {
    vi.useFakeTimers();
    const { page, log } = make();
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
