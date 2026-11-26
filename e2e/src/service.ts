// service.ts — what the file service's conformance specs share: their own account, its ticket, a call through
// /api/files, a workspace made and removed by name, and the service's folders read back under CONFORMANCE_FILES_ROOT.
// The account-state names (a journal's folder, a capture's folder, the active marker) are spelled here again on
// purpose: a spec never imports the service's code, so it agrees with no port by construction.
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { join, posix, win32 } from "node:path";
import {
  type APIRequestContext,
  expect,
  request as playwright,
} from "@playwright/test";
import { base, signedIn } from "./instance.ts";
import type { SessionUser } from "./session.ts";
import { accountDir, filesRoot } from "./sync.ts";
import { until } from "./until.ts";
import {
  catalogueOf,
  completeOf,
  importFileVia,
  importPathVia,
  pictureOf,
  renderPictureVia,
  runVerb,
  type VerbAnswer,
  type VerbService,
} from "./verbs.ts";

/** The file service's account: every workspace, journal and capture the specs make is the suite's alone. */
export const SERVICE_USER: SessionUser = {
  sub: "conformance-service",
  name: "Conformance Service",
  email: "conformance-service@st0.app",
  groups: [],
};

// biome-ignore lint/suspicious/noExplicitAny: a reply's JSON is read as each spec reads it, never declared twice.
type Json = any;

export type Reply<T = unknown> = {
  status: number;
  body: T;
  headers: Record<string, string>;
};

/** The account's file service, reached through the hub with a ticket, as a page reaches it. */
export class Service {
  private constructor(
    readonly request: APIRequestContext,
    readonly ticket: string,
    /** Who the ticket is for: the media route a runner reads bytes through asks for it in its query. */
    readonly email: string,
  ) {}

  /** A context of its own, so one connection serves a whole serial file: beforeAll's fixture cannot. */
  static async connect(user: SessionUser = SERVICE_USER): Promise<Service> {
    const request = await playwright.newContext({
      baseURL: base,
      ignoreHTTPSErrors: true,
    });
    const res = await request.get("/api/files-hub", {
      headers: signedIn(user),
    });
    expect(res.status()).toBe(200);
    const ticket = (await res.json()).ticket;
    return new Service(request, ticket, user.email ?? "");
  }

  dispose = () => this.request.dispose();

  headers(ws?: string, extra: Record<string, string> = {}) {
    return {
      "x-studio-engine-token": this.ticket,
      ...(ws ? { "x-studio-workspace": ws } : {}),
      ...extra,
    };
  }

  /** One request: its status, its JSON body (or text when it is not JSON), and its headers. */
  async call<T = Json>(
    method: string,
    path: string,
    opts: {
      ws?: string;
      data?: unknown;
      headers?: Record<string, string>;
      again?: boolean;
    } = {},
  ): Promise<Reply<T>> {
    const retry = method === "GET" || opts.again === true;
    // A minute of asking again at most, as long as a restarted service takes to answer.
    const deadline = Date.now() + 60_000;
    for (;;) {
      const res = await this.request
        .fetch(`/api/files${path}`, {
          method,
          headers: this.headers(opts.ws, opts.headers),
          data: opts.data,
        })
        .catch((e: Error) => {
          // The dev session's backend restarted under the call: a socket reset, asked again like a cut.
          if (
            retry &&
            Date.now() < deadline &&
            /ECONNRESET|ECONNREFUSED|socket hang up/.test(e.message)
          )
            return null;
          throw e;
        });
      if (!res) {
        await this.answering(deadline);
        continue;
      }
      const text = await res.text();
      let body: unknown = text;
      try {
        body = text ? JSON.parse(text) : undefined;
      } catch {
        /* not JSON: the text itself */
      }
      // A call cut by the hub restarting the service on a source change is asked again when asking twice is harmless.
      const cut =
        /^5\d\d$/.test(String(res.status())) &&
        /^file service/.test((body as { error?: string })?.error ?? "");
      if (cut && retry && Date.now() < deadline) {
        await this.answering(deadline);
        continue;
      }
      return { status: res.status(), body: body as T, headers: res.headers() };
    }
  }

  /** Waits until the account's file service answers a listing again, after a restart cut a call. */
  private async answering(deadline: number): Promise<void> {
    const status = () =>
      this.request
        .get("/api/files/workspaces", { headers: this.headers() })
        .then((r) => r.status())
        .catch(() => 0);
    // A listing that never answers ends the wait at the deadline; the call is then asked once more and returns.
    await until(status, (s) => s > 0 && s < 500, deadline - Date.now()).catch(
      () => undefined,
    );
  }

  get = <T = Json>(path: string, ws?: string) =>
    this.call<T>("GET", path, { ws });
  post = <T = Json>(path: string, data: unknown, ws?: string) =>
    this.call<T>("POST", path, { ws, data });
  /** A whole file written twice is written once: a PUT is asked again when the service was cut. */
  put = <T = Json>(path: string, data: unknown, ws?: string) =>
    this.call<T>("PUT", path, { ws, data, again: true });

  /** The service as the runtime's node build reaches it. */
  verbService(): VerbService {
    return {
      base: `${base}/api/files`,
      headers: (ws) => this.headers(ws, { "x-studio-user": this.email }),
    };
  }

  /** Runs one line through `POST /verb`, on the open project (`at` overrides). */
  async verb(
    ws: string,
    line: string,
    o: {
      want?: string[];
      journal?: "join" | "skip";
      at?: string;
      headers?: Record<string, string>;
    } = {},
  ): Promise<{ status: number; body: VerbAnswer }> {
    const at =
      o.at ??
      ((await this.get("/workspace", ws)).body.activeProject as
        | string
        | null) ??
      "";
    const body = await runVerb(this.verbService(), ws, at, line, o);
    return { status: 200, body };
  }

  /** A project file of the workspace, imported by the runtime's node build into a new folder. */
  importPath(ws: string, rel: string, as?: string) {
    return importPathVia(this.verbService(), ws, rel, as);
  }

  /** …and a file the caller holds: what a drop on the home page does. */
  importFile(ws: string, data: Blob, name: string, as?: string) {
    return importFileVia(this.verbService(), ws, data, name, as);
  }

  /** Performs `picture render`'s ask as the page does, on the open project; the picture job's id. */
  async renderPicture(ws: string, args: Record<string, unknown>) {
    const at =
      ((await this.get("/workspace", ws)).body.activeProject as
        | string
        | null) ?? "";
    const { rev } = (await this.get("/revision", ws)).body;
    return renderPictureVia(this.verbService(), ws, at, rev, args);
  }

  /** The open project's picture and its warnings, from the runtime's node build, as the page reads them. */
  async picture(ws: string) {
    const at =
      ((await this.get("/workspace", ws)).body.activeProject as
        | string
        | null) ?? "";
    const { rev } = (await this.get("/revision", ws)).body;
    return pictureOf(this.verbService(), ws, at, rev);
  }

  /** The command bar's list, as the page builds it. */
  catalogue(_ws: string) {
    return catalogueOf();
  }

  /** A completion on the open project, through `POST /verb/complete`. */
  complete(ws: string, line: string) {
    return completeOf(this.verbService(), ws, line);
  }

  /** A file's text through GET /file, which must answer it. */
  async text(ws: string, path: string): Promise<string> {
    const r = await this.get(`/file?path=${encodeURIComponent(path)}`, ws);
    expect(r.status, `${path}: ${JSON.stringify(r.body)}`).toBe(200);
    return r.body.content;
  }

  /** Bytes put through POST /upload, refused when the file exists. */
  upload(ws: string, dir: string, name: string, bytes: Buffer) {
    return this.call("POST", `/upload?${new URLSearchParams({ dir, name })}`, {
      ws,
      data: bytes,
      headers: { "content-type": "application/octet-stream" },
    });
  }

  /** Make `project` the open one, waiting out the sync gate's 409. */
  open(ws: string, project: string) {
    return until(
      () =>
        this.call("POST", "/open", {
          ws,
          data: { path: project },
          again: true,
        }),
      (r) => r.status !== 409,
    );
  }

  async names(): Promise<string[]> {
    const r = await this.get<{ workspaces: { name: string }[] }>("/workspaces");
    return r.body.workspaces.map((w) => w.name);
  }

  /** Make `ws`, which must not exist: `false` only when a first try was cut after it made it. */
  async create(ws: string) {
    const r = await this.call("POST", "/workspaces", {
      data: { name: ws },
      again: true,
    });
    expect([r.status, typeof r.body.created]).toEqual([200, "boolean"]);
    expect(await this.names()).toContain(ws);
  }

  /** Remove `ws` until a listing no longer brings it back: a removal cut by a service restart is asked again. */
  async remove(ws: string) {
    // Its projects' journals go too: a journal outlives its workspace, and is the suite's to remove by name.
    const journals = filesRoot ? await this.journalsOf(ws) : [];
    const gone = async () =>
      !(await this.names()).includes(ws) &&
      !(filesRoot && existsSync(wsDir(ws)));
    for (let round = 0; round < 3; round++) {
      await this.call("POST", "/workspaces/remove", {
        data: { name: ws },
        again: true,
      });
      // Gone, then gone again: a listing is what brings back a workspace its relay room still holds.
      if ((await until(gone, (g) => g, 5_000)) && (await gone())) break;
    }
    for (const dir of journals) rmSync(dir, { recursive: true, force: true });
  }

  /** The journal folder of every project folder in `ws`, from the service's own root; none when it is gone. */
  private async journalsOf(ws: string): Promise<string[]> {
    const r = await this.get<{ root?: string }>("/workspace", ws);
    if (r.status !== 200 || !r.body.root || !existsSync(wsDir(ws))) return [];
    const root = r.body.root;
    return readdirSync(wsDir(ws), { withFileTypes: true })
      .filter((e) => e.isDirectory() && !e.name.startsWith("."))
      .map((e) => journalDir(root, e.name));
  }

  /** The workspace's root as the SERVICE spells it, which the hashed folder names are made from. */
  async root(ws: string): Promise<string> {
    return (await this.get<{ root: string }>("/workspace", ws)).body.root;
  }
}

/** The account's folder on this machine; only called by a test that skips without CONFORMANCE_FILES_ROOT. */
export const serviceDir = (): string => accountDir(SERVICE_USER);

/** A workspace's folder on this machine. */
export const wsDir = (ws: string): string =>
  join(serviceDir(), "workspace", ws);

/** A file of a workspace, read from disk as bytes. */
export const onDiskBytes = (ws: string, rel: string): Buffer =>
  readFileSync(join(wsDir(ws), ...rel.split("/")));

/** …and as text. */
export const onDisk = (ws: string, rel: string): string =>
  onDiskBytes(ws, rel).toString("utf8");

/** Whether a path the service answered is a Windows one: it starts with a drive. */
export const spelledOnWindows = (path: string): boolean =>
  /^[A-Za-z]:[\\/]/.test(path);

/** The path module the service's own paths are spelled in: Windows when its root has a drive. */
const pathOf = (root: string) => (spelledOnWindows(root) ? win32 : posix);

/** Whether the SERVER runs on Windows, from the root its file service spells for Examples; the runner's own is no clue. */
export async function serverOnWindows(): Promise<boolean> {
  const svc = await Service.connect();
  try {
    const r = await svc.get<{ root?: string }>("/workspace", "Examples");
    expect(r.status, JSON.stringify(r.body)).toBe(200);
    return spelledOnWindows(r.body.root ?? "");
  } finally {
    await svc.dispose();
  }
}

/** `<name>-<12 hex of sha256 of the absolute path>`: how a journal or a capture log names its folder. */
export function hashedName(abs: string): string {
  const p = pathOf(abs);
  const name = p.basename(abs).replace(/[^\w.-]+/g, "_") || "project";
  return `${name}-${createHash("sha256").update(abs).digest("hex").slice(0, 12)}`;
}

/** A project's undo journal folder under the account's `.history`, from the service's own root. */
export const journalDir = (root: string, project: string): string =>
  join(serviceDir(), ".history", hashedName(pathOf(root).join(root, project)));

/** A workspace's capture log folder under the account's `.sync`. */
export const captureDir = (root: string): string =>
  join(serviceDir(), ".sync", hashedName(root));

/** The marker naming a workspace's open project: beside the workspaces, never in one. */
export const markerFile = (ws: string): string =>
  join(serviceDir(), "workspace", ".active-projects", ws);

/** A random id written by a scaffold or a verb, for a text compared with its ids blanked. */
export const UUID =
  /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi;

/** `text` with every id as `<id>`. */
export const blankIds = (text: string): string => text.replace(UUID, "<id>");

/** Every file under `dir`, relative and posix, names sorted by code point per folder, depth first. */
export function filesUnder(dir: string, rel = ""): string[] {
  return readdirSync(join(dir, rel), { withFileTypes: true })
    .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))
    .flatMap((e) =>
      e.isDirectory()
        ? filesUnder(dir, rel ? `${rel}/${e.name}` : e.name)
        : [rel ? `${rel}/${e.name}` : e.name],
    );
}

/** The sha256 of `bytes`, as hex. */
export const sha = (bytes: Buffer | string): string =>
  createHash("sha256").update(bytes).digest("hex");

/** One `.jsonl` file's lines, parsed. */
export const jsonLines = (file: string): Json[] =>
  existsSync(file)
    ? readFileSync(file, "utf8")
        .split("\n")
        .filter(Boolean)
        .map((l) => JSON.parse(l))
    : [];
