// browserWorker.ts — the real Worker, built from the URL the Vite plugin names.
// What a reader gets wrong: nothing imports this at boot; the page asks for the Worker after its first paint.

import workerUrl from "virtual:k7-verbs-worker";
import type { WorkerLike } from "./verbsWorker.ts";

export const spawnBrowserWorker = (): WorkerLike =>
  new Worker(workerUrl, { type: "module", name: "k7-verbs" }) as WorkerLike;
