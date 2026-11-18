// remote.ts — what the runtime asks of the server, as an interface, and its HTTP form over `/verb-bundle` and `/changes`.
// What a reader gets wrong: `changes` answers a status and a body, never throws on 409 — the runtime's rerun reads it.

import {
  ENGINE_TOKEN_HEADER,
  USER_HEADER,
  WORKSPACE_HEADER,
} from "@studio/web-protocol";
import type { StagedOp } from "./vfs.ts";

/** A file as the bundle holds it: its text, or its length for a binary. */
export interface BundleFile {
  text?: string;
  len?: number;
  binary?: boolean;
  io?: string;
}

export interface Entry {
  name: string;
  kind: "file" | "dir";
}

/** `GET /verb-bundle`'s answer. */
export interface VerbBundle {
  format: number;
  at: string;
  rev: string;
  source: {
    format: number;
    mounts: string[];
    files: Record<string, BundleFile>;
    lists: Record<string, Entry[]>;
    lengths?: Record<string, number>;
    [more: string]: unknown;
  };
  workspaceRoot: string;
  preferences: string | null;
  /** The published versions each library holds on the machine, with the lowest level that runs each. */
  installed?: Record<string, { version: number; requires: number }[]>;
  libraries: {
    name: string;
    root: string;
    hash: string;
    files: number;
    bytes: number;
  }[];
}

/** `GET /verb-bundle/library`'s answer: the files by their path below the library root. */
export interface LibraryFiles {
  name: string;
  hash: string;
  files: Record<string, BundleFile>;
  lengths?: Record<string, number>;
}

/** A file a read outside the bundle asked for: text, a binary's length, or absent. */
export type Fetched = { text: string } | { len: number; binary: true } | null;

export interface ChangeOp {
  kind: StagedOp["kind"] | "read";
  file: string;
  from?: string;
  /** An adopt's staged output: the job and the file below its folder. */
  job?: string;
  out?: string;
  text?: string;
  was?: string | null;
  replace?: boolean;
}

export interface ChangeSet {
  at: string;
  base: string;
  label: string;
  journal?: "join" | "skip";
  /** A new project folder below the workspace, whose files the set writes (an import); `at` is empty then. */
  create?: string;
  /** With `create`: take that folder away (an import that failed before its set). */
  discard?: boolean;
  ops: ChangeOp[];
  reload?: boolean;
  want?: string[];
}

export interface ChangesAnswer {
  status: number;
  body: {
    error?: string;
    rev?: string;
    revBefore?: string;
    changed?: { path: string; kind: string }[];
    graph?: unknown;
    stale?: string[];
    project?: boolean;
    /** The wrong `at`: the project open now (`project` is false and `stale` empty then). */
    open?: string;
    file?: string;
    /** The folder a set creates holds a project already. */
    taken?: boolean;
  };
}

/** A host route's answer: the status and the body, never thrown on a refusal (`{error, code?}`). */
export interface HostAnswer {
  status: number;
  body: Record<string, unknown>;
}

/** `GET /verb-host/job`'s answer, one poll of a long member. */
export type JobState =
  | { state: "running"; fraction?: number; phase?: string }
  | { state: "done"; value: Record<string, unknown> }
  | { state: "failed"; error: string; code?: string };

export interface Remote {
  bundle(at: string): Promise<VerbBundle>;
  library(at: string, name: string, hash: string): Promise<LibraryFiles>;
  /** A file outside the bundle, by its workspace-relative name. */
  file(rel: string): Promise<Fetched>;
  /** A folder's listing outside the bundle, when the server can give one. */
  list?(rel: string): Promise<Entry[] | null>;
  changes(set: ChangeSet): Promise<ChangesAnswer>;
  /** A workspace file's bytes, by its relative name; null when it is not there. */
  bytes?(rel: string): Promise<Uint8Array | null>;
  /** One file's bytes into `dir`, which is made; refused when the name is taken (`POST /upload`). */
  upload?(dir: string, name: string, bytes: Uint8Array): Promise<HostAnswer>;
  /** An entry recycled (`POST /entry/delete`); used to take back an upload a failed import left in a shared folder. */
  remove?(rel: string): Promise<HostAnswer>;
  /** A host member's route, by its path below `/verb-host`: a GET carries `query`, a POST its JSON body. */
  host(
    path: string,
    body: Record<string, unknown> | null,
    query?: string,
  ): Promise<HostAnswer>;
  /** One poll of a job; a forgotten or foreign id is a `failed` state with code `gone`. */
  job(id: string): Promise<JobState>;
  /** A clip's whole-source envelope, from the existing `/waveform` route; null on any failure. */
  envelope?(clip: string): Promise<{ rms: number[]; seconds: number } | null>;
}

export interface HttpOptions {
  base: string;
  headers?: () => Record<string, string>;
  fetch?: typeof fetch;
  /** How long a poll that cannot reach the server is tried again, and between tries; well under the heartbeat. */
  retry?: { forMs: number; everyMs: number };
}

const JSON_HEAD = { "Content-Type": "application/json" };

/** The most one workspace file's bytes a browser reads for an import: `MAX_UPLOAD_BYTES`, which is also what `/upload` takes. */
const MAX_BYTES = 512 * 1024 * 1024;

/** A response's body, refused past `max` before it is held: by its declared length, and again by what streamed. */
async function readBounded(r: Response, max: number): Promise<Uint8Array> {
  const declared = Number(r.headers.get("content-length") ?? 0);
  if (declared > max)
    throw new Error("that file is larger than an import takes");
  const reader = r.body?.getReader();
  if (!reader) return new Uint8Array(await r.arrayBuffer());
  const parts: Uint8Array[] = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > max) {
      await reader.cancel();
      throw new Error("that file is larger than an import takes");
    }
    parts.push(value);
  }
  const out = new Uint8Array(total);
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.byteLength;
  }
  return out;
}

/** The routes outside `/verb-host` a member may reach through `host`. */
const OUTSIDE = ["/bench/save"];

/** The server's routes as a Remote. */
export function httpRemote(o: HttpOptions): Remote {
  const f = o.fetch ?? fetch;
  const head = (): Record<string, string> => o.headers?.() ?? {};
  const get = async <T>(path: string): Promise<T> => {
    const r = await f(`${o.base}${path}`, { headers: head() }).catch(
      (e: Error & { cause?: Error }) => {
        throw new Error(
          `${path.split("?")[0]}: ${e.cause?.message ?? e.message}`,
        );
      },
    );
    const body = (await r.json().catch(() => ({}))) as T & { error?: string };
    if (!r.ok) throw new Error(body.error ?? `${path}: ${r.status}`);
    return body;
  };
  const q = encodeURIComponent;
  return {
    bundle: (at) => get(`/verb-bundle?at=${q(at)}`),
    library: (at, name, hash) =>
      get(`/verb-bundle/library?at=${q(at)}&name=${q(name)}&hash=${q(hash)}`),
    async file(rel) {
      const r = await f(`${o.base}/file?path=${q(rel)}`, { headers: head() });
      if (r.status === 404) return null;
      const body = (await r.json()) as {
        binary?: boolean;
        content?: string;
        error?: string;
      };
      if (!r.ok) throw new Error(body.error ?? `${rel}: ${r.status}`);
      return body.binary
        ? { len: 0, binary: true }
        : { text: body.content ?? "" };
    },
    async bytes(rel) {
      const h = head();
      // A GET sends no headers a media route reads: the identity rides in the query, as the page's own `mediaUrl` sends it.
      const query = new URLSearchParams({
        user: h[USER_HEADER] ?? "",
        workspace: h[WORKSPACE_HEADER] ?? "",
        path: rel,
        token: h[ENGINE_TOKEN_HEADER] ?? "",
      });
      const r = await f(`${o.base}/media?${query.toString()}`, {
        headers: h,
      }).catch((e: Error & { cause?: Error }) => {
        throw new Error(`/media: ${e.cause?.message ?? e.message}`);
      });
      if (r.status === 404) return null;
      if (!r.ok) {
        const why = (await r.json().catch(() => ({}))) as { error?: string };
        throw new Error(why.error ?? `${rel}: ${r.status}`);
      }
      return readBounded(r, MAX_BYTES);
    },
    async upload(dir, name, bytes) {
      const r = await f(`${o.base}/upload?dir=${q(dir)}&name=${q(name)}`, {
        method: "POST",
        headers: { "Content-Type": "application/octet-stream", ...head() },
        body: bytes as unknown as BodyInit,
      }).catch((e: Error & { cause?: Error }) => {
        throw new Error(`/upload: ${e.cause?.message ?? e.message}`);
      });
      return { status: r.status, body: await r.json().catch(() => ({})) };
    },
    async remove(rel) {
      const r = await f(`${o.base}/entry/delete`, {
        method: "POST",
        headers: { ...JSON_HEAD, ...head() },
        body: JSON.stringify({ path: rel }),
      }).catch((e: Error & { cause?: Error }) => {
        throw new Error(`/entry/delete: ${e.cause?.message ?? e.message}`);
      });
      return { status: r.status, body: await r.json().catch(() => ({})) };
    },
    async changes(set) {
      const r = await f(`${o.base}/changes`, {
        method: "POST",
        headers: { "Content-Type": "application/json", ...head() },
        body: JSON.stringify(set),
      }).catch((e: Error & { cause?: Error }) => {
        throw new Error(`/changes: ${e.cause?.message ?? e.message}`);
      });
      return { status: r.status, body: await r.json().catch(() => ({})) };
    },
    async host(path, body, query = "") {
      // A path with a leading slash is a route outside `/verb-host`, and only the bench's save is one.
      if (path.startsWith("/") && !OUTSIDE.includes(path))
        throw new Error(`${path}: not a host route`);
      const url = `${o.base}${path.startsWith("/") ? "" : "/verb-host/"}${path}${query ? `?${query}` : ""}`;
      const r = await f(url, {
        method: body === null ? "GET" : "POST",
        headers: { ...(body === null ? {} : JSON_HEAD), ...head() },
        ...(body === null ? {} : { body: JSON.stringify(body) }),
      }).catch((e: Error & { cause?: Error }) => {
        throw new Error(
          `${url}: ${e.cause?.message ?? e.message}`.replace(o.base, ""),
        );
      });
      return { status: r.status, body: await r.json().catch(() => ({})) };
    },
    async envelope(clip) {
      const url = `${o.base}/waveform?clip=${q(clip)}&buckets=2000&whole=1`;
      const r = await f(url, { headers: head() }).catch(() => undefined);
      if (!r?.ok) return null;
      const body = (await r.json().catch(() => null)) as {
        rms?: unknown;
        seconds?: unknown;
      } | null;
      return Array.isArray(body?.rms) && typeof body?.seconds === "number"
        ? { rms: body.rms as number[], seconds: body.seconds }
        : null;
    },
    async job(id) {
      const { forMs, everyMs } = o.retry ?? { forMs: 30_000, everyMs: 1000 };
      const until = Date.now() + forMs;
      let r: Response;
      for (;;) {
        let lost: Error | undefined;
        r = await f(`${o.base}/verb-host/job?id=${q(id)}`, {
          headers: head(),
        }).catch((e: Error & { cause?: Error }) => {
          lost = new Error(`/verb-host/job: ${e.cause?.message ?? e.message}`);
          return undefined as unknown as Response;
        });
        // A restart or a proxy's 502 is not the job's end: the same poll is asked again for a while.
        const transient =
          lost !== undefined || [502, 503, 504].includes(r.status);
        if (!transient) break;
        if (Date.now() >= until) {
          if (lost) throw lost;
          break;
        }
        await new Promise((ok) => setTimeout(ok, everyMs));
      }
      const body = (await r.json().catch(() => ({}))) as Record<
        string,
        unknown
      >;
      if (r.status === 404)
        return {
          state: "failed",
          error: String(body.error ?? "the job is forgotten"),
          code: "gone",
        };
      if (!r.ok)
        return {
          state: "failed",
          error: String(body.error ?? `/verb-host/job: ${r.status}`),
          ...(typeof body.code === "string" ? { code: body.code } : {}),
        };
      return body as JobState;
    },
  };
}
