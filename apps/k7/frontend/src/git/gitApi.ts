// gitApi.ts — the calls to `/api/files/git/*` (apps/k7/backend-rs/docs/files/git.md): one transport, and a
// refusal that keeps its status, its code and what else the answer said, so a screen branches on the code.

import { HttpError } from "@studio/http-client/call";
import { m } from "../paraglide/messages.js";
import type {
  CommitInfo,
  GitState,
  MergeView,
  PullState,
  StatusEntry,
} from "./gitTypes";
import type { RoamAnswer } from "./roamState";

/** Where the file service is and how to open its door — an account's, or the open project's. */
export interface GitTransport {
  base: string;
  headers: () => Record<string, string>;
}

/** A refused route: `code` is the contract's word, `body` what else the answer said (`fingerprint`, `expected`...). */
export class GitError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code: string,
    readonly body: Record<string, unknown>,
  ) {
    super(message);
  }
}

/** One sentence for a person, from any failure the routes can give. */
export function explain(e: unknown): string {
  if (e instanceof GitError) {
    if (e.code === "behind") return m["git.pullFirst"]();
    if (e.code === "uncommitted") return m["git.commitFirst"]();
    if (e.code === "busy") return m["git.busy"]();
    if (e.code === "merge-pending") return m["git.mergePending"]();
    if (e.code === "device-only") return m["git.deviceOnly"]();
    if (e.code === "ssh-only") return m["git.sshOnly"]();
    return e.message;
  }
  if (e instanceof HttpError) return e.message;
  return e instanceof Error ? e.message : String(e);
}

async function call<T>(
  t: GitTransport,
  path: string,
  init: { json?: unknown } = {},
): Promise<T> {
  const headers = t.headers();
  if (init.json !== undefined) headers["content-type"] = "application/json";
  const res = await fetch(`${t.base}/git/${path}`, {
    method: init.json === undefined ? "GET" : "POST",
    headers,
    body: init.json === undefined ? undefined : JSON.stringify(init.json),
  });
  if (res.ok) return (await res.json()) as T;
  const body = (await res.json().catch(() => ({}))) as Record<string, unknown>;
  const said =
    typeof body.error === "string" ? body.error : `git ${res.status}`;
  throw new GitError(said, res.status, String(body.code ?? ""), body);
}

/** Where a workspace points: an ssh address, and the host key the person confirmed (absent at first contact). */
export interface Remote {
  url: string;
  hostKey?: string;
}

const enc = encodeURIComponent;

export const git = {
  state: (t: GitTransport, ws: string) =>
    call<GitState>(t, `state?workspace=${enc(ws)}`),
  status: (t: GitTransport, ws: string) =>
    call<{ entries: StatusEntry[] }>(t, `status?workspace=${enc(ws)}`),
  log: (t: GitTransport, ws: string, limit: number, before?: string) =>
    call<{ commits: CommitInfo[] }>(
      t,
      `log?workspace=${enc(ws)}&limit=${limit}${before ? `&before=${enc(before)}` : ""}`,
    ),
  connect: (t: GitTransport, workspace: string, r: Remote) =>
    call<{ ok: boolean; remote: string; lfsWarning?: string }>(t, "connect", {
      json: { workspace, ...r },
    }),
  roam: (t: GitTransport) => call<RoamAnswer>(t, "roam"),
  clone: (t: GitTransport, name: string, r: Remote) =>
    call<{ ok: boolean; name: string }>(t, "clone", { json: { name, ...r } }),
  commit: (
    t: GitTransport,
    workspace: string,
    message: string,
    exclude: string[] = [],
  ) =>
    call<{ ok: boolean; id: string }>(t, "commit", {
      json: { workspace, message, exclude },
    }),
  push: (t: GitTransport, workspace: string) =>
    call<{ ok: boolean }>(t, "push", { json: { workspace } }),
  pull: (t: GitTransport, workspace: string) =>
    call<{ ok: boolean; state: PullState }>(t, "pull", {
      json: { workspace },
    }),
  merge: (t: GitTransport, ws: string) =>
    call<MergeView>(t, `merge?workspace=${enc(ws)}`),
  resolve: (
    t: GitTransport,
    workspace: string,
    choices: { id: string; choice: string }[],
  ) =>
    call<{ ok: boolean; id: string }>(t, "merge/resolve", {
      json: { workspace, choices },
    }),
  abort: (t: GitTransport, workspace: string) =>
    call<{ ok: boolean }>(t, "merge/abort", { json: { workspace } }),
  identity: (t: GitTransport) => call<Identity>(t, "identity"),
  setIdentity: (t: GitTransport, id: Identity) =>
    call<{ ok: boolean }>(t, "identity", { json: id }),
  /** The device's public SSH key line, made on first use; the private half never leaves the device. */
  key: (t: GitTransport) =>
    call<{ publicKey: string; synced?: boolean }>(t, "key"),
  /** Takes a server off this account's trusted list, so the next contact shows its key again. */
  forgetHost: (t: GitTransport, target: { host: string; port?: number }) =>
    call<{ ok: boolean; forgotten: boolean }>(t, "forget-host", {
      json: target,
    }),
  /** Says the page is in front (or left), which drives the fetch. */
  focus: (t: GitTransport, workspace: string, front: boolean, opened = false) =>
    call<{ ok: boolean }>(t, "focus", { json: { workspace, front, opened } }),
};

export interface Identity {
  name: string;
  email: string;
}
