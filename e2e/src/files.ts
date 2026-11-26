// files.ts — the e2e account's file service, reached the way the browser reaches it: through /api/files with a hub ticket.
// Writes go over HTTP, never to disk: the capture reads a folder deleted behind its back as an edit (docs/e2e.md).
import { type APIRequestContext, type Page, request } from "@playwright/test";
import { until } from "./until.ts";
import { runVerb, type VerbService } from "./verbs.ts";

const TICKET_HEADER = "x-studio-engine-token";
const WORKSPACE_HEADER = "x-studio-workspace";

/** One entry of GET /workspace-tree. */
export type TreeNode = { path: string; dir: boolean; children?: TreeNode[] };

/** One workspace of GET /workspaces/projects, with its project folders. */
export type WorkspaceListing = { name: string; projects: { path: string }[] };

/** One cookie-less session per phone origin, since `ofPage` is called on every poll of a read. */
const phoneContexts = new Map<string, Promise<APIRequestContext>>();

export class FileService {
  private constructor(
    /** The session the service is reached with, which reaches the rest of k7's backend too. */
    readonly request: APIRequestContext,
    private readonly base: string,
    private readonly ticket: string,
  ) {}

  /** Asks k7's backend for a ticket; `request` must carry the e2e session cookie, and `origin` prefixes every path. */
  static async connect(
    request: APIRequestContext,
    origin = "",
  ): Promise<FileService> {
    const res = await request.get(`${origin}/api/files-hub`);
    if (!res.ok())
      throw new Error(`files-hub: ${res.status()} ${await res.text()}`);
    const { path, ticket } = (await res.json()) as {
      path: string;
      ticket: string;
    };
    return new FileService(request, `${origin}${path}`, ticket);
  }

  /** The service as the PAGE reaches it, with its cookies at its origin: it sees what the account driving the screen wrote. */
  static async ofPage(page: Page): Promise<FileService> {
    const url = new URL(page.url());
    const http = url.protocol === "http:" || url.protocol === "https:";
    // A phone's WebView gives Playwright no cookies (Storage.getCookies), and its front signs the local user in without one.
    if (process.env.E2E_TARGET === "android" && http) {
      let api = phoneContexts.get(url.origin);
      if (!api) {
        // `/auth/me` hands the context a cookie of its own, as the app's view is handed one (android.ts).
        api = request.newContext().then(async (ctx) => {
          await ctx.get(`${url.origin}/auth/me`);
          return ctx;
        });
        phoneContexts.set(url.origin, api);
      }
      return FileService.connect(await api, url.origin);
    }
    return FileService.connect(page.request, http ? url.origin : "");
  }

  /** The engine the calls name, as a page that picked one names it: a render goes to that engine's machine. */
  private engine = "";

  /** Names `id` on every later call, which binds the account's session to that engine. */
  bindEngine(id: string): void {
    this.engine = id;
  }

  private headers(workspace?: string): Record<string, string> {
    const h: Record<string, string> = { [TICKET_HEADER]: this.ticket };
    if (this.engine) h["x-studio-engine-id"] = this.engine;
    if (workspace) h[WORKSPACE_HEADER] = workspace;
    return h;
  }

  private async call<T>(
    method: "GET" | "POST" | "PUT",
    path: string,
    opts: { workspace?: string; data?: unknown; ok?: number[] } = {},
  ): Promise<{ status: number; body: T }> {
    const res = await this.request.fetch(`${this.base}${path}`, {
      method,
      headers: this.headers(opts.workspace),
      data: opts.data,
    });
    const text = await res.text();
    const body = (text ? JSON.parse(text) : undefined) as T;
    if (!res.ok() && !opts.ok?.includes(res.status()))
      throw new Error(`${method} ${path}: ${res.status()} ${text}`);
    return { status: res.status(), body };
  }

  /** Removes the workspace and its relay room; a missing one is not an error. */
  async removeWorkspace(name: string) {
    await this.call("POST", "/workspaces/remove", {
      data: { name },
      ok: [400],
    });
  }

  /** The libraries the hub has a published version of: a workspace cannot be created before one. */
  async libraries(): Promise<string[]> {
    const { body } = await this.call<{
      libraries: { name: string; present: boolean; versions?: unknown[] }[];
    }>("GET", "/account/libraries");
    return body.libraries
      .filter((l) => l.present && l.versions?.length)
      .map((l) => l.name);
  }

  async createWorkspace(name: string) {
    await this.call("POST", "/workspaces", { data: { name } });
  }

  /** Scaffolds a project by archetype, as the "New project" dialog does. */
  async createProject(workspace: string, shape: string, name: string) {
    return (
      await this.call<{ project: string }>("POST", "/project/create", {
        workspace,
        data: { shape, name },
      })
    ).body;
  }

  /** The text of `path`, through GET /file. */
  async readText(workspace: string, path: string): Promise<string> {
    const q = encodeURIComponent(path);
    const { body } = await this.call<{ content: string }>(
      "GET",
      `/file?path=${q}`,
      { workspace },
    );
    return body.content;
  }

  /** The workspace's folders and files, nested as GET /workspace-tree answers them. */
  async tree(workspace: string): Promise<TreeNode[]> {
    return (
      await this.call<TreeNode[]>("GET", "/workspace-tree", { workspace })
    ).body;
  }

  /** Every workspace of the account with its project folders, through GET /workspaces/projects. */
  async projects(): Promise<WorkspaceListing[]> {
    const { body } = await this.call<{ workspaces: WorkspaceListing[] }>(
      "GET",
      "/workspaces/projects",
    );
    return body.workspaces;
  }

  /** Runs one command-bar line on the active project (none: the workspace alone), through the runtime's node build. */
  async command(
    workspace: string,
    line: string,
  ): Promise<{ output: unknown; refused?: boolean }> {
    const at = await this.activeProject(workspace);
    const headers = this.engine
      ? { "x-studio-engine-id": this.engine }
      : undefined;
    return runVerb(this.verbService(), workspace, at, line, {
      want: [],
      headers,
    });
  }

  /** The mixdown of the open project, rendered by the engine as the page's Export does: the status and the bytes. */
  async exportMix(
    workspace: string,
    format: "wav" | "mp3" = "wav",
  ): Promise<{ status: number; bytes: Buffer }> {
    const res = await this.request.fetch(
      `${this.base}/export?format=${format}`,
      { headers: this.headers(workspace), timeout: 180_000 },
    );
    return { status: res.status(), bytes: await res.body() };
  }

  /** The tracks zip of the open project, as the page's link asks it: the workspace rides in the query, no header. */
  async exportTracks(
    workspace: string,
  ): Promise<{ status: number; bytes: Buffer }> {
    const res = await this.request.fetch(
      `${this.base}/export/tracks?workspace=${encodeURIComponent(workspace)}`,
      { timeout: 240_000 },
    );
    return { status: res.status(), bytes: await res.body() };
  }

  /** The workspace's active project as `/changes` names it, "" when none is open. */
  async activeProject(workspace: string): Promise<string> {
    const { body } = await this.call<{ activeProject?: string | null }>(
      "GET",
      "/workspace",
      { workspace },
    );
    return body.activeProject ?? "";
  }

  /** A host member as a verb asks it (`/verb-host/<member>`): the status and body, whatever the status. */
  async host(
    workspace: string,
    member: string,
    data: unknown,
  ): Promise<{ status: number; body: Record<string, unknown> }> {
    return this.call("POST", `/verb-host/${member}`, {
      workspace,
      data,
      ok: [400, 403, 404, 409, 415, 500, 503],
    });
  }

  /** A host job polled to its end, as the backend does each second. */
  async hostJob(
    workspace: string,
    id: string,
    timeoutMs = 60_000,
  ): Promise<Record<string, unknown>> {
    return until(
      async () =>
        (
          await this.call<Record<string, unknown>>(
            "GET",
            `/verb-host/job?id=${id}`,
            { workspace },
          )
        ).body,
      (b) => b.state !== "running",
      timeoutMs,
    );
  }

  /** What the runtime's node build needs of this service. */
  verbService(): VerbService {
    // A connection made through a page has a relative base; the runner is node's and wants the whole URL.
    const device = process.env.E2E_TARGET && process.env.E2E_TARGET !== "web";
    if (this.base.startsWith("/") && device && !process.env.E2E_BASE_URL)
      throw new Error(
        `a ${process.env.E2E_TARGET} target's verb runner needs the app's origin, not the web default`,
      );
    const origin = process.env.E2E_BASE_URL ?? "https://app.priv.k7.rocks";
    return {
      base: this.base.startsWith("/") ? `${origin}${this.base}` : this.base,
      headers: (workspace) => this.headers(workspace) as Record<string, string>,
    };
  }

  /** A clip's envelope through GET /waveform; `whole` asks for the whole file rather than the clip's window. */
  async waveform(
    workspace: string,
    clip: string,
    opts: { whole?: boolean } = {},
  ): Promise<{ peak: number[] }> {
    const q = new URLSearchParams({ clip });
    if (opts.whole) q.set("whole", "1");
    const { body } = await this.call<{ peak: number[] }>(
      "GET",
      `/waveform?${q}`,
      { workspace },
    );
    return body;
  }

  /** The last `tail` lines of the engine machine's log, the Logs panel's source, through GET /logs. */
  async logs(workspace: string, tail = 2000): Promise<string[]> {
    const { body } = await this.call<{ lines: string[] }>(
      "GET",
      `/logs?tail=${tail}`,
      { workspace },
    );
    return body.lines;
  }

  /** The service's own log lines (a background job's end lands here), through GET /logs. */
  async serviceLog(workspace: string, tail = 2000): Promise<string[]> {
    const { body } = await this.call<{
      service: { lines: { text: string }[] };
    }>("GET", `/logs?tail=${tail}`, { workspace });
    return body.service.lines.map((l) => l.text);
  }

  /** Recycles one file or folder of the workspace, through POST /entry/delete. */
  async deleteEntry(workspace: string, path: string) {
    await this.call("POST", "/entry/delete", { workspace, data: { path } });
  }

  async writeText(workspace: string, path: string, content: string) {
    await this.call("PUT", "/file", { workspace, data: { path, content } });
  }

  /** Uploads bytes; the service refuses to overwrite, so a fresh workspace is assumed. */
  async upload(workspace: string, path: string, bytes: Buffer) {
    const slash = path.lastIndexOf("/");
    const query = new URLSearchParams({
      dir: slash < 0 ? "" : path.slice(0, slash),
      name: path.slice(slash + 1),
    });
    const res = await this.request.fetch(`${this.base}/upload?${query}`, {
      method: "POST",
      headers: {
        ...this.headers(workspace),
        "content-type": "application/octet-stream",
      },
      data: bytes,
    });
    if (!res.ok())
      throw new Error(`upload ${path}: ${res.status()} ${await res.text()}`);
  }

  /** Makes `project` the workspace's active one. */
  async open(workspace: string, project: string) {
    await this.call("POST", "/open", { workspace, data: { path: project } });
  }

  /** The active project's compile errors; empty when it loads. */
  async projectErrors(workspace: string): Promise<unknown> {
    return (await this.call("GET", "/project-errors", { workspace })).body;
  }

  /** The active project compiled for the engine; throws with the compiler's message when it does not compile. */
  async engineDoc(workspace: string): Promise<unknown> {
    return (await this.call("GET", "/engine-doc", { workspace })).body;
  }
}
