// verbsWorker.ts — the page's side of the project's Worker (picture views, import): one Worker, started lazily, its calls surviving its death.
// What a reader gets wrong: a crash fails the calls in flight with one sentence and the NEXT call starts a new Worker and reopens the projects.

import { client, type PortLike } from "@studio/commands/runtime/protocol";

/** What the page needs of a Worker: a port, an error event and a way to end it. */
export interface WorkerLike extends PortLike {
  addEventListener(
    type: "error",
    listener: (e: { message?: string }) => void,
  ): void;
  addEventListener(
    type: "message",
    listener: (e: { data: unknown }) => void,
  ): void;
  terminate(): void;
}

export interface VerbsOptions {
  /** Starts a Worker; a test passes one over a MessageChannel. */
  spawn: () => WorkerLike;
  base: string | (() => string);
  headers: () => Record<string, string>;
  /** Told true when a call has waited past `loadingMs`, false when the Worker answers. */
  onLoading?: (loading: boolean) => void;
  loadingMs?: number;
}

type Calls = ReturnType<typeof client>;

export const WORKER_DIED =
  "the project's Worker stopped unexpectedly — ask again";

/** What a call in flight is failed with when the session changes under it. */
export const VERBS_RESET = "the verbs were reset";

export function createVerbs(o: VerbsOptions) {
  let worker: WorkerLike | undefined;
  let calls: Calls | undefined;
  let ready: Promise<Calls> | undefined;
  // The projects the page opened, in order: a new Worker opens them again before the call that started it.
  const opened = new Set<string>();
  let waiting = 0;
  let slow: ReturnType<typeof setTimeout> | undefined;
  let shown = false;

  function start(): Promise<Calls> {
    const w = o.spawn();
    const c = client(w);
    worker = w;
    calls = c;
    w.addEventListener("error", (e) => {
      if (worker !== w) return;
      worker = undefined;
      ready = undefined;
      calls = undefined;
      w.terminate();
      c.fail(
        new Error(e.message ? `${WORKER_DIED} (${e.message})` : WORKER_DIED),
      );
    });
    return c
      .init(typeof o.base === "function" ? o.base() : o.base, o.headers())
      .then(async () => {
        // A project that is not there any more must not keep every later start from answering.
        for (const at of [...opened]) {
          try {
            await c.open(at);
          } catch {
            opened.delete(at);
          }
        }
        return c;
      })
      .catch((e: Error) => {
        if (worker === w) {
          worker = undefined;
          ready = undefined;
          calls = undefined;
          w.terminate();
        }
        throw e;
      });
  }

  async function call<T>(fn: (c: Calls) => Promise<T>): Promise<T> {
    waiting++;
    slow ??= setTimeout(() => {
      shown = true;
      o.onLoading?.(true);
    }, o.loadingMs ?? 300);
    try {
      ready ??= start();
      const c = await ready;
      return await fn(c);
    } finally {
      if (--waiting === 0) {
        clearTimeout(slow);
        slow = undefined;
        if (shown) o.onLoading?.(false);
        shown = false;
      }
    }
  }

  return {
    /** Starts the Worker if it is not up; resolves when it has answered `init`. */
    ready: async (): Promise<void> => {
      await call(async () => undefined);
    },
    async open(at: string): Promise<void> {
      await call((c) => c.open(at));
      opened.add(at);
    },
    videoDoc: (token?: string, at?: string) =>
      call((c) => c.videoDoc(token, at)),
    videoErrors: (token?: string, at?: string) =>
      call((c) => c.videoErrors(token, at)),
    pictureScene: (token?: string, at?: string) =>
      call((c) => c.pictureScene(token, at)),
    refresh: (files: string[], rev?: string, at?: string) =>
      call((c) => c.refresh(files, rev, at)),
    /** A project file the page holds, into a new folder: the Worker reads, uploads and lands it. */
    importFile: (data: Blob | ArrayBuffer, name: string, as?: string) =>
      call((c) => c.importFile(data, name, as)),
    /** A project file already in the workspace, imported from where it sits. */
    importPath: (rel: string, as?: string) =>
      call((c) => c.importPath(rel, as)),
    async close(at?: string): Promise<void> {
      if (at !== undefined) opened.delete(at);
      else opened.clear();
      if (worker) await call((c) => c.close(at));
    },
    /** A workspace switch or sign-out: the Worker's runtimes go and the next call starts it again with the new headers. */
    reset(): void {
      opened.clear();
      worker?.terminate();
      worker = undefined;
      calls?.fail(new Error(VERBS_RESET));
      calls = undefined;
      ready = undefined;
    },
  };
}

export type Verbs = ReturnType<typeof createVerbs>;
