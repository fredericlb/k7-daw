// verbsWorker.test.ts — the page's Worker handle over a MessageChannel and `serve`: waiting, a crash and its restart, the loading hint.

import { MessageChannel, type MessagePort } from "node:worker_threads";
import type { Runtime } from "@studio/commands/runtime";
import type { Importer } from "@studio/commands/runtime/importer";
import { serve } from "@studio/commands/runtime/protocol";
import { describe, expect, it, vi } from "vitest";
import { createVerbs, WORKER_DIED, type WorkerLike } from "./verbsWorker.ts";

const portLike = (p: MessagePort) => ({
  postMessage: (m: unknown) => p.postMessage(m),
  addEventListener: (_t: "message", f: (e: { data: unknown }) => void) =>
    p.on("message", (data) => f({ data })),
});

/** A Runtime that records what it was asked and answers `slow` ms late on `videoDoc`. */
const gone = new Set<string>();

function stubRuntime(log: string[], slow = 0): Runtime {
  return {
    open: async (at) => {
      if (gone.has(at)) throw new Error(`'${at}' is not the open project`);
      log.push(`open ${at}`);
    },
    videoDoc: async (token) => {
      await new Promise((ok) => setTimeout(ok, slow));
      log.push(`videoDoc ${token}`);
      return { error: "stub" };
    },
    videoErrors: async (token) => {
      log.push(`videoErrors ${token}`);
      return { errors: [], diagnostics: [] };
    },
    refresh: async () => {},
    close: () => {},
    revision: undefined,
  };
}

function harness(slow = 0, importer?: Importer) {
  const log: string[] = [];
  const spawned: { crash: (m?: string) => void; ended: boolean }[] = [];
  const errors: ((e: { message?: string }) => void)[] = [];
  const spawn = (): WorkerLike => {
    const { port1, port2 } = new MessageChannel();
    serve(
      portLike(port1),
      () => stubRuntime(log, slow),
      importer ? () => importer : undefined,
    );
    port1.unref();
    port2.unref();
    const me = { crash: (_m?: string) => {}, ended: false };
    const listeners: ((e: { message?: string }) => void)[] = [];
    me.crash = (m) => {
      for (const f of listeners) f({ message: m });
    };
    spawned.push(me);
    errors.length = 0;
    return {
      postMessage: (m) => port2.postMessage(m),
      addEventListener: ((t: string, f: never) => {
        if (t === "error") listeners.push(f);
        else
          port2.on("message", (data) => (f as (e: unknown) => void)({ data }));
      }) as WorkerLike["addEventListener"],
      terminate: () => {
        me.ended = true;
        port2.close();
      },
    };
  };
  return { log, spawned, spawn };
}

describe("the project's Worker handle", () => {
  it("hands a dropped file to the Worker's importer by its name and the folder asked for, and a path by its own", async () => {
    const seen: unknown[][] = [];
    const importer: Importer = {
      importFile: async (data, name, as) => {
        seen.push(["file", (data as Blob).size, name, as]);
        return { project: as ?? "", log: ["wrote"] };
      },
      importPath: async (rel, as) => {
        seen.push(["path", rel, as]);
        return { project: "p", log: [] };
      },
    };
    const h = harness(0, importer);
    const verbs = createVerbs({
      spawn: h.spawn,
      base: "/b",
      headers: () => ({}),
    });
    const r = await verbs.importFile(new Blob(["abc"]), "Song.zip", "song.k7");
    expect(r).toEqual({ project: "song.k7", log: ["wrote"] });
    await verbs.importPath("Live/Song.als");
    expect(seen).toEqual([
      ["file", 3, "Song.zip", "song.k7"],
      ["path", "Live/Song.als", undefined],
    ]);
    // An import opens no project: a Worker restarted after a crash has none to reopen.
    expect(h.log).toEqual([]);
  });

  it("restarts after a crash even when a project it held is gone", async () => {
    gone.clear();
    const h = harness();
    const v = createVerbs({
      spawn: h.spawn,
      base: "/api",
      headers: () => ({}),
    });
    await v.open("p.k7");
    await v.open("q.k7");
    gone.add("p.k7");
    h.spawned[0]?.crash("out of memory");
    expect(await v.videoDoc("next", "q.k7")).toEqual({ error: "stub" });
    gone.clear();
  });

  it("starts on the first call and keeps one Worker", async () => {
    const h = harness();
    const v = createVerbs({
      spawn: h.spawn,
      base: "/api",
      headers: () => ({}),
    });
    expect(h.spawned).toHaveLength(0);
    await v.ready();
    await v.open("p.k7");
    expect(await v.videoDoc("a")).toEqual({ error: "stub" });
    expect(h.spawned).toHaveLength(1);
  });

  it("carries the picture views and the revision the page names", async () => {
    const h = harness();
    const v = createVerbs({
      spawn: h.spawn,
      base: "/api",
      headers: () => ({}),
    });
    await v.open("p.k7");
    expect(await v.videoDoc("r1", "p.k7")).toEqual({ error: "stub" });
    expect(await v.videoErrors("r2", "p.k7")).toEqual({
      errors: [],
      diagnostics: [],
    });
    expect(h.log).toEqual(["open p.k7", "videoDoc r1", "videoErrors r2"]);
  });

  it("fails the calls in flight with one sentence when the Worker dies, then starts again and reopens", async () => {
    const h = harness(40);
    const v = createVerbs({
      spawn: h.spawn,
      base: "/api",
      headers: () => ({}),
    });
    await v.open("p.k7");
    await v.open("q.k7");
    const pending = v.videoDoc("slowline");
    await vi.waitFor(() => expect(h.spawned).toHaveLength(1));
    await new Promise((ok) => setTimeout(ok, 5));
    h.spawned[0]?.crash("out of memory");
    await expect(pending).rejects.toThrow(WORKER_DIED);
    expect(h.spawned[0]?.ended).toBe(true);
    h.log.length = 0;
    expect(await v.videoDoc("next")).toEqual({ error: "stub" });
    expect(h.spawned).toHaveLength(2);
    // The old Worker's stub still finishes its slow line; only the new one's calls count.
    expect(h.log.filter((x) => x !== "videoDoc slowline")).toEqual([
      "open p.k7",
      "open q.k7",
      "videoDoc next",
    ]);
  });

  it("says the verbs are loading only past the delay, and stops saying it", async () => {
    const h = harness(60);
    const seen: boolean[] = [];
    const v = createVerbs({
      spawn: h.spawn,
      base: "/api",
      headers: () => ({}),
      loadingMs: 20,
      onLoading: (l) => seen.push(l),
    });
    await v.open("p.k7");
    await v.videoDoc("slow");
    expect(seen).toEqual([true, false]);
    const quick = harness(0);
    const w = createVerbs({
      spawn: quick.spawn,
      base: "/api",
      headers: () => ({}),
      loadingMs: 500,
      onLoading: (l) => seen.push(l),
    });
    await w.open("p.k7");
    expect(seen).toEqual([true, false]);
  });

  it("a reset ends the Worker and the next call starts a fresh one without the old projects", async () => {
    const h = harness();
    const v = createVerbs({
      spawn: h.spawn,
      base: "/api",
      headers: () => ({}),
    });
    await v.open("p.k7");
    v.reset();
    expect(h.spawned[0]?.ended).toBe(true);
    h.log.length = 0;
    await v.ready();
    expect(h.spawned).toHaveLength(2);
    expect(h.log).toEqual([]);
  });
});
