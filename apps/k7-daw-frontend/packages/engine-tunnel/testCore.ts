// testCore.ts — where the tunnel and registry suites find the hub they test: the Rust testbed ENGINE_CORE_BIN
// names (`k7-engine-testbed`), a fresh process of it per test, driven over its routes and its `/__engine-test`
// hooks (apps/k7-daw-backend/docs/engine.md). Without it the suites skip, unless ENGINE_CORE_REQUIRED=1 (CI).
// The one thing a reader would get wrong: a hook reaches the core directly, and stands for nothing a page can do.

import { type ChildProcess, spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import {
  type AddressInfo,
  connect,
  createServer as createTcp,
  type Socket,
} from "node:net";
import { networkInterfaces, tmpdir } from "node:os";
import { join } from "node:path";
import { describe } from "vitest";
import { WebSocket } from "ws";
import type { TunnelChannel } from "./channel.ts";
import { type ChannelKind, TUNNEL_PATH, type TunnelRoot } from "./frames.ts";

/** The Rust testbed's binary; unset, the suites that need it skip. */
export const RUST_CORE = process.env.ENGINE_CORE_BIN ?? "";

// CI builds the testbed first: there a suite that would skip is a failure.
if (!RUST_CORE && process.env.ENGINE_CORE_REQUIRED === "1")
  throw new Error(
    "ENGINE_CORE_REQUIRED=1, but ENGINE_CORE_BIN names no testbed",
  );

/** A suite of the engine core: run against the testbed, skipped without one. */
export const describeCore = describe.skipIf(!RUST_CORE);

/** The secret the testbed's sessions are signed with, and its front secret. */
export const TESTBED_SECRET = "engine-testbed-secret";

/** A running testbed: its port, what it logged (prefix aside), and its hooks. */
export interface Testbed {
  port: number;
  base: string;
  lines: string[];
  hook<T = unknown>(op: string, body?: Record<string, unknown>): Promise<T>;
  stop(): Promise<void>;
}

/** Starts the Rust testbed with `env` over its defaults: sessions known, the bindings in memory. */
export async function startTestbed(
  env: Record<string, string> = {},
): Promise<Testbed> {
  const lines: string[] = [];
  const child: ChildProcess = spawn(RUST_CORE, [], {
    env: {
      ...process.env,
      K7_ENGINE_TESTBED: "1",
      K7_FRONT_SECRET: TESTBED_SECRET,
      K7_FRONT_LISTEN: "127.0.0.1:0",
      K7_SESSION_TOKEN: TESTBED_SECRET,
      STUDIO_AUTH_DIR: mkdtempSync(join(tmpdir(), "testbed-auth-")),
      K7_ENGINE_TEST_MEMORY: "1",
      ...env,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const read = (data: Buffer) => {
    for (const l of data.toString("utf8").split(/\r?\n/))
      if (l) lines.push(l.replace(/^\[registry\] |^tunnel \| /, ""));
  };
  child.stdout?.on("data", read);
  child.stderr?.on("data", read);
  const port = await new Promise<number>((resolve, reject) => {
    const timer = setTimeout(
      () =>
        reject(new Error(`the testbed did not start: ${lines.join(" | ")}`)),
      10_000,
    );
    const look = () => {
      const m = lines
        .map((l) => /k7-front on http:\/\/[^:]+:(\d+)/.exec(l))
        .find(Boolean);
      if (m) {
        clearTimeout(timer);
        resolve(Number(m[1]));
      }
    };
    child.stdout?.on("data", look);
    child.once("exit", (c) =>
      reject(new Error(`the testbed exited ${c}: ${lines.join(" | ")}`)),
    );
  });
  const base = `http://127.0.0.1:${port}`;
  return {
    port,
    base,
    lines,
    async hook<T>(op: string, body: Record<string, unknown> = {}) {
      const r = await fetch(`${base}/__engine-test/${op}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
        // A hung testbed fails here, naming the hook, instead of eating the caller's whole test timeout.
        signal: AbortSignal.timeout(20_000),
      });
      return (await r.json()) as T;
    },
    stop: () =>
      new Promise<void>((resolve) => {
        if (child.exitCode !== null) return resolve();
        child.once("exit", () => resolve());
        child.kill();
      }),
  };
}

/** Waits for `ok`, which may ask the hub asynchronously, or throws after `ms`. */
export async function until(
  ok: () => boolean | Promise<boolean>,
  ms = 3000,
): Promise<void> {
  const end = Date.now() + ms;
  while (!(await ok())) {
    if (Date.now() > end) throw new Error("timed out");
    await new Promise((r) => setTimeout(r, 10));
  }
}

/** Set where a firewall drops a LAN peer's connection to the unsigned testbed: its tests skip (docs/release.md, macOS). */
export const NO_LAN_PEER = process.env.K7_TEST_NO_LAN_PEER === "1";

/** A non-loopback IPv4 of this machine, to dial a server as a peer that is not this machine's loopback. */
export function lanAddress(): string | undefined {
  for (const list of Object.values(networkInterfaces()))
    for (const a of list ?? [])
      if (a.family === "IPv4" && !a.internal) return a.address;
  return undefined;
}

/** What a channel is to a suite: a socket on a bound path of the hub, shaped as the agent's channel. */
export type Chan = TunnelChannel;

/** How a suite reaches a bound path as some peer: a URL host and the socket's options. */
export interface Dial {
  host: string;
  headers?: Record<string, string>;
  localAddress?: string;
}

/** The hub under test, its answers asynchronous whichever implementation gives them. */
export interface HubTarget {
  port: number;
  /** What an agent dials: the hub, or a cut-able relay to it. */
  agentUrl: string;
  lines: string[];
  has(id: string): Promise<boolean>;
  openReady(
    id: string,
    roots?: TunnelRoot[],
    timeoutMs?: number,
    kind?: ChannelKind,
  ): Promise<Chan | null>;
  bindLocal(
    id: string,
    roots: TunnelRoot[],
    kind?: ChannelKind,
    opts?: { synced?: boolean },
  ): Promise<string>;
  unbindLocal(path: string): Promise<void>;
  signal(id: string, sdp: string): Promise<string>;
  /** Drops every tunnel's socket as a network would, no close frame. */
  cut(): void;
  /** How to dial a bound path as `peer`, the address the front would vouch for. */
  as(peer: string): Dial;
  stop(): Promise<void>;
}

/** What one tunnel may upload. */
export interface UploadLimits {
  maxOpen: number;
  maxBytes: number;
  bytesPerHour: number;
  idleMs: number;
}

export interface HubOptions {
  /** Who may hold a tunnel: anyone, or one id with its token (and secret when given). */
  verify?: "all" | { id: string; token: string; secret?: string };
  minProtocol?: number;
  /** What one tunnel may upload. */
  uploads?: Partial<UploadLimits>;
}

/** A TCP relay whose sockets `cut` destroys, for a hub in another process. */
async function cuttable(
  port: number,
): Promise<{ url: string; cut(): void; close(): void }> {
  const pairs: Socket[] = [];
  const relay = createTcp((a) => {
    const b = connect(port, "127.0.0.1");
    pairs.push(a, b);
    a.pipe(b);
    b.pipe(a);
    a.on("error", () => b.destroy());
    b.on("error", () => a.destroy());
    a.on("close", () => b.destroy());
    b.on("close", () => a.destroy());
  });
  await new Promise<void>((r) => relay.listen(0, "127.0.0.1", () => r()));
  return {
    url: `http://127.0.0.1:${(relay.address() as AddressInfo).port}`,
    cut: () => {
      for (const s of pairs.splice(0)) s.destroy();
    },
    close: () => {
      for (const s of pairs.splice(0)) s.destroy();
      relay.close();
    },
  };
}

/** A socket on `path`, resolved once open, or null when refused. */
function dialPath(base: string, path: string): Promise<WebSocket | null> {
  return new Promise((resolve) => {
    const ws = new WebSocket(`${base.replace("http", "ws")}${path}`);
    ws.once("open", () => resolve(ws));
    ws.once("unexpected-response", () => resolve(null));
    ws.once("error", () => resolve(null));
  });
}

/** The hub the suite tests, started. */
export async function startHub(opts: HubOptions = {}): Promise<HubTarget> {
  return rustHub(opts);
}

async function rustHub(opts: HubOptions): Promise<HubTarget> {
  const lan = lanAddress();
  const bed = await startTestbed({
    K7_FRONT_LISTEN: "0.0.0.0:0",
    ...(opts.minProtocol
      ? { K7_ENGINE_TEST_MIN_PROTOCOL: String(opts.minProtocol) }
      : {}),
    ...(opts.uploads
      ? {
          K7_ENGINE_TEST_UPLOAD_OPEN: String(opts.uploads.maxOpen ?? 16),
          K7_ENGINE_TEST_UPLOAD_BYTES: String(opts.uploads.maxBytes ?? 2 ** 31),
          K7_ENGINE_TEST_UPLOAD_HOUR: String(
            opts.uploads.bytesPerHour ?? 20 * 2 ** 30,
          ),
          K7_ENGINE_TEST_UPLOAD_IDLE_MS: String(opts.uploads.idleMs ?? 60_000),
        }
      : {}),
  });
  const v = opts.verify ?? "all";
  await bed.hook(
    "verify",
    v === "all" ? { mode: "all" } : { mode: "fixed", ...v },
  );
  const relay = await cuttable(bed.port);
  return {
    port: bed.port,
    agentUrl: relay.url,
    lines: bed.lines,
    has: (id) => bed.hook<boolean>("hubHas", { id }),
    async openReady(id, roots = [], _timeoutMs = 5000, kind = "engine") {
      // A one-shot binding, as the seam's callers open a channel.
      const path = await bed.hook<string>("hubBind", {
        engineId: id,
        roots,
        kind,
      });
      const ws = await dialPath(bed.base, path);
      await bed.hook("hubUnbind", { path });
      return ws as unknown as Chan | null;
    },
    bindLocal: (id, roots, kind = "engine", o = {}) =>
      bed.hook<string>("hubBind", {
        engineId: id,
        roots,
        kind,
        synced: !!o.synced,
      }),
    unbindLocal: async (path) => void (await bed.hook("hubUnbind", { path })),
    async signal(id, sdp) {
      const r = await bed.hook<{ sdp?: string; error?: string }>("hubSignal", {
        id,
        sdp,
      });
      if (r.sdp === undefined) throw new Error(r.error);
      return r.sdp;
    },
    cut: () => relay.cut(),
    // The Rust front IS the edge: a peer that is not loopback is a socket from another address.
    as: (peer) =>
      /^(::ffff:)?127\./.test(peer) || peer === "::1"
        ? { host: "127.0.0.1" }
        : { host: lan ?? "127.0.0.1", localAddress: lan },
    stop: async () => {
      relay.close();
      await bed.stop();
    },
  };
}

/** A reply the hub sent a bare agent: a control message, or a downloaded chunk. */
type Heard = { t: string; req?: number; ch?: number; [k: string]: unknown } & {
  payload?: Buffer;
};

/** A bare supervisor speaking the tunnel's frames by hand, so a suite can send what no agent would. */
export interface BareAgent {
  ws: WebSocket;
  /** The next channel the hub opens, answered `opened`. */
  opened(): Promise<{ ch: number; roots: TunnelRoot[] }>;
  stat(ch: number, paths: string[]): Promise<{ path: string; kind: string }[]>;
  /** A file's bytes, or the hub's refusal. */
  get(ch: number, path: string): Promise<Buffer | { error: string }>;
  /** Sends `chunks` as one upload, the last one marked; resolves the hub's reply. */
  put(
    ch: number,
    path: string,
    chunks: Buffer[],
    req?: number,
  ): Promise<{ error?: string; stat?: unknown }>;
  /** One raw upload chunk, its reply not awaited. */
  putChunk(
    ch: number,
    req: number,
    path: string,
    chunk: Buffer,
    last: boolean,
  ): void;
  replies(t: string): Heard[];
  close(): void;
}

const binary = (header: object, payload: Buffer): Buffer => {
  const head = Buffer.from(JSON.stringify(header));
  const len = Buffer.alloc(4);
  len.writeUInt32BE(head.length);
  return Buffer.concat([len, head, payload]);
};

/** A bare supervisor for engine `id`, dialled into `url`. */
export async function bareAgent(
  url: string,
  id = "e1",
  token = "t",
): Promise<BareAgent> {
  const ws = new WebSocket(
    `${url.replace("http", "ws")}${TUNNEL_PATH}?id=${id}`,
    {
      headers: { "x-studio-engine-token": token },
    },
  );
  const heard: Heard[] = [];
  const waiters: (() => void)[] = [];
  ws.on("message", (d: Buffer, isBinary) => {
    if (isBinary) {
      const len = d.readUInt32BE(0);
      heard.push({
        ...JSON.parse(d.subarray(4, 4 + len).toString()),
        payload: d.subarray(4 + len),
      });
    } else heard.push(JSON.parse(d.toString()));
    for (const w of waiters.splice(0)) w();
  });
  await new Promise((r, j) => {
    ws.once("open", r);
    ws.once("unexpected-response", (_q, res) =>
      j(new Error(`refused ${res.statusCode}`)),
    );
  });
  const next = async <T>(pick: () => T | undefined, ms = 5000): Promise<T> => {
    const end = Date.now() + ms;
    for (;;) {
      const v = pick();
      if (v !== undefined) return v;
      if (Date.now() > end) throw new Error("the hub did not answer");
      await new Promise<void>((r) => {
        waiters.push(r);
        setTimeout(r, 50);
      });
    }
  };
  let req = 1000;
  let seenOpens = 0;
  return {
    ws,
    async opened() {
      const m = await next(
        () => heard.filter((h) => h.t === "open")[seenOpens],
      );
      seenOpens++;
      ws.send(JSON.stringify({ t: "opened", ch: m.ch }));
      return { ch: m.ch as number, roots: m.roots as TunnelRoot[] };
    },
    async stat(ch, paths) {
      const r = req++;
      ws.send(JSON.stringify({ t: "stat", req: r, ch, paths }));
      const m = await next(() =>
        heard.find((h) => h.t === "stat-reply" && h.req === r),
      );
      return m.entries as { path: string; kind: string }[];
    },
    async get(ch, path) {
      const r = req++;
      ws.send(JSON.stringify({ t: "get", req: r, ch, path }));
      return next(() => {
        const err = heard.find((h) => h.t === "get-error" && h.req === r);
        if (err) return { error: String(err.error) };
        const chunks = heard.filter((h) => h.t === "chunk" && h.req === r);
        if (!chunks.some((c) => c.last)) return undefined;
        return Buffer.concat(chunks.map((c) => c.payload ?? Buffer.alloc(0)));
      });
    },
    putChunk(ch, r, path, chunk, last) {
      ws.send(binary({ t: "put", req: r, ch, path, last }, chunk));
    },
    async put(ch, path, chunks, r = req++) {
      chunks.forEach((c, i) => {
        ws.send(
          binary(
            { t: "put", req: r, ch, path, last: i === chunks.length - 1 },
            c,
          ),
        );
      });
      const m = await next(() =>
        heard.find((h) => h.t === "put-reply" && h.req === r),
      );
      return { error: m.error as string | undefined, stat: m.stat };
    },
    replies: (t) => heard.filter((h) => h.t === t),
    close: () => ws.terminate(),
  };
}
