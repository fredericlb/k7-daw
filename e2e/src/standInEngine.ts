// standInEngine.ts — an engine the conformance suite plays itself: a WebSocket server speaking the engine's wire
// (every frame kept, `list-sessions` answered for the sessions it holds), listed at the instance by a heartbeat and
// reached through the reference tunnel agent (`@studio/engine-tunnel/agent`), as a machine's supervisor reaches it.
// What a reader gets wrong: it plays no sound and opens no audio device, so it runs beside a person's own engine.

import { randomBytes } from "node:crypto";
import { mkdtempSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, request as playwright } from "@playwright/test";
import { TunnelAgent } from "@studio/engine-tunnel/agent";
import type { SessionUser } from "@studio/oidc";
import { type WebSocket, WebSocketServer } from "ws";
import { base, signedIn } from "./instance.ts";

// biome-ignore lint/suspicious/noExplicitAny: a frame is read as the engine reads it, by its fields.
export type Frame = Record<string, any>;

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
  private agent!: TunnelAgent;
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
    e.agent = new TunnelAgent({
      registryUrl: base,
      id: e.id,
      token: e.token,
      enginePort: e.port,
      cacheDir: e.cache,
      log: () => {},
      jobs: {
        prefetch: (req, emit) => {
          const files = Array.isArray(req.files) ? (req.files as string[]) : [];
          e.prefetched.push(files);
          emit({ type: "prefetched", count: files.length });
        },
      },
    });
    e.agent.start();
    await expect.poll(() => e.agent.connected, { timeout: 20_000 }).toBe(true);
    await e.claim(user);
    return e;
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
    this.agent?.stop();
    for (const ws of this.sockets.values()) ws.terminate();
    await new Promise((ok) => this.server.close(() => ok(null)));
    rmSync(this.cache, { recursive: true, force: true });
    rmSync(`${this.cache}.aside`, { recursive: true, force: true });
  }
}
