// api.ts — what this screen asks of the account's file service, which holds every workspace
// and the libraries pointer, so what projects EXIST — and creating, deleting and opening one — is its answer.
//
// Every call to it names its `AccountFiles` (that service and its ticket): this screen opens no project,
// so `fileApi`'s module-level transport is unconfigured here and would throw. See ../../docs/projects.md.

import { readJson } from "@studio/http-client/call";
import type { ProjectShape } from "@studio/song-schema/yaml/projectSchema";
import {
  type AccountLibrariesReply,
  type AccountWorkspace,
  type AccountWorkspaceList,
  type ColdChainsReply,
  ENGINE_TOKEN_HEADER,
  type FileNode,
  type FrozenChain,
  type ModelDownloadJob,
  type RepoProject,
  USER_HEADER,
  WORKSPACE_HEADER,
} from "@studio/web-protocol";
import {
  ABOUT_PAGE,
  LIBRARIES_PAGE,
  localProjectUrl,
  SETTINGS_PAGE,
  SURFACES_PAGE,
  TASKS_PAGE,
} from "@studio/web-protocol/projects";

/** The one field an address is chosen from — narrower than `AccountWorkspace` on purpose, the surfaces listing carrying it flattened. */
type Addressable = Pick<AccountWorkspace, "name">;

/** Where a project opens: by its workspace's folder name, the one address (../../docs/projects.md). */
export function projectHref(
  ws: Addressable,
  p: Pick<RepoProject, "path" | "app">,
): string {
  return localProjectUrl(window.location.origin, ws.name, p.path, p.app);
}

/**
 * Where the WORKSPACE itself opens — the same choice of address with `.` for the project, which
 * the file service then resolves: the one last open, else the first, else the first-project wizard.
 * This is the only way into a workspace that holds nothing (../../docs/projects.md).
 */
export function workspaceHref(ws: Addressable): string {
  return localProjectUrl(window.location.origin, ws.name, ".", "k7");
}

/** The LIBRARIES page as a ROUTE: with no project in the address the router's basename is the mount, so the home pages switch without a reload. */
export const LIBRARIES_ROUTE = `/${LIBRARIES_PAGE}`;

/** …and the FROZEN CHAINS page under it, so no workspace name is taken (../../docs/frozenChains.md). */
export const FROZEN_CHAINS_ROUTE = `/${LIBRARIES_PAGE}/chains`;

/** …and the SETTINGS page, the same way. */
export const SETTINGS_ROUTE = `/${SETTINGS_PAGE}`;

/** …and the ABOUT page: k7's version, its licence, its source and the third-party notices. */
export const ABOUT_ROUTE = `/${ABOUT_PAGE}`;

/** …and the ENGINE page beside the settings: this machine's on the desktop app, the engines this account may use everywhere. */
export const ENGINE_SETTINGS_ROUTE = `/${SETTINGS_PAGE}/engine`;

/** …and the SURFACES page: the surfaces of every project open on the network. */
export const SURFACES_ROUTE = `/${SURFACES_PAGE}`;

/** …and the TASKS page, the same way. */
export const TASKS_ROUTE = `/${TASKS_PAGE}`;

// ── the chosen engine's file service: the working copies ────────────────────────

/** The file service this screen acts on — the account's, on the server — and the token that opens the door. */
export interface AccountFiles {
  fileBase: string;
  token: string;
  /** Who is asking — read off `/auth/me` rather than off a configured workspace, this page having none. */
  userEmail: string;
}

/** One call to the account's file service: its headers, the workspace a row names, and a body when there is one. */
async function call<T>(
  m: AccountFiles,
  path: string,
  what: string,
  opts: {
    method?: string;
    workspace?: string;
    json?: unknown;
    body?: BodyInit;
    type?: string;
    headers?: Record<string, string>;
  } = {},
): Promise<T> {
  const headers: Record<string, string> = {
    ...filesHeaders(m),
    ...opts.headers,
  };
  if (opts.workspace) headers[WORKSPACE_HEADER] = opts.workspace;
  let body = opts.body;
  if (opts.json !== undefined) {
    headers["content-type"] = "application/json";
    body = JSON.stringify(opts.json);
  } else if (opts.type) headers["content-type"] = opts.type;
  return readJson<T>(
    await fetch(`${m.fileBase}${path}`, { method: opts.method, headers, body }),
    what,
  );
}

/** The file-service calls this page makes, each carrying its file service explicitly: a row touches a different workspace every time. */
export const accountFiles = {
  /**
   * The LIBRARIES the origin holds — an empty list is the ordinary state of a new one.
   *
   * Named service and not through `fileApi`: this screen opens no project, so the module-level
   * transport is unconfigured and `cfg()` would throw "no project opened yet" (../../docs/projects.md).
   */
  libraries: async (m: AccountFiles): Promise<AccountLibrariesReply> =>
    call<AccountLibrariesReply>(m, `/account/libraries`, "libraries"),

  /** The Frozen chains page's list: the account's own chains, then the libraries' cold ones (../../docs/frozenChains.md). */
  frozenChains: async (m: AccountFiles): Promise<{ chains: FrozenChain[] }> =>
    call(m, `/account/frozen-chains`, "frozen-chains"),

  /** Copy any library chain whole into the account's, as `name`; the reply is the copy as it reads. */
  copyFrozenChain: async (
    m: AccountFiles,
    from: string,
    name: string,
  ): Promise<FrozenChain> =>
    call(m, `/account/frozen-chains/${encodeURIComponent(name)}/copy`, "copy", {
      method: "POST",
      json: { from },
    }),

  /** Compile a live chain into a slot and hand it to this machine's supervisor, which keeps it (../../docs/slots.md). */
  saveSlot: async (m: AccountFiles, ref: string): Promise<string> => {
    const slot = await call<{ id: string }>(m, `/account/slot`, "slot", {
      method: "POST",
      json: { ref },
    });
    const r = await fetch("/api/local/engine/slots", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(slot),
    });
    if (!r.ok)
      throw new Error(
        ((await r.json().catch(() => ({}))) as { error?: string }).error ??
          `${r.status}`,
      );
    return slot.id;
  },

  deleteFrozenChain: async (m: AccountFiles, name: string): Promise<void> => {
    await call(
      m,
      `/account/frozen-chains/${encodeURIComponent(name)}`,
      "delete",
      {
        method: "DELETE",
      },
    );
  },

  /** Which COLD chains of the library this machine separates and makes instrumentals with, and the ones it could — machine settings, no project open. */
  coldChains: async (m: AccountFiles): Promise<ColdChainsReply> =>
    call(m, `/account/cold-chains`, "cold-chains"),

  /** Choose one — `""` clears it. The far side refuses a chain that is not cold, or writes no instrumental when that is asked. */
  setColdChain: async (
    m: AccountFiles,
    which: "stems" | "instrumental",
    ref: string,
  ): Promise<ColdChainsReply> =>
    call(m, `/account/cold-chains/${which}`, "cold-chains", {
      method: "PUT",
      json: { ref },
    }),

  /** Download the models a cold chain needs and this machine has not got — one job each; the URLs are the chain's modules', never sent from here. */
  downloadChainModels: async (
    m: AccountFiles,
    ref: string,
  ): Promise<ModelDownloadJob[]> => {
    const { jobs } = await call<{ jobs: ModelDownloadJob[] }>(
      m,
      `/account/cold-chains/download`,
      "cold-chains/download",
      { method: "POST", json: { ref } },
    );
    return jobs;
  },

  /** One model download, as it stands. */
  modelDownload: async (
    m: AccountFiles,
    id: string,
  ): Promise<ModelDownloadJob> =>
    call(m, `/account/models/${encodeURIComponent(id)}`, "models"),

  /** Every workspace of the account, with the projects in it — the file service answers, they being folders on the server. */
  workspaces: async (m: AccountFiles): Promise<AccountWorkspace[]> => {
    const { workspaces } = await call<AccountWorkspaceList>(
      m,
      `/workspaces/projects`,
      "workspaces",
    );
    return workspaces;
  },

  /** Start bringing a server workspace down here, as a copy of its own; the answer names its folder here. */
  downloadWorkspace: async (
    m: AccountFiles,
    name: string,
  ): Promise<{ name: string }> =>
    call<{ name: string }>(m, `/workspaces/import`, "download", {
      method: "POST",
      json: { name },
    }),

  /** One workspace's whole tree, asked of a workspace this page has not opened — `/tree` needs one. */
  workspaceTree: async (
    m: AccountFiles,
    workspace: string,
  ): Promise<FileNode[]> => call(m, `/workspace-tree`, "tree", { workspace }),

  /** A workspace and NOTHING else: an empty folder. */
  createWorkspace: async (m: AccountFiles, name: string): Promise<void> => {
    await call(m, `/workspaces`, "workspace", {
      method: "POST",
      json: { name },
    });
  },

  /** Scaffold a project in one of the account's workspaces: `name` carries its suffix, and `shape` is the ARCHETYPE (`PROJECT_SHAPES` in @studio/song). */
  createProject: async (
    m: AccountFiles,
    workspace: string,
    name: string,
    shape: ProjectShape,
  ): Promise<{ project: string; missing: string[] }> => {
    const { project, missing } = await call<{
      project: string;
      missing?: string[];
    }>(m, `/project/create`, "create", {
      method: "POST",
      workspace,
      json: { shape, name },
    });
    return { project, missing: missing ?? [] };
  },

  /**
   * A foreign project DROPPED on a workspace row: the backend translates it, writes its samples and lands the texts
   * under the folder the popin named — nothing is left in the workspace but the project (apps/k7-daw-backend/docs/import.md).
   */
  importProjectFile: async (
    m: AccountFiles,
    workspace: string,
    file: File,
    folder: string,
  ): Promise<{ project: string; log: string[] }> =>
    call(m, "/import", "import", {
      method: "POST",
      workspace,
      body: file,
      type: "application/octet-stream",
      headers: {
        "x-k7-file": encodeURIComponent(file.name),
        ...(folder ? { "x-k7-as": encodeURIComponent(folder) } : {}),
      },
    }),

  /** Copy a project into another of the account's workspaces. No workspace HEADER: the body names both. */
  copyProject: async (
    m: AccountFiles,
    from: string,
    path: string,
    to: string,
    name: string,
  ): Promise<string> => {
    const { project } = await call<{ project: string }>(
      m,
      `/workspaces/copy-project`,
      "copy",
      { method: "POST", json: { from, path, to, name } },
    );
    return project;
  },

  /** Rename a project's FOLDER in place; the suffix stays. */
  renameProject: async (
    m: AccountFiles,
    workspace: string,
    path: string,
    name: string,
  ): Promise<string> => {
    const { path: renamed } = await call<{ path: string }>(
      m,
      `/project/rename`,
      "rename",
      { method: "POST", workspace, json: { path, name } },
    );
    return renamed;
  },

  /** Delete a workspace — the folder and everything under it, for good. */
  removeWorkspace: async (
    m: AccountFiles,
    workspace: string,
  ): Promise<void> => {
    await call(m, `/workspaces/remove`, "delete", {
      method: "POST",
      json: { name: workspace },
    });
  },

  /** Unload the project and move its folder to the workspace's recycle bin. */
  deleteProject: async (
    m: AccountFiles,
    workspace: string,
    path: string,
  ): Promise<void> => {
    await call(m, `/project/delete`, "delete", {
      method: "POST",
      workspace,
      json: { path },
    });
  },
};

export const filesHeaders = (m: AccountFiles): Record<string, string> => ({
  [USER_HEADER]: m.userEmail,
  [ENGINE_TOKEN_HEADER]: m.token,
});
