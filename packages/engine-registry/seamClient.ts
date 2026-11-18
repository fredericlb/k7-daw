// seamClient.ts — the TypeScript callers' side of the internal seam (seam.ts): the registry's reads, the hub's
// loopback bindings and the local engine's commands, every one an HTTP call to whichever process holds them.
// A channel is a WebSocket to a ONE-SHOT binding, dropped once the upgrade is settled: the hub reads a binding
// at the upgrade only. A KEPT binding outlives a restart of the hub's process: re-established under its key.
// The contract is docs/internalSeam.md.

import { randomBytes } from "node:crypto";
import type { ChannelKind, TunnelRoot } from "@studio/engine-tunnel/frames";
import { WebSocket } from "ws";
import {
  type EngineView,
  INTERNAL_ENGINE_PATH,
  INTERNAL_SECRET_HEADER,
} from "./seam.ts";

export type { EngineView } from "./seam.ts";

/** Where the seam is reached and how it is proved. */
export interface SeamConfig {
  /** `http://127.0.0.1:<port>`, read per call: a test learns its port after listening. */
  base: () => string;
  secret: string;
  /** How often a client holding bindings asks the hub's epoch; default HUB_WATCH_MS. */
  watchMs?: number;
  log?: (msg: string) => void;
}

/** What the hub says of the server's render machine, and which run of it answers. */
export interface HubState {
  rendersEnabled: boolean;
  renderer: string | null;
  /** Changes when the process holding the hub restarts, which forgets every binding. */
  epoch?: string;
}

/** A binding `bind` answered and `unbind` has not dropped, and the hub epoch it is known to. */
interface Held {
  engineId: string | null;
  roots: TunnelRoot[];
  kind: ChannelKind;
  synced?: boolean;
  epoch?: string;
}

/** The seam did not answer, or answered what the contract does not allow. */
export class SeamError extends Error {}

/** How long a channel's upgrade may take before it counts as no channel. */
const OPEN_MS = 10_000;
/** How soon a hub that restarted gets the bindings back: a file service redials its engine every 4 s at most. */
const HUB_WATCH_MS = 2_000;

const keyOf = (path: string): string => path.split("/").pop() ?? "";

/** The front's address when one runs in front of this backend, else this backend's own; the secret is the front's, or drawn now. */
export function seamConfigFromEnv(
  ownPort: number,
  env: NodeJS.ProcessEnv = process.env,
): { base: string; secret: string } {
  const front = env.K7_FRONT_SECRET?.trim() ?? "";
  const frontPort = env.K7_FRONT_PORT?.trim() ?? "";
  return {
    base: `http://127.0.0.1:${front && frontPort ? frontPort : ownPort}`,
    secret: front || randomBytes(32).toString("hex"),
  };
}

export class EngineSeam {
  /** The kept bindings by key, re-established when the hub's epoch moves on. */
  private readonly held = new Map<string, Held>();
  private watch: NodeJS.Timeout | null = null;
  private checking = false;

  constructor(private readonly c: SeamConfig) {}

  private async call(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<Response> {
    try {
      return await fetch(`${this.c.base()}${INTERNAL_ENGINE_PATH}${path}`, {
        method,
        headers: {
          [INTERNAL_SECRET_HEADER]: this.c.secret,
          ...(body === undefined ? {} : { "content-type": "application/json" }),
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    } catch (e) {
      throw new SeamError(
        `the engine seam did not answer: ${(e as Error).message}`,
      );
    }
  }

  private async json<T>(r: Response, what: string): Promise<T> {
    if (!r.ok) {
      const said = (await r.json().catch(() => ({}))) as { error?: string };
      throw new SeamError(
        `the engine seam refused ${what} (${r.status}): ${said.error ?? ""}`,
      );
    }
    return (await r.json()) as T;
  }

  /** Engine `id` as `holder` sees it, or null when no engine has that id. */
  async engine(id: string, holder: string): Promise<EngineView | null> {
    const r = await this.call(
      "GET",
      `/engines/${encodeURIComponent(id)}?holder=${encodeURIComponent(holder)}`,
    );
    if (r.status === 404) return null;
    return this.json<EngineView>(r, "an engine");
  }

  /** Every live engine, as `holder` sees each. */
  async engines(holder: string): Promise<EngineView[]> {
    const r = await this.call(
      "GET",
      `/engines?holder=${encodeURIComponent(holder)}`,
    );
    return (await this.json<{ engines: EngineView[] }>(r, "the engines"))
      .engines;
  }

  async hub(): Promise<HubState> {
    return this.json<HubState>(await this.call("GET", "/hub"), "the hub");
  }

  private async post(
    b: Omit<Held, "epoch">,
    key?: string,
  ): Promise<{ path: string; epoch?: string }> {
    const r = await this.call("POST", "/bindings", {
      engineId: b.engineId,
      roots: b.roots,
      kind: b.kind,
      ...(b.synced === undefined ? {} : { synced: b.synced }),
      ...(key === undefined ? {} : { key }),
    });
    return this.json<{ path: string; epoch?: string }>(r, "a binding");
  }

  /** A loopback path reaching engine `engineId`, or the render machine when null, with `roots` shared; kept until `unbind`. */
  async bind(
    engineId: string | null,
    roots: TunnelRoot[],
    kind: ChannelKind,
    { synced }: { synced?: boolean } = {},
  ): Promise<string> {
    const b = { engineId, roots, kind, synced };
    const { path, epoch } = await this.post(b);
    this.held.set(keyOf(path), { ...b, epoch });
    this.watchHub();
    return path;
  }

  /** Drop a path `bind` answered; dropping one twice, or one never bound, is no error. */
  async unbind(path: string): Promise<void> {
    const key = keyOf(path);
    this.held.delete(key);
    if (this.held.size === 0 && this.watch) {
      clearInterval(this.watch);
      this.watch = null;
    }
    await this.drop(key);
  }

  private async drop(key: string): Promise<void> {
    const r = await this.call("DELETE", `/bindings/${encodeURIComponent(key)}`);
    if (!r.ok && r.status !== 404)
      throw new SeamError(`the engine seam refused an unbind (${r.status})`);
  }

  private watchHub(): void {
    if (this.watch) return;
    this.watch = setInterval(() => {
      if (this.checking) return;
      this.checking = true;
      // A hub that does not answer is restarting: the next tick asks again.
      void this.rebind()
        .catch(() => 0)
        .finally(() => {
          this.checking = false;
        });
    }, this.c.watchMs ?? HUB_WATCH_MS);
    this.watch.unref();
  }

  /** Re-establish, under its own key, every kept binding a restarted hub forgot; how many were. */
  async rebind(): Promise<number> {
    if (this.held.size === 0) return 0;
    const { epoch } = await this.hub();
    let n = 0;
    for (const [key, b] of [...this.held]) {
      if (b.epoch === epoch) continue;
      const again = await this.post(b, key);
      // Dropped while it was re-posted: the hub must not keep it.
      if (!this.held.has(key)) await this.drop(key).catch(() => {});
      else {
        b.epoch = again.epoch;
        n++;
      }
    }
    if (n)
      this.c.log?.(
        `the engine hub restarted: ${n} loopback binding(s) re-established`,
      );
    return n;
  }

  /** The `ws://` address of a bound path, on the process the seam is reached at. */
  wsUrl(path: string): string {
    return `${this.c.base().replace(/^http/, "ws")}${path}`;
  }

  /** A socket to engine `engineId` (null: the render machine), or null when it has no tunnel or the upgrade fails. */
  async open(
    engineId: string | null,
    roots: TunnelRoot[],
    kind: ChannelKind,
    {
      synced,
      timeoutMs = OPEN_MS,
    }: { synced?: boolean; timeoutMs?: number } = {},
  ): Promise<WebSocket | null> {
    const { path } = await this.post({ engineId, roots, kind, synced });
    const ws = new WebSocket(this.wsUrl(path));
    // A refused upgrade is an `error` then a `close`; unheard, the error would end the process.
    ws.on("error", () => {});
    const opened = await new Promise<boolean>((resolve) => {
      const timer = setTimeout(() => {
        ws.terminate();
        resolve(false);
      }, timeoutMs);
      ws.once("open", () => {
        clearTimeout(timer);
        resolve(true);
      });
      ws.once("close", () => {
        clearTimeout(timer);
        resolve(false);
      });
    });
    // The hub read the binding at the upgrade; it serves nobody after.
    await this.drop(keyOf(path)).catch(() => {});
    return opened ? ws : null;
  }

  /** Whether the local engine's supervisor answered yet, or null when this backend runs none. */
  async localEngine(): Promise<{ answered: boolean } | null> {
    const r = await this.call("GET", "/local");
    if (r.status === 404) return null;
    return this.json<{ answered: boolean }>(r, "the local engine");
  }
}
