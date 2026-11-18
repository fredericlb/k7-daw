// syncHistory.ts — the workspace's history, from the device's routes (docs/files/history.md).
// It is the ordered log, not the undo journal; what a viewer may do with it is historySource.ts.

import { projectTransport } from "./git/transports";

interface SyncCheckpoint {
  seq: number;
  name: string;
  ts: number;
  by: string;
}

interface SyncEntry {
  seq: number;
  ts: number;
  replica: string;
  label: string;
  files: string[];
  parked: number;
}

export interface SyncChange {
  file: string;
  before: string | null;
  after: string | null;
  binary?: true;
}

export interface SyncHistory {
  head: number;
  checkpoints: SyncCheckpoint[];
  entries: SyncEntry[];
  /** The device's own reason its history could not start; the page is then empty. */
  error?: string;
}

/** A restore's answer: the entry it ordered, and the binary files it left as they are. */
export interface RestoreAnswer {
  entry: SyncEntry | null;
  kept?: string[];
}

/** A refused history route: `status` and `code` are the contract's, the message is its sentence. */
class HistoryError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code: string,
  ) {
    super(message);
  }
}

async function call<T>(
  url: string,
  headers: Record<string, string>,
  body?: unknown,
): Promise<T> {
  const res = await fetch(url, {
    credentials: "include",
    headers:
      body === undefined
        ? headers
        : { ...headers, "content-type": "application/json" },
    ...(body === undefined
      ? {}
      : { method: "POST", body: JSON.stringify(body) }),
  });
  const json = (await res.json().catch(() => ({}))) as T & {
    error?: string;
    code?: string;
  };
  if (!res.ok)
    throw new HistoryError(
      json.error ?? `history: ${res.status}`,
      res.status,
      json.code ?? "",
    );
  return json;
}

export interface HistoryClient {
  list: (workspace: string, limit?: number) => Promise<SyncHistory>;
  checkpoint: (workspace: string, name: string) => Promise<SyncCheckpoint>;
  diff: (workspace: string, from: number, to: number) => Promise<SyncChange[]>;
  restore: (workspace: string, seq: number) => Promise<RestoreAnswer>;
}

const ws = (workspace: string) => encodeURIComponent(workspace);

/** The device's routes, on the open project's file service (`/history/*`, apps/k7/backend-rs/docs/files/history.md). */
function deviceCall<T>(path: string, body?: unknown): Promise<T> {
  const t = projectTransport();
  if (!t) return Promise.reject(new HistoryError("no file service", 0, ""));
  return call<T>(`${t.base}/history/${path}`, t.headers(), body);
}

const device: HistoryClient = {
  list: (workspace, limit = 50) =>
    deviceCall(`page?workspace=${ws(workspace)}&limit=${limit}`),
  checkpoint: (workspace, name) =>
    deviceCall("checkpoint", { workspace, name }),
  diff: (workspace, from, to) =>
    deviceCall<{ files: SyncChange[] }>(
      `diff?workspace=${ws(workspace)}&from=${from}&to=${to}`,
    ).then((d) => d.files),
  restore: (workspace, seq) => deviceCall("restore", { workspace, seq }),
};

export const historyClient: HistoryClient = device;
