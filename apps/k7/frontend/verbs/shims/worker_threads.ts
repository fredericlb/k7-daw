// worker_threads.ts — `node:worker_threads` in the verbs' Worker: a stand-in whose Worker refuses to start.
// What a reader gets wrong: song's stamp walk starts a thread inside a try and falls back to walking inline.

export class Worker {
  constructor() {
    throw new Error("worker_threads is not available in the browser");
  }
}

export const parentPort = null;
export const isMainThread = true;
