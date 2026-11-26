// standInEngine.ts — an engine the conformance suite plays itself: a WebSocket server speaking the engine's wire
// (every frame kept, `list-sessions` answered for the sessions it holds), listed at the instance by a heartbeat and
// reached through the supervisor's own tunnel agent (its `tunnel_harness` binary), as a machine's supervisor reaches it.
// What a reader gets wrong: it plays no sound and opens no audio device, so it runs beside a person's own engine.

import { type ChildProcess, spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { expect, request as playwright } from "@playwright/test";
import { type WebSocket, WebSocketServer } from "ws";
import { base, signedIn } from "./instance.ts";
import type { SessionUser } from "./session.ts";

// biome-ignore lint/suspicious/noExplicitAny: a frame is read as the engine reads it, by its fields.
export type Frame = Record<string, any>;

/** The agent binary `$TUNNEL_HARNESS_BIN` names, else one built under the bed's target folder or the backend's. */
function harnessBin(): string {
  const named = process.env.TUNNEL_HARNESS_BIN;
  if (named) {
    if (existsSync(named)) return named;
    throw new Error(`TUNNEL_HARNESS_BIN names no file: ${named}`);
  }
  const backend = fileURLToPath(
    new URL("../../apps/k7-daw-backend/target", import.meta.url),
  );
  for (const target of [process.env.K7_FRONT_TARGET_DIR, backend])
    for (const profile of ["release", "debug"])
      for (const name of ["tunnel_harness", "tunnel_harness.exe"]) {
        const p = join(target ?? "", profile, name);
        if (target && existsSync(p)) return p;
      }
  throw new Error(
    "no tunnel_harness: build it (cargo build -p k7-supervisor --bin tunnel_harness, in apps/k7-daw-backend) or name it in TUNNEL_HARNESS_BIN",
  );
}

/** One frame the engine heard: on which socket, when, and the frame. */
export interface Heard {
  socket: number;
  at: number;
  frame: Frame;
}

export class StandInEngine {
  readonly id = `conformance-engine-${randomBytes(5).toString("hex")}`;
  readonly token = randomBytes(8).toString("hex");
  readonly heard: Heard[] = [];
  /** The file lists the account's prefetch asked this machine for, oldest first. */
  readonly prefetched: string[][] = [];
  /** The sessions a project was sent to and not dropped since: what `list-sessions` answers loaded. */
  readonly loaded = new Set<string>();
  private readonly sockets = new Map<number, WebSocket>();
  private readonly sessionOf = new Map<number, string>();
  private server!: WebSocketServer;
  private agent?: ChildProcess;
  private up = false;
  private beat?: NodeJS.Timeout;
  private cache = "";
  /** When set, `plugin-scan` and `list-plugins` are answered with these frames, as a scanning engine does. */
  scan: { frames: Frame[]; plugins: Frame[] } | null = null;
  /** When set, `list-audio-devices` is answered with these, and `arm` with the lane armed. */
  devices: Frame[] | null = null;
  private secret = "";
  port = 0;

  /** Listening, listed by a heartbeat, its tunnel up, and claimed by `user` so the account's files bind it. */
  static async start(user: SessionUser): Promise<StandInEngine> {
    const e = new StandInEngine();
    await e.listen();
    await e.heartbeat();
    e.beat = setInterval(() => void e.heartbeat().catch(() => {}), 5_000);
    e.cache = mkdtempSync(join(tmpdir(), "k7-stand-in-"));
    // The agent exits when its stdin closes; its log and the file lists it was asked to prefetch come on stderr.
    e.agent = spawn(
      harnessBin(),
      [base, e.id, e.token, String(e.port), e.cache],
      { stdio: ["pipe", "ignore", "pipe"] },
    );
    if (e.agent.stderr)
      createInterface({ input: e.agent.stderr }).on("line", (l) => e.said(l));
    await expect.poll(() => e.up, { timeout: 20_000 }).toBe(true);
    await e.claim(user);
    return e;
  }

  /** One line of the agent's stderr: the tunnel's state, or a prefetch it was asked for. */
  private said(line: string): void {
    if (line.includes("tunnel | up to ")) this.up = true;
    else if (line.includes("tunnel | down from ")) this.up = false;
    else if (line.startsWith("prefetch ")) {
      try {
        this.prefetched.push(JSON.parse(line.slice("prefetch ".length)));
      } catch {
        // A line that is no file list is the agent's own noise.
      }
    }
  }

  private listen(): Promise<void> {
    this.server = new WebSocketServer({ port: 0, host: "127.0.0.1" });
    let n = 0;
    this.server.on("connection", (ws) => {
      const socket = ++n;
      this.sockets.set(socket, ws);
      ws.on("close", () => this.sockets.delete(socket));
      ws.on("message", (data) => this.hear(socket, String(data)));
    });
    return new Promise((ok) =>
      this.server.on("listening", () => {
        this.port = (this.server.address() as { port: number }).port;
        ok();
      }),
    );
  }

  private hear(socket: number, text: string): void {
    let frame: Frame;
    try {
      frame = JSON.parse(text);
    } catch {
      return;
    }
    this.heard.push({ socket, at: Date.now(), frame });
    const session = typeof frame.session === "string" ? frame.session : "";
    if (session) this.sessionOf.set(socket, session);
    if (frame.type === "project") this.loaded.add(session);
    if (frame.type === "drop-session") this.loaded.delete(frame.session);
    if (this.scan && frame.type === "plugin-scan")
      for (const f of this.scan.frames) this.reply(socket, f);
    if (this.scan && frame.type === "list-plugins")
      this.reply(socket, { type: "plugins", plugins: this.scan.plugins });
    // The catalogue first, then the end of the scan, as the engine sends them.
    if (this.scan && frame.type === "rescan-plugins") {
      this.reply(socket, { type: "plugins", plugins: this.scan.plugins });
      this.reply(socket, {
        type: "plugin-scan",
        scanning: false,
        done: 0,
        total: 0,
        version: 1,
      });
    }
    if (this.devices && frame.type === "list-audio-devices")
      this.reply(socket, { type: "audio-devices", devices: this.devices });
    if (this.devices && frame.type === "arm")
      this.reply(socket, {
        type: "armed",
        track: frame.track,
        armed: frame.armed ?? true,
      });
    if (frame.type === "list-sessions")
      this.reply(socket, {
        type: "sessions",
        sessions: [...this.loaded].map((id) => ({
          id,
          project: "stand-in",
          loaded: true,
          playing: false,
          tracks: 1,
          listeners: 1,
        })),
      });
  }

  private reply(socket: number, frame: Frame): void {
    this.sockets.get(socket)?.send(JSON.stringify(frame));
  }

  /** A frame from the engine on the socket of `session`, as the engine stamps its session's news. */
  say(session: string, frame: Frame): void {
    for (const [socket, s] of this.sessionOf)
      if (s === session) this.reply(socket, { session, ...frame });
  }

  /** A frame from the engine on every socket, as a scan's progress is broadcast. */
  broadcast(frame: Frame): void {
    for (const socket of this.sockets.keys()) this.reply(socket, frame);
  }

  /** The agent's cache made unusable until `mendCache`: a copy to the machine then fails. */
  breakCache(): void {
    renameSync(this.cache, `${this.cache}.aside`);
    writeFileSync(this.cache, "");
  }

  /** The cache as it was before `breakCache`. */
  mendCache(): void {
    rmSync(this.cache, { force: true });
    renameSync(`${this.cache}.aside`, this.cache);
  }

  /** The frames one session's sockets carried, oldest first. */
  of(session: string): Frame[] {
    return this.heard
      .filter((h) => h.frame.session === session)
      .map((h) => h.frame);
  }

  /** The engine's heartbeat, as a supervisor sends it every few seconds. */
  private async heartbeat(): Promise<void> {
    const ctx = await playwright.newContext({
      baseURL: base,
      ignoreHTTPSErrors: true,
    });
    const r = await ctx.post("/api/registry/heartbeat", {
      data: {
        id: this.id,
        token: this.token,
        hostname: "conformance-stand-in",
        machineFingerprint: "stand-in",
        addresses: { loopback: "127.0.0.1", lanIps: [], port: this.port },
        performance: {
          score: 1,
          cores: 0,
          singleThread: 1,
          diskWriteMBps: null,
          fsyncPerSec: null,
          memoryGB: 1,
        },
      },
    });
    expect(r.status()).toBe(200);
    await ctx.dispose();
  }

  /** The account takes the engine, as a page selecting it does; renewed by the heartbeat's own clock. */
  private async claim(user: SessionUser): Promise<void> {
    const ctx = await playwright.newContext({
      baseURL: base,
      ignoreHTTPSErrors: true,
    });
    const r = await ctx.post("/api/engine/select", {
      data: {
        id: this.id,
        token: this.token,
        project: null,
        ...(this.secret ? { secret: this.secret } : {}),
      },
      headers: signedIn(user),
    });
    expect(r.status()).toBe(200);
    this.secret = (await r.json()).secret;
    await ctx.dispose();
    const renew = setInterval(
      () => void this.claim(user).catch(() => {}),
      30_000,
    );
    renew.unref();
    if (this.renew) clearInterval(this.renew);
    this.renew = renew;
  }
  private renew?: NodeJS.Timeout;

  /** Everything let go: the claim, the listing, the tunnel, the socket, the agent's cache. */
  async stop(user: SessionUser): Promise<void> {
    if (this.beat) clearInterval(this.beat);
    if (this.renew) clearInterval(this.renew);
    const ctx = await playwright.newContext({
      baseURL: base,
      ignoreHTTPSErrors: true,
    });
    await ctx.post("/api/engine/release", {
      data: { id: this.id, secret: this.secret },
      headers: signedIn(user),
    });
    await ctx.post("/api/registry/deregister", {
      data: { id: this.id, token: this.token },
    });
    await ctx.dispose();
    this.agent?.stdin?.end();
    this.agent?.kill();
    for (const ws of this.sockets.values()) ws.terminate();
    await new Promise((ok) => this.server.close(() => ok(null)));
    rmSync(this.cache, { recursive: true, force: true });
    rmSync(`${this.cache}.aside`, { recursive: true, force: true });
  }
}
