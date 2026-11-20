// project.ts — every file-service route a project app needs: files.ts's, plus the compiled views
// the engine and the graph are drawn from, the modules, the CLI's verbs and the undo journal.
// k7 spreads this into its own client and adds what only it has (timeline, waveforms, video).

import type { ProjectDoc } from "@studio/engine-protocol/project";
import type { WebGraph } from "@studio/graph-model";
// The wire shape only, type-only, so nothing of the parser that walks a disk ships here.
import type { ProjectSurfaces } from "@studio/song-schema/yaml/surfaceFile";
import type {
  CatalogueModule,
  Hints,
  HistoryListEntry,
  HistoryState,
  IoPort,
  ModuleLibrary,
  ModuleUiFile,
  ProjectErrors,
  ProjectInfo,
  UndoResult,
  WebPreferences,
  WebProject,
  WebVideoProject,
} from "@studio/web-protocol";
import {
  fenceReads,
  fget,
  fsend,
  ReadOnlyDocument,
  refusedWrite,
} from "./fileApi.ts";
import { fileRoutes } from "./files.ts";
import { verbsHost } from "./verbsHost.ts";

/** One `/complete` candidate: the line it completes to, and what a list shows for its value instead. */
export interface Completion {
  line: string;
  label?: string;
}

/** A derived view a writing command may be asked to build — a list, see docs/fileApi.md. */
export type CommandWant = "graph";

/** How a command's writes land in the undo journal when not as a step of their own — see docs/fileApi.md. */
export type CommandJournal = "join" | "skip";

export const projectApi = {
  ...fileRoutes,

  // --- The open project's libraries, modules and scaffolding --------------------
  /** The module libraries this project can reach, nearest first — see docs/fileApi.md. */
  moduleLibraries: () =>
    fget<{ libraries: ModuleLibrary[] }>("/module-libraries"),
  /** The CHAINS a library holds: `ref` is what one is called, `path` where it is. */
  libraryChains: () =>
    fget<{ chains: { ref: string; path: string; nodes: string[] }[] }>(
      "/library-chains",
    ),
  /** …and the ASSEMBLY templates, the third family — apps/k7-daw-frontend/packages/song/docs/assemblies.md. */
  libraryAssemblies: () =>
    fget<{ assemblies: { ref: string; path: string; file: string }[] }>(
      "/library-assemblies",
    ),
  /** Make an empty module in one: `root` is a path that call answered with, `rel` at least `<type>/<name>`. */
  createModule: (root: string, rel: string) =>
    fsend<{ path: string; dir: string }>("POST", "/module", { root, rel }),
  project: () => fget<ProjectInfo>("/project"),
  /** Scaffold a project in the OPEN workspace: `name` carries its suffix, `shape` is the archetype. */
  createProject: (shape: string, name: string) =>
    fsend<{ project: string; missing?: string[] }>("POST", "/project/create", {
      shape,
      name,
    }),
  /** Play a document this browser built, through the service so ONE thing writes to the engine. */
  sendEngineProject: (doc: ProjectDoc) =>
    fsend<{ ok: true }>("POST", "/engine/project", { doc }),

  // --- Derived views. A cheap token changes when the sources do; poll it, not these. ---
  revision: () => fget<{ rev: string }>("/revision"),
  projectDoc: () => fget<WebProject>("/project-doc"),
  // The compiled document the ENGINE takes, for a surface that drives one itself — the desk.
  engineDoc: () => fget<ProjectDoc>("/engine-doc"),
  // The same folder read as PICTURE, by the Worker's picture compile — see docs/fileApi.md.
  videoDoc: async (): Promise<WebVideoProject> => {
    const r = await verbsHost().videoDoc();
    if ("doc" in r) return r.doc;
    // A 400, as the route said: the view is replaced by the compile's sentence, not kept (readFailure.ts).
    throw Object.assign(new Error(r.error), { status: 400 });
  },
  videoErrors: (): Promise<ProjectErrors> => verbsHost().videoErrors(),
  graph: () => fget<WebGraph>("/graph"),
  /** The graph of a library CHAIN, by the path `/library-chains` gave — the builder's drawing. */
  graphAt: (path: string) =>
    fget<WebGraph>(`/graph?at=${encodeURIComponent(path)}`),
  // The project's `surfaces/` folder, parsed, and empty for almost every project — apps/k7-daw-frontend/packages/song/docs/surfaces.md.
  surfaces: () => fget<ProjectSurfaces>("/surfaces"),
  projectErrors: () => fget<ProjectErrors>("/project-errors"),

  /** Move a library module to another folder of its library, its refs repointed — apps/k7-daw-frontend/packages/song/docs/moduleMove.md. */
  moveLibraryModule: (path: string, toDir: string) =>
    fsend<{ path: string; from: string; to: string; repointed: string[] }>(
      "POST",
      "/library/move",
      { path, toDir },
    ),
  // --- Modules, as a project sees them ------------------------------------------
  hints: () => fget<Hints>("/hints"),
  /** The refs a node may be an instance of, each with its panel size when it draws one. */
  baseModules: () =>
    fget<{ ref: string; panel?: { cols: number; rows: number } }[]>(
      "/base-modules",
    ),
  ioPorts: () => fget<IoPort[]>("/io-ports"),
  moduleCatalogue: () => fget<CatalogueModule[]>("/module-catalogue"),
  moduleUi: (path: string) =>
    fget<ModuleUiFile>(`/module-ui?path=${encodeURIComponent(path)}`),
  /** The console systems this project may run, and the one it does — apps/k7-daw-frontend/packages/song/docs/console.md. */
  consoles: () =>
    fget<{ systems: { ref: string; label: string }[]; current?: string }>(
      "/consoles",
    ),
  /** The engines beside a module file — apps/k7-daw-frontend/packages/song/docs/engines.md. */
  moduleEngines: (path: string) =>
    fget<{ engines: string[]; engine?: string }>(
      `/module-engines?path=${encodeURIComponent(path)}`,
    ),
  cloneModule: (path: string, newName: string) =>
    fsend<{ path: string }>("POST", "/module/clone", { path, newName }),
  // `node` is optional: without one the service names it `<module>_<n>` (see `freeNodeName`).
  createNode: (path: string, node?: string) =>
    fsend<{ path: string }>("POST", "/node", {
      path,
      ...(node ? { node } : {}),
    }),

  // --- The CLI's own verbs, which the command bar dispatches through ------------
  commands: () => verbsHost().catalogue(),
  /** Every candidate as an item, the label set only where the host gave one — commands/docs/completions.md. */
  complete: (line: string) =>
    verbsHost()
      .complete(line)
      .then((list) =>
        list.map((c): Completion => (typeof c === "string" ? { line: c } : c)),
      ),
  /** Run a verb; `want`, `graph`, `rev`, `refused` and `changed` are all in docs/fileApi.md. */
  command: async (
    line: string,
    // `at` points the verbs at another folder — a path this caller was GIVEN, never one it built.
    opts?: { at?: string; want?: CommandWant[]; journal?: CommandJournal },
  ) => {
    const locked = refusedWrite({ kind: "command", line });
    if (locked) throw new ReadOnlyDocument(locked);
    // Absent `want` means the graph, as the route said; an empty list is a caller refusing it.
    const r = await verbsHost().run(line, {
      ...(opts?.at ? { at: opts.at } : {}),
      want: opts?.want ?? ["graph"],
      ...(opts?.journal ? { journal: opts.journal } : {}),
    });
    // The verb wrote around the transport: reads shared before it are not the project now.
    if (r.rev !== undefined && r.rev !== r.revBefore) fenceReads();
    return {
      output: r.output,
      ...(r.refused ? { refused: true } : {}),
      ...(r.graph ? { graph: r.graph as WebGraph } : {}),
      ...(r.rev !== undefined ? { rev: r.rev } : {}),
      ...(r.revBefore !== undefined ? { revBefore: r.revBefore } : {}),
      ...(r.changed ? { changed: r.changed } : {}),
    };
  },

  // --- The project-wide undo/redo journal, driven by Ctrl+Z / Ctrl+Y ------------
  historyState: () => fget<HistoryState>("/history"),
  /** Every step of the open project's journal, oldest first — the History panel (docs/history.md). */
  historyEntries: () => fget<HistoryListEntry[]>("/history/entries"),
  // `confirm` applies a process step; without it one comes back `needsConfirm` for the client to gate.
  undo: (confirm?: boolean) => fsend<UndoResult>("POST", "/undo", { confirm }),
  redo: (confirm?: boolean) => fsend<UndoResult>("POST", "/redo", { confirm }),

  // --- This machine's preferences, and the merged engine + file-service log -----
  preferences: () => fget<WebPreferences>("/preferences"),
  setVim: (vim: boolean) =>
    fsend<{ vim: boolean }>("PUT", "/preferences/vim", { vim }),
  /** Both halves of the Logs panel: the engine machine's `.studio.log` by byte `offset`, this service's own lines after `after`. */
  logs: (offset?: number, tail = 300, after?: number) =>
    fget<{
      offset: number;
      lines: string[];
      service?: {
        seq: number;
        lines: {
          seq: number;
          t: number;
          level: "info" | "warn" | "error";
          text: string;
        }[];
      };
    }>(
      `/logs?${offset != null ? `offset=${offset}` : `tail=${tail}`}${after != null ? `&after=${after}` : ""}`,
    ),
};
