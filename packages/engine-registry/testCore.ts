// testCore.ts — the registry, its routes and its relay as the suites test them: the Rust testbed ENGINE_CORE_BIN
// names, one process per test, its sessions minted as cookies and its store reached through its `/__engine-test`
// hooks (apps/k7/backend-rs/docs/engine.md). Without it the suites skip, or fail where ENGINE_CORE_REQUIRED says so.
// The one thing a reader would get wrong: `reg` is the registry's API spoken through those hooks, asynchronously.

import { createHmac } from "node:crypto";
import {
  describeCore,
  RUST_CORE,
  startTestbed,
  TESTBED_SECRET,
  type Testbed,
} from "@studio/engine-tunnel/test-core";
import type { EngineAddresses, PublicEngine } from "@studio/web-protocol";
import type { EnginePerformance } from "@studio/web-protocol/performance";
import { WebSocket } from "ws";
import type { Ownership } from "./owners.ts";

export { describeCore, RUST_CORE };

/** How long a listing lasts without a heartbeat. */
export const REGISTRY_TTL_MS = 15_000;
/** The id-to-secret bindings' file under the registry's state folder. */
export const BINDINGS_FILE = "engine-bindings.json";
/** The owners' file beside it. */
export const OWNERS_FILE = "engine-owners.json";

export interface CoreOptions {
  /** The account an unsigned call is made as, as a suite that sets one `req.user` for all. */
  user?: string;
  /** Who may list an engine; absent, anyone. */
  heartbeat?: {
    requireDevice: boolean;
    renderKey: string;
  };
  /** Accounts that administer this origin. */
  admins?: string[];
  /** An engine the hub reads as the render machine. */
  renderer?: string;
  /** Believe `X-Forwarded-For`, as behind one proxy. */
  trustProxy?: boolean;
  /** Where the bindings and owners outlive a restart. */
  stateDir?: string;
  maxEngines?: number;
  newIdsPerIp?: number;
  /** Answers a WebRTC offer, standing for an engine's machine on its tunnel. */
  signal?: (sdp: string) => Promise<string>;
  /** A real tunnel hub over the registry, its `verify` the registry's own. */
  hub?: boolean;
  /** The internal seam's engine half, behind its gate (docs/internalSeam.md). */
  seam?: boolean;
  /** Where the testbed listens, as a host and a port; a restart on the same port is the same front again. */
  listen?: string;
}

/** A heartbeat as the registry reads it. */
export interface HeartbeatInput {
  id: string;
  token: string;
  hostname: string;
  machineFingerprint: string;
  addresses: EngineAddresses;
  performance?: EnginePerformance;
  libraryLevel?: number;
  protocol?: number;
  build?: string;
  safeMode?: boolean;
  publicIpHint?: string;
  tunnelSecret?: string;
}

type Upsert = { ok: true } | { ok: false; why: string };
type Claim =
  | {
      ok: true;
      secret: string;
      tookOver?: { holder: string; project: string | null };
    }
  | { ok: false; lost: string; heldBy: string; tookFor?: string | null };

/** The registry's API, asynchronous whichever process holds it. */
export interface AsyncRegistry {
  upsert(input: HeartbeatInput): Promise<Upsert>;
  get(id: string): Promise<
    | {
        id: string;
        token: string;
        hostname: string;
        performance?: unknown;
        legacy: boolean;
      }
    | undefined
  >;
  list(ip?: string, viewer?: string): Promise<PublicEngine[]>;
  claim(
    id: string,
    req: { project: string | null; holder: string; secret?: string },
  ): Promise<Claim>;
  release(id: string, secret: string): Promise<void>;
  holds(id: string, holder: string): Promise<boolean>;
  engineForProject(project: string): Promise<string | undefined>;
  verifyToken(id: string, token: string): Promise<boolean>;
  verifyTunnel(id: string, token: string, secret?: string): Promise<boolean>;
  remove(id: string, token: string, secret?: string): Promise<boolean>;
  binding(id: string, secret?: string): Promise<string>;
  owners: {
    get(id: string): Promise<Ownership | undefined>;
    assign(id: string, owner: string): Promise<string>;
    share(id: string, emails: string[], everyone?: boolean): Promise<boolean>;
    release(id: string): Promise<boolean>;
    all(): Promise<[string, Ownership][]>;
  };
}

export interface CoreTarget {
  /** The secret the seam's gate takes. */
  seamSecret: string;
  base: string;
  port: number;
  lines: string[];
  reg: AsyncRegistry;
  ticket(id: string, holder: string): Promise<string>;
  verifyTicket(t: string): Promise<{ id: string; holder: string } | null>;
  /** A ticket minted by another process of the same implementation. */
  foreignTicket(id: string, holder: string): Promise<string>;
  /** `Date` moved, as `vi.setSystemTime` moves it in-process. */
  clock: {
    fake(): void;
    real(): void;
    set(ms: number): Promise<void>;
    advance(ms: number): Promise<void>;
  };
  hubHas(id: string): Promise<boolean>;
  /** A request signed in as `user` ("" an anonymous session). */
  call(
    user: string,
    method: string,
    path: string,
    body?: unknown,
    headers?: Record<string, string>,
  ): Promise<{ status: number; body: never }>;
  /** A request as the suite's default account, or none. */
  post(
    path: string,
    body: unknown,
    headers?: Record<string, string>,
  ): Promise<{ status: number; body: never }>;
  stop(): Promise<void>;
}

export function startCore(opts: CoreOptions = {}): Promise<CoreTarget> {
  return rustCore(opts);
}

const parse = async (res: Response) => ({
  status: res.status,
  body: (await res.json().catch(() => null)) as never,
});

/** The cookies a browser of `user` sends, signed as cookie-session and keygrip sign them. */
function cookieOf(user: string, admins: string[] = []): string {
  const u = user
    ? { email: user, ...(admins.includes(user) ? { groups: ["admins"] } : {}) }
    : {};
  const iat = Date.now();
  const value = Buffer.from(
    JSON.stringify({ user: u, iat, exp: iat + 86_400_000 }),
  ).toString("base64");
  const sig = createHmac("sha1", TESTBED_SECRET)
    .update(`studio.sid=${value}`)
    .digest("base64")
    .replace(/\//g, "_")
    .replace(/\+/g, "-")
    .replace(/=/g, "");
  return `studio.sid=${value}; studio.sid.sig=${sig}`;
}

async function rustCore(opts: CoreOptions): Promise<CoreTarget> {
  const env: Record<string, string> = {};
  if (opts.trustProxy) env.K7_FRONT_TRUST_HOPS = "1";
  if (opts.stateDir) env.K7_ENGINE_TEST_STATE_DIR = opts.stateDir;
  if (opts.maxEngines) env.K7_ENGINE_TEST_MAX_ENGINES = String(opts.maxEngines);
  if (opts.newIdsPerIp)
    env.K7_ENGINE_TEST_NEW_IDS_PER_IP = String(opts.newIdsPerIp);
  if (opts.heartbeat) {
    env.STUDIO_RENDER_KEY = opts.heartbeat.renderKey;
    if (opts.heartbeat.requireDevice) env.STUDIO_REGISTRY_REQUIRE_DEVICE = "1";
  }
  if (opts.seam) env.K7_FRONT_LISTEN = "0.0.0.0:0";
  if (opts.listen) env.K7_FRONT_LISTEN = opts.listen;
  const bed: Testbed = await startTestbed(env);
  if (opts.renderer) await bed.hook("hubMarkRendered", { id: opts.renderer });
  const extra: WebSocket[] = [];
  if (opts.signal) {
    await bed.hook("verify", { mode: "all" });
    extra.push(await fakeSupervisor(bed.base, opts.signal));
  }
  const send = (
    cookie: string | undefined,
    method: string,
    path: string,
    body: unknown,
    headers: Record<string, string>,
  ) =>
    fetch(`${bed.base}/api${path}`, {
      method,
      headers: {
        "content-type": "application/json",
        ...(cookie ? { cookie } : {}),
        ...headers,
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    }).then(parse);
  const h = bed.hook;
  return {
    seamSecret: TESTBED_SECRET,
    base: bed.base,
    port: bed.port,
    lines: bed.lines,
    reg: {
      upsert: (i) => h<Upsert>("upsert", i as never),
      get: async (id) => (await h<never>("get", { id })) ?? undefined,
      list: (ip, viewer) => h<PublicEngine[]>("list", { ip, viewer }),
      claim: (id, req) => h<Claim>("claim", { id, ...req }),
      release: async (id, secret) => void (await h("release", { id, secret })),
      holds: (id, holder) => h<boolean>("holds", { id, holder }),
      engineForProject: async (project) =>
        (await h<string | null>("engineForProject", { project })) ?? undefined,
      verifyToken: (id, token) => h<boolean>("verifyToken", { id, token }),
      verifyTunnel: (id, token, secret) =>
        h<boolean>("verifyTunnel", { id, token, secret }),
      remove: (id, token, secret) =>
        h<boolean>("remove", { id, token, secret }),
      binding: (id, secret) => h<string>("binding", { id, secret }),
      owners: {
        get: async (id) =>
          (await h<Ownership | null>("ownerGet", { id })) ?? undefined,
        assign: (id, owner) => h<string>("ownerAssign", { id, owner }),
        share: (id, emails, everyone) =>
          h<boolean>("ownerShare", { id, emails, everyone }),
        release: (id) => h<boolean>("ownerRelease", { id }),
        all: () => h<[string, Ownership][]>("ownersAll"),
      },
    },
    ticket: (id, holder) => h<string>("ticket", { id, holder }),
    verifyTicket: (ticket) => h("verifyTicket", { ticket }),
    async foreignTicket(id, holder) {
      const other = await startTestbed();
      try {
        return await other.hook<string>("ticket", { id, holder });
      } finally {
        await other.stop();
      }
    },
    clock: {
      fake: () => {},
      real: () => {},
      set: async (now) => void (await h("clock", { now })),
      advance: async (advance) => void (await h("clock", { advance })),
    },
    hubHas: (id) => h<boolean>("hubHas", { id }),
    call: (user, method, path, body, headers = {}) =>
      send(cookieOf(user, opts.admins), method, path, body, headers),
    post: (path, body, headers = {}) =>
      send(
        opts.user === undefined ? undefined : cookieOf(opts.user, opts.admins),
        "POST",
        path,
        body,
        headers,
      ),
    stop: async () => {
      for (const w of extra) w.terminate();
      await bed.stop();
    },
  };
}

/** A bare supervisor for engine `e1` whose machine answers every WebRTC offer with `answer`. */
async function fakeSupervisor(
  base: string,
  answer: (sdp: string) => Promise<string>,
): Promise<WebSocket> {
  const ws = new WebSocket(
    `${base.replace("http", "ws")}/api/engine-tunnel?id=e1`,
    {
      headers: { "x-studio-engine-token": "tok" },
    },
  );
  ws.on("message", (d, binary) => {
    if (binary) return;
    const m = JSON.parse(String(d)) as { t: string; req: number; sdp: string };
    if (m.t === "rtc-offer")
      void answer(m.sdp).then((sdp) =>
        ws.send(JSON.stringify({ t: "rtc-answer", req: m.req, sdp })),
      );
  });
  await new Promise((r, j) => {
    ws.once("open", r);
    ws.once("error", j);
  });
  return ws;
}
