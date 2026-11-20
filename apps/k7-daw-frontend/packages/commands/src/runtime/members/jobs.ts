// jobs.ts — how a host member reaches the server: one call, or a job started and polled each second until it ends (3f).
// What a reader gets wrong: every call marks the run `hostCalled` BEFORE the request, so a 409 afterwards is said, never rerun.

import { NoFfmpegError } from "../../hostTypes.ts";
import type { RunFlags } from "../host.ts";
import type { HostAnswer, JobState, Remote } from "../remote.ts";
import type { Vfs } from "../vfs.ts";

/** A job is asked after every second, as `COLD_POLL_MS` asks a separation. */
export const JOB_POLL_MS = 1000;

/** What a member needs of its run: the server, the run's flags, and the page closing under it. */
export interface Hosting {
  remote: Remote;
  flags: RunFlags;
  /** Aborted when the runtime closes: a poll stops and the job is left to the server's heartbeat. */
  signal?: AbortSignal;
  pollMs?: number;
  /** The project the runtime opened, as `/changes` names it; sent as `at`, never the verb's virtual `songPath`. */
  at?: string;
}

/** A refused request or a failed job: the sentence is the server's, the code is `off`, `busy`, `no-engine`… */
export class HostError extends Error {
  constructor(
    message: string,
    readonly code?: string,
    readonly status?: number,
  ) {
    super(message);
    this.name = "HostError";
  }
}

/** The error a verb's own catch words: `no-ffmpeg` is the class the picture verbs catch, any other a `HostError`. */
export function failure(
  error: string,
  code?: string,
  status?: number,
): HostError | NoFfmpegError {
  return code === "no-ffmpeg"
    ? new NoFfmpegError("ffmpeg", error)
    : new HostError(error, code, status);
}

function refused(a: HostAnswer): Error {
  const error = typeof a.body.error === "string" ? a.body.error : "";
  const code = typeof a.body.code === "string" ? a.body.code : undefined;
  return failure(error || `the server answered ${a.status}`, code, a.status);
}

/** One request to a host route; a body of `null` is a GET with `query`. Throws on any status past 299. */
export async function hostCall<T = Record<string, unknown>>(
  h: Hosting,
  path: string,
  body: Record<string, unknown> | null,
  query?: string,
): Promise<T> {
  // A closed page starts nothing: the request would outlive the run that asked for it.
  if (h.signal?.aborted) throw new HostError("closed", "closed");
  h.flags.hostCalled = true;
  const sent = body && h.at !== undefined ? { ...body, at: h.at } : body;
  const a = await h.remote.host(path, sent, query);
  if (a.status > 299) {
    // A member the server does not serve refuses the whole run, whatever the verb does with the throw.
    if (a.body.code === "unsupported") h.flags.unsupported = path.split("/")[0];
    throw refused(a);
  }
  return a.body as T;
}

const sleep = (ms: number, signal?: AbortSignal): Promise<void> =>
  new Promise((ok, fail) => {
    if (signal?.aborted) return fail(new HostError("closed", "closed"));
    const t = setTimeout(ok, ms);
    signal?.addEventListener(
      "abort",
      () => {
        clearTimeout(t);
        fail(new HostError("closed", "closed"));
      },
      { once: true },
    );
  });

/** What a finished job leaves: its id (the outputs are below it) and the value it answered. */
export interface Finished<T> {
  job: string;
  value: T;
}

/** A job started by `path` and polled until it ends; a failed job throws as a refused request does. */
export async function hostJob<T = Record<string, unknown>>(
  h: Hosting,
  path: string,
  body: Record<string, unknown>,
): Promise<Finished<T>> {
  const { job } = await hostCall<{ job: string }>(h, path, body);
  for (;;) {
    const s: JobState = await h.remote.job(job);
    if (s.state === "done") return { job, value: s.value as T };
    if (s.state === "failed") throw failure(s.error, s.code);
    await sleep(h.pollMs ?? JOB_POLL_MS, h.signal);
  }
}

/** A file a job staged: its name below the job's folder, its length, and a wav's seconds. */
export interface JobFile {
  out: string;
  len: number;
  seconds?: number | null;
}

/** Puts a job's files in the Vfs where `place` says, each as an adopt the run's change set lands. */
export function adoptFiles(
  vfs: Vfs,
  job: string,
  files: JobFile[],
  place: (out: string) => string,
): void {
  for (const f of files)
    vfs.adopt(job, f.out, place(f.out), {
      len: f.len,
      ...(typeof f.seconds === "number" ? { seconds: f.seconds } : {}),
    });
}
